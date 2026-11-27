#![forbid(unsafe_code)]

//! 进程内事件总线。
//!
//! 事件以 `Arc` 分发给订阅方，热路径不再做 JSON 编解码。序号由生产者分配；
//! 总线只在同一来源内核对连续性。队列满时先做有限等待，仍无法入队则把丢失
//! 记成协议缺口，而不是丢掉不留痕迹。来源标识不授予权限。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use winstrata_protocols::{
    Counter, Event, EventTime, GapReason, Record, SequenceRange, Source, Version,
};

use crate::runtime::{Limits, MAX_ADMISSION_WAIT, RuntimeError};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub device: Option<winstrata_protocols::Id>,
    pub provider: Option<winstrata_protocols::Id>,
    pub event_type: Option<winstrata_protocols::Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishOutcome {
    pub sequence: u64,
    pub enqueued: usize,
    pub lost: usize,
    pub filtered: usize,
}

#[derive(Debug)]
pub struct PublishError {
    pub error: RuntimeError,
    pub event: Event,
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.error)
    }
}

impl std::error::Error for PublishError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusStats {
    pub accepted: u64,
    pub duplicate_rejects: u64,
    pub missing_rejects: u64,
    pub invalid_rejects: u64,
    pub queue_rejects: u64,
    pub lost: u64,
    pub filtered: u64,
    pub released: u64,
    pub queued: usize,
    pub pending: usize,
    pub subscribers: usize,
    pub global_queued: usize,
}

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<BusInner>,
}

struct BusInner {
    limits: Limits,
    mu: Mutex<BusState>,
    cv: Condvar,
}

struct BusState {
    closed: bool,
    next_id: u64,
    next_admit: u64,
    subs: Vec<Subscriber>,
    cursors: HashMap<Source, u64>,
    accepted: u64,
    duplicate_rejects: u64,
    missing_rejects: u64,
    invalid_rejects: u64,
    queue_rejects: u64,
    lost: u64,
    filtered: u64,
    released: u64,
    global_used: usize,
}

struct Subscriber {
    id: u64,
    priority: u8,
    filter: Filter,
    queue: VecDeque<Item>,
    pending: HashMap<Source, Pending>,
    lost: u64,
    filtered: u64,
}

struct Item {
    admit: u64,
    event: Arc<Event>,
}

struct Pending {
    reason: GapReason,
    first: u64,
    last: u64,
    admit: u64,
    time: EventTime,
}

enum Decision {
    Deliver { flush: bool },
    Record { flush: bool, reason: GapReason },
}

struct Placement {
    index: usize,
    decision: Decision,
}

impl EventBus {
    pub fn new(limits: Limits) -> Result<Self, RuntimeError> {
        limits.check()?;
        Ok(Self {
            inner: Arc::new(BusInner {
                limits,
                mu: Mutex::new(BusState {
                    closed: false,
                    next_id: 1,
                    next_admit: 1,
                    subs: Vec::new(),
                    cursors: HashMap::new(),
                    accepted: 0,
                    duplicate_rejects: 0,
                    missing_rejects: 0,
                    invalid_rejects: 0,
                    queue_rejects: 0,
                    lost: 0,
                    filtered: 0,
                    released: 0,
                    global_used: 0,
                }),
                cv: Condvar::new(),
            }),
        })
    }

    pub fn subscribe(&self, priority: u8, filter: Filter) -> Result<Subscription, RuntimeError> {
        let mut state = self.lock();
        if state.closed {
            return Err(RuntimeError::new(
                "shutting_down",
                "event bus is shutting down",
            ));
        }
        if state.subs.len() >= self.inner.limits.subscriptions {
            return Err(RuntimeError::new(
                "resource_limit",
                "subscription limit has been reached",
            ));
        }
        let id = state.next_id;
        state.next_id += 1;
        state.subs.push(Subscriber {
            id,
            priority,
            filter,
            queue: VecDeque::new(),
            pending: HashMap::new(),
            lost: 0,
            filtered: 0,
        });
        Ok(Subscription {
            inner: Arc::clone(&self.inner),
            id,
        })
    }

