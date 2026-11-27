#![forbid(unsafe_code)]

//! 固定大小的工作者池。任务合作式响应取消和超时，队列满时在有限等待后拒绝。

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::runtime::{Limits, MAX_TASK_TIMEOUT, RuntimeError};

static NEXT_LABEL: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
    shared: Weak<Shared>,
    id: u64,
}

impl Cancel {
    pub fn cancel(&self) {
        self.flag.store(true, AtomicOrdering::Release);
        if let Some(shared) = self.shared.upgrade() {
            shared.remove_queued(self.id);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(AtomicOrdering::Acquire)
    }
}

pub struct Task {
    pub id: TaskId,
    pub cancel: Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedulerStats {
    pub submitted: u64,
    pub finished: u64,
    pub cancelled: u64,
    pub timed_out: u64,
    pub rejected: u64,
    pub panicked: u64,
    pub queued: usize,
    pub inflight: usize,
    pub live_threads: usize,
}

struct Queued {
    priority: u8,
    order: u64,
    id: u64,
    flag: Arc<AtomicBool>,
    timeout: Duration,
    work: Box<dyn FnOnce(&Cancel) + Send>,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.order == other.order
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.order.cmp(&self.order))
    }
}

struct Inflight {
    id: u64,
    flag: Arc<AtomicBool>,
    deadline: Instant,
    timed_out: bool,
}

struct State {
    queue: BinaryHeap<Queued>,
    inflight: Vec<Inflight>,
    shutdown: bool,
    next_id: u64,
    submitted: u64,
    finished: u64,
    cancelled: u64,
    timed_out: u64,
    rejected: u64,
    panicked: u64,
}

struct Shared {
    limits: Limits,
    label: String,
    mu: Mutex<State>,
    cv: Condvar,
    live: AtomicUsize,
    #[cfg(target_os = "linux")]
    tids: Mutex<Vec<u64>>,
}

impl Shared {
    fn remove_queued(&self, id: u64) {
        let mut state = self.mu.lock().expect("scheduler lock");
        let mut kept = BinaryHeap::new();
        let pending = std::mem::take(&mut state.queue);
        let mut removed = false;
        for job in pending.into_iter() {
            if job.id == id && !removed {
                removed = true;
                state.cancelled += 1;
            } else {
                kept.push(job);
            }
        }
        state.queue = kept;
        if removed {
            self.cv.notify_all();
        }
    }
}