    pub fn publish(&self, mut event: Event) -> Result<PublishOutcome, PublishError> {
        let mut state = self.lock();
        if state.closed {
            return rejected(
                state,
                RuntimeError::new("shutting_down", "event bus is shutting down"),
                event,
            );
        }
        if let Err(error) = reconcile(state.cursors.get(&event.source).copied(), &mut event) {
            return rejected(state, error, event);
        }
        let deadline = Instant::now() + self.inner.limits.admission_wait;
        let cap = self.inner.limits.event_queue;
        let global_limit = self.inner.limits.global_queue;
        loop {
            if state.closed {
                return rejected(
                    state,
                    RuntimeError::new("shutting_down", "event bus is shutting down"),
                    event,
                );
            }
            let decision = {
                let indexes = ordered_subscribers(&state, &event.source);
                let free = global_limit.saturating_sub(state.global_used);
                plan_delivery(&state, &indexes, &event, free, cap)
            };
            let expired = Instant::now() >= deadline;
            let accept = decision
                .as_ref()
                .is_some_and(|plan| expired || !records_loss(plan));
            if accept {
                let outcome = commit(
                    &self.inner.cv,
                    state,
                    decision.expect("accepted plan"),
                    event,
                );
                return Ok(outcome);
            }
            if expired {
                return rejected(
                    state,
                    RuntimeError::new(
                        "queue_full",
                        "event queue remained full until the wait elapsed",
                    ),
                    event,
                );
            }
            let (guard, _) = self
                .inner
                .cv
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .expect("event bus lock");
            state = guard;
        }
    }

    pub fn stats(&self) -> BusStats {
        let state = self.lock();
        let queued = state.subs.iter().map(|sub| sub.queue.len()).sum();
        let pending = state
            .subs
            .iter()
            .map(|sub| pending_events(&sub.pending))
            .sum();
        BusStats {
            accepted: state.accepted,
            duplicate_rejects: state.duplicate_rejects,
            missing_rejects: state.missing_rejects,
            invalid_rejects: state.invalid_rejects,
            queue_rejects: state.queue_rejects,
            lost: state.lost,
            filtered: state.filtered,
            released: state.released,
            queued,
            pending,
            subscribers: state.subs.len(),
            global_queued: state.global_used,
        }
    }

    pub fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.inner.cv.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, BusState> {
        self.inner.mu.lock().expect("event bus lock")
    }
}

pub struct Subscription {
    inner: Arc<BusInner>,
    id: u64,
}

impl Subscription {
    pub fn recv_batch(&self, max: usize, wait: Duration) -> Result<Vec<Arc<Event>>, RuntimeError> {
        if max == 0 || wait > MAX_ADMISSION_WAIT {
            return Err(RuntimeError::new(
                "invalid_limit",
                "batch size or wait is outside the supported range",
            ));
        }
        let max = max.min(self.inner.limits.batch);
        let mut state = self.inner.mu.lock().expect("event bus lock");
        let deadline = Instant::now() + wait;
        loop {
            if !state.subs.iter().any(|sub| sub.id == self.id) {
                return Ok(Vec::new());
            }
            if let Some(batch) = take_batch(&mut state, self.id, max) {
                self.inner.cv.notify_all();
                return Ok(batch);
            }
            if state.closed || Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            let (guard, _) = self
                .inner
                .cv
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .expect("event bus lock");
            state = guard;
        }
    }

    pub fn backlog(&self) -> (usize, usize) {
        let state = self.inner.mu.lock().expect("event bus lock");
        state
            .subs
            .iter()
            .find(|sub| sub.id == self.id)
            .map(|sub| (sub.queue.len(), pending_events(&sub.pending)))
            .unwrap_or((0, 0))
    }

    pub fn is_closed(&self) -> bool {
        let state = self.inner.mu.lock().expect("event bus lock");
        state.closed || !state.subs.iter().any(|sub| sub.id == self.id)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut state = self.inner.mu.lock().expect("event bus lock");
        let Some(index) = state.subs.iter().position(|sub| sub.id == self.id) else {
            return;
        };
        let sub = state.subs.remove(index);
        state.global_used = state.global_used.saturating_sub(sub.queue.len());
        state.released += sub.queue.len() as u64 + pending_events(&sub.pending) as u64;
        self.inner.cv.notify_all();
    }
}

fn rejected(
    mut state: MutexGuard<'_, BusState>,
    error: RuntimeError,
    event: Event,
) -> Result<PublishOutcome, PublishError> {
    match error.code() {
        "duplicate_sequence" => state.duplicate_rejects += 1,
        "missing_record" => state.missing_rejects += 1,
        "invalid_record" => state.invalid_rejects += 1,
        "queue_full" => state.queue_rejects += 1,
        _ => {}
    }
    Err(PublishError { error, event })
}

fn records_loss(plan: &[Placement]) -> bool {
    plan.iter().any(|placement| match &placement.decision {
        Decision::Record { reason, .. } => reason == &GapReason::Lost,
        Decision::Deliver { .. } => false,
    })
}

fn ordered_subscribers(state: &BusState, source: &Source) -> Vec<usize> {
    let mut indexes: Vec<usize> = state
        .subs
        .iter()
        .enumerate()
        .filter(|(_, sub)| source_selected(&sub.filter, source))
        .map(|(index, _)| index)
        .collect();
    indexes.sort_by(|&left, &right| {
        state.subs[right]
            .priority
            .cmp(&state.subs[left].priority)
            .then(state.subs[left].id.cmp(&state.subs[right].id))
    });
    indexes
}

fn plan_delivery(
    state: &BusState,
    indexes: &[usize],
    event: &Event,
    mut free: usize,
    cap: usize,
) -> Option<Vec<Placement>> {
    let seq = event.sequence.get();
    let mut plan = Vec::with_capacity(indexes.len());
    for &index in indexes {
        let (decision, cost) = decide(&state.subs[index], event, seq, free, cap)?;
        free = free.saturating_sub(cost);
        plan.push(Placement { index, decision });
    }
    Some(plan)
}

fn decide(
    sub: &Subscriber,
    event: &Event,
    seq: u64,
    global_free: usize,
    cap: usize,
) -> Option<(Decision, usize)> {
    let pending = sub.pending.get(&event.source);
    if event_selected(&sub.filter, event) {
        let need = usize::from(pending.is_some()) + 1;
        if sub.queue.len() + need <= cap && global_free >= need {
            return Some((
                Decision::Deliver {
                    flush: pending.is_some(),
                },
                need,
            ));
        }
    }
    let reason = if event_selected(&sub.filter, event) {
        GapReason::Lost
    } else {
        GapReason::Filtered
    };
    match pending {
        Some(gap) if gap.reason == reason && gap.last + 1 == seq => Some((
            Decision::Record {
                flush: false,
                reason,
            },
            0,
        )),
        Some(gap) if gap.last + 1 == seq && sub.queue.len() + 1 <= cap && global_free >= 1 => {
            Some((
                Decision::Record {
                    flush: true,
                    reason,
                },
                1,
            ))
        }
        Some(_) => None,
        None if sub.pending.len() < cap => Some((
            Decision::Record {
                flush: false,
                reason,
            },
            0,
        )),
        None => None,
    }
}

fn commit(
    wake: &Condvar,
    mut state: MutexGuard<'_, BusState>,
    plan: Vec<Placement>,
    event: Event,
) -> PublishOutcome {
    let seq = event.sequence.get();
    let admit = state.next_admit;
    state.next_admit += 1;
    let source = event.source.clone();
    let time = event.time.clone();
    let shared = Arc::new(event);
    let mut enqueued = 0;
    let mut lost = 0;
    let mut filtered = 0;
    let mut extra = 0usize;
    for placement in plan {
        let sub = &mut state.subs[placement.index];
        let decision = placement.decision;
        let flush = match &decision {
            Decision::Deliver { flush } | Decision::Record { flush, .. } => *flush,
        };
        if flush && let Some(gap) = sub.pending.remove(&source) {
            let gap_admit = gap.admit;
            sub.queue.push_back(Item {
                admit: gap_admit,
                event: Arc::new(gap_event(&source, &gap)),
            });
            extra += 1;
        }
        match decision {
            Decision::Deliver { .. } => {
                sub.queue.push_back(Item {
                    admit,
                    event: Arc::clone(&shared),
                });
                extra += 1;
                enqueued += 1;
            }
            Decision::Record { reason, .. } => {
                let counts_as_loss = reason == GapReason::Lost;
                match sub.pending.get_mut(&source) {
                    Some(gap) => {
                        gap.last = seq;
                        gap.time = time.clone();
                    }
                    None => {
                        sub.pending.insert(
                            source.clone(),
                            Pending {
                                reason,
                                first: seq,
                                last: seq,
                                admit,
                                time: time.clone(),
                            },
                        );
                    }
                }
                if counts_as_loss {
                    sub.lost += 1;
                    lost += 1;
                } else {
                    sub.filtered += 1;
                    filtered += 1;
                }
            }
        }
    }
    state.global_used += extra;
    state.cursors.insert(source, seq);
    state.accepted += 1;
    state.lost += lost as u64;
    state.filtered += filtered as u64;
    drop(state);
    wake.notify_all();
    PublishOutcome {
        sequence: seq,
        enqueued,
        lost,
        filtered,
    }
}