pub struct Scheduler {
    shared: Arc<Shared>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Scheduler {
    pub fn start(limits: Limits) -> Result<Self, RuntimeError> {
        limits.check()?;
        let label = format!(
            "w{:04x}",
            NEXT_LABEL.fetch_add(1, AtomicOrdering::Relaxed) & 0xffff
        );
        let shared = Arc::new(Shared {
            limits,
            label,
            mu: Mutex::new(State {
                queue: BinaryHeap::new(),
                inflight: Vec::new(),
                shutdown: false,
                next_id: 1,
                submitted: 0,
                finished: 0,
                cancelled: 0,
                timed_out: 0,
                rejected: 0,
                panicked: 0,
            }),
            cv: Condvar::new(),
            live: AtomicUsize::new(0),
            #[cfg(target_os = "linux")]
            tids: Mutex::new(Vec::new()),
        });
        let mut handles = Vec::new();
        for index in 0..shared.limits.workers {
            if let Err(error) = spawn_thread(
                &shared,
                &mut handles,
                format!("{}k{index}", shared.label),
                worker_main,
            ) {
                release_started(&shared, &mut handles);
                return Err(error);
            }
        }
        if let Err(error) = spawn_thread(
            &shared,
            &mut handles,
            format!("{}t", shared.label),
            timer_main,
        ) {
            release_started(&shared, &mut handles);
            return Err(error);
        }
        Ok(Self {
            shared,
            threads: Mutex::new(handles),
        })
    }

    pub fn submit<F>(&self, priority: u8, timeout: Duration, work: F) -> Result<Task, RuntimeError>
    where
        F: FnOnce(&Cancel) + Send + 'static,
    {
        if timeout.is_zero() || timeout > MAX_TASK_TIMEOUT {
            return Err(RuntimeError::new(
                "invalid_limit",
                "task timeout is outside the supported range",
            ));
        }
        let flag = Arc::new(AtomicBool::new(false));
        let mut work = Some(work);
        let mut state = self.shared.mu.lock().expect("scheduler lock");
        let deadline = Instant::now() + self.shared.limits.admission_wait;
        loop {
            if state.shutdown {
                return Err(RuntimeError::new(
                    "shutting_down",
                    "scheduler is shutting down",
                ));
            }
            if state.queue.len() < self.shared.limits.task_queue {
                let id = state.next_id;
                state.next_id += 1;
                let cancel = Cancel {
                    flag: Arc::clone(&flag),
                    shared: Arc::downgrade(&self.shared),
                    id,
                };
                state.queue.push(Queued {
                    priority,
                    order: id,
                    id,
                    flag,
                    timeout,
                    work: Box::new(work.take().expect("task is submitted once")),
                });
                state.submitted += 1;
                self.shared.cv.notify_all();
                return Ok(Task {
                    id: TaskId(id),
                    cancel,
                });
            }
            let now = Instant::now();
            if now >= deadline {
                state.rejected += 1;
                return Err(RuntimeError::new(
                    "queue_full",
                    "task queue remained full until the wait elapsed",
                ));
            }
            let (guard, _) = self
                .shared
                .cv
                .wait_timeout(state, deadline.saturating_duration_since(now))
                .expect("scheduler lock");
            state = guard;
        }
    }

    pub fn stats(&self) -> SchedulerStats {
        let state = self.shared.mu.lock().expect("scheduler lock");
        SchedulerStats {
            submitted: state.submitted,
            finished: state.finished,
            cancelled: state.cancelled,
            timed_out: state.timed_out,
            rejected: state.rejected,
            panicked: state.panicked,
            queued: state.queue.len(),
            inflight: state.inflight.len(),
            live_threads: self.shared.live.load(AtomicOrdering::Acquire),
        }
    }

    pub fn shutdown(&self) {
        self.shared.begin_shutdown();
        let mut handles = self.threads.lock().expect("scheduler thread list");
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn named_threads_alive(&self) -> usize {
        self.shared
            .tids
            .lock()
            .expect("thread ids")
            .iter()
            .filter(|tid| {
                thread_name(**tid).is_some_and(|name| name.starts_with(&self.shared.label))
            })
            .count()
    }
}

impl Shared {
    fn begin_shutdown(&self) {
        let mut state = self.mu.lock().expect("scheduler lock");
        state.shutdown = true;
        while let Some(job) = state.queue.pop() {
            job.flag.store(true, AtomicOrdering::Release);
            state.cancelled += 1;
        }
        for job in &state.inflight {
            job.flag.store(true, AtomicOrdering::Release);
        }
        self.cv.notify_all();
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn_thread(
    shared: &Arc<Shared>,
    handles: &mut Vec<JoinHandle<()>>,
    name: String,
    entry: fn(Arc<Shared>),
) -> Result<(), RuntimeError> {
    let shared = Arc::clone(shared);
    let handle = thread::Builder::new()
        .name(name)
        .spawn(move || entry(shared))
        .map_err(|_| {
            RuntimeError::new(
                "resource_limit",
                "the operating system refused a scheduler thread",
            )
        })?;
    handles.push(handle);
    Ok(())
}

fn release_started(shared: &Shared, handles: &mut Vec<JoinHandle<()>>) {
    shared.begin_shutdown();
    for handle in handles.drain(..) {
        let _ = handle.join();
    }
}

struct LiveGuard<'a>(&'a Shared);

impl Drop for LiveGuard<'_> {
    fn drop(&mut self) {
        self.0.live.fetch_sub(1, AtomicOrdering::AcqRel);
    }
}

fn enter_thread(shared: &Shared) -> LiveGuard<'_> {
    shared.live.fetch_add(1, AtomicOrdering::AcqRel);
    #[cfg(target_os = "linux")]
    if let Some(tid) = current_tid() {
        shared.tids.lock().expect("thread ids").push(tid);
    }
    LiveGuard(shared)
}

fn worker_main(shared: Arc<Shared>) {
    let _live = enter_thread(&shared);
    loop {
        let job = {
            let mut state = shared.mu.lock().expect("scheduler lock");
            loop {
                if let Some(job) = state.queue.pop() {
                    if job.flag.load(AtomicOrdering::Acquire) {
                        state.cancelled += 1;
                        continue;
                    }
                    state.inflight.push(Inflight {
                        id: job.id,
                        flag: Arc::clone(&job.flag),
                        deadline: Instant::now() + job.timeout,
                        timed_out: false,
                    });
                    shared.cv.notify_all();
                    break job;
                }
                if state.shutdown {
                    return;
                }
                state = shared.cv.wait(state).expect("scheduler lock");
            }
        };
        let cancel = Cancel {
            flag: Arc::clone(&job.flag),
            shared: Arc::downgrade(&shared),
            id: job.id,
        };
        let ran = panic::catch_unwind(AssertUnwindSafe(|| (job.work)(&cancel)));
        let mut state = shared.mu.lock().expect("scheduler lock");
        state.inflight.retain(|item| item.id != job.id);
        if ran.is_err() {
            state.panicked += 1;
        } else {
            state.finished += 1;
        }
        shared.cv.notify_all();
    }
}

fn timer_main(shared: Arc<Shared>) {
    let _live = enter_thread(&shared);
    loop {
        let mut state = shared.mu.lock().expect("scheduler lock");
        let now = Instant::now();
        let mut next_deadline: Option<Instant> = None;
        let mut newly_timed_out = 0u64;
        for job in &mut state.inflight {
            if job.deadline <= now {
                if !job.timed_out {
                    job.flag.store(true, AtomicOrdering::Release);
                    job.timed_out = true;
                    newly_timed_out += 1;
                }
            } else {
                next_deadline = Some(match next_deadline {
                    Some(earliest) => earliest.min(job.deadline),
                    None => job.deadline,
                });
            }
        }
        state.timed_out += newly_timed_out;
        if state.shutdown && state.inflight.is_empty() {
            return;
        }
        match next_deadline {
            Some(deadline) => {
                let wait = deadline.saturating_duration_since(Instant::now());
                let (_guard, _) = shared.cv.wait_timeout(state, wait).expect("scheduler lock");
            }
            None => {
                let _guard = shared.cv.wait(state).expect("scheduler lock");
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn current_tid() -> Option<u64> {
    let link = std::fs::read_link("/proc/thread-self").ok()?;
    link.file_name()?.to_str()?.parse().ok()
}

#[cfg(all(test, target_os = "linux"))]
fn thread_name(tid: u64) -> Option<String> {
    std::fs::read_to_string(format!("/proc/self/task/{tid}/comm"))
        .ok()
        .map(|name| name.trim().to_owned())
}