fn take_batch(state: &mut BusState, id: u64, max: usize) -> Option<Vec<Arc<Event>>> {
    let mut removed = 0usize;
    let batch = {
        let sub = state.subs.iter_mut().find(|sub| sub.id == id)?;
        let mut batch = Vec::new();
        while batch.len() < max {
            let pending = sub
                .pending
                .iter()
                .min_by_key(|(_, gap)| gap.admit)
                .map(|(source, gap)| (source.clone(), gap.admit));
            let queued = sub.queue.front().map(|item| item.admit);
            match (queued, pending) {
                (Some(queued_admit), Some((source, admit))) if admit < queued_admit => {
                    let gap = sub.pending.remove(&source).expect("pending gap");
                    batch.push(Arc::new(gap_event(&source, &gap)));
                }
                (Some(_), _) => {
                    let item = sub.queue.pop_front().expect("queued event");
                    removed += 1;
                    batch.push(item.event);
                }
                (None, Some((source, _))) => {
                    let gap = sub.pending.remove(&source).expect("pending gap");
                    batch.push(Arc::new(gap_event(&source, &gap)));
                }
                (None, None) => break,
            }
        }
        batch
    };
    state.global_used -= removed;
    if batch.is_empty() { None } else { Some(batch) }
}

fn reconcile(cursor: Option<u64>, event: &mut Event) -> Result<(), RuntimeError> {
    event
        .validate()
        .map_err(|_| RuntimeError::new("invalid_record", "event does not satisfy the protocol"))?;
    let seq = event.sequence.get();
    let Some(cursor) = cursor else {
        return Ok(());
    };
    if seq <= cursor {
        return Err(RuntimeError::new(
            "duplicate_sequence",
            "the source sequence was already accepted",
        ));
    }
    if seq == cursor + 1 {
        return if matches!(event.record, Record::Gap { range: Some(_), .. }) {
            Err(RuntimeError::new(
                "missing_record",
                "a contiguous gap marker cannot carry a range",
            ))
        } else {
            Ok(())
        };
    }
    let hole_first = cursor + 1;
    let hole_last = seq - 1;
    match &mut event.record {
        Record::Gap { range, .. } => match range {
            Some(existing)
                if existing.first.get() == hole_first && existing.last.get() == hole_last =>
            {
                Ok(())
            }
            None => {
                *range = Some(SequenceRange {
                    first: Counter::new(hole_first).expect("open hole starts at one or later"),
                    last: Counter::new(hole_last).expect("open hole ends at one or later"),
                });
                Ok(())
            }
            Some(_) => Err(RuntimeError::new(
                "missing_record",
                "the gap range does not match the open source hole",
            )),
        },
        Record::Observation { .. } => Err(RuntimeError::new(
            "missing_record",
            "the source skipped one or more sequences without a gap marker",
        )),
    }
}

/// 单个丢失序号直接占据标记序号。连续多个丢失时，标记使用最后一个序号，
/// 其余序号写入其前置闭区间。这样每个未投递序号都出现在缺口里。
fn gap_event(source: &Source, gap: &Pending) -> Event {
    let range = (gap.first < gap.last).then(|| SequenceRange {
        first: Counter::new(gap.first).expect("loss range start"),
        last: Counter::new(gap.last - 1).expect("loss range end"),
    });
    let event = Event {
        version: Version::CURRENT,
        source: source.clone(),
        sequence: Counter::new(gap.last).expect("loss marker sequence"),
        time: gap.time.clone(),
        record: Record::Gap {
            reason: gap.reason.clone(),
            range,
        },
    };
    debug_assert!(event.validate().is_ok());
    event
}

fn source_selected(filter: &Filter, source: &Source) -> bool {
    filter
        .device
        .as_ref()
        .is_none_or(|device| device == &source.device)
        && filter
            .provider
            .as_ref()
            .is_none_or(|provider| provider == &source.provider)
}

fn event_selected(filter: &Filter, event: &Event) -> bool {
    let Some(expected) = &filter.event_type else {
        return true;
    };
    match &event.record {
        Record::Observation { event_type, .. } => event_type == expected,
        Record::Gap { .. } => true,
    }
}

fn pending_events(pending: &HashMap<Source, Pending>) -> usize {
    pending
        .values()
        .map(|gap| usize::try_from(gap.last - gap.first + 1).unwrap_or(usize::MAX))
        .sum()
}
