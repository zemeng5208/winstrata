#![forbid(unsafe_code)]

//! 进程插件的采样线程。
//!
//! 线程只调用计划 010 的采样和跟踪。禁用时先让线程退出，再写释放标记。
//! 不读取命令行，不打开 ETW，也不加载驱动。

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use winstrata_core::ProcessTracker;
use winstrata_kernel::capture;
use winstrata_protocols::Event;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);
const MAX_EVENT_BYTES: u64 = 16 * 1024 * 1024;

struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    samples: AtomicU64,
    events: AtomicU64,
}

pub struct Session {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    state: PathBuf,
}

impl Session {
    pub fn start(state: &Path) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            samples: AtomicU64::new(0),
            events: AtomicU64::new(0),
        });
        let worker = Arc::clone(&shared);
        let directory = state.to_path_buf();
        let thread = thread::Builder::new()
            .name("ws-process".to_owned())
            .spawn(move || sample_loop(worker, directory))
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(Self {
            shared,
            thread: Some(thread),
            state: state.to_path_buf(),
        })
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Release);
    }

    pub fn request_stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
    }

    pub fn status(&self) -> String {
        format!(
            "samples={} events={}",
            self.shared.samples.load(Ordering::Acquire),
            self.shared.events.load(Ordering::Acquire)
        )
    }

    /// 停止采样并等待线程结束后才写释放标记。进程此时仍未退出。
    pub fn release(mut self) -> io::Result<()> {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("process sampler thread panicked"))?;
        }
        fs::write(self.state.join("released"), b"joined")?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn sample_loop(shared: Arc<Shared>, state: PathBuf) {
    let mut tracker = ProcessTracker::new();
    while !shared.stop.load(Ordering::Acquire) {
        if shared.paused.load(Ordering::Acquire) {
            interruptible_wait(&shared, SAMPLE_INTERVAL);
            continue;
        }
        match capture().and_then(|snapshot| tracker.observe(&snapshot)) {
            Ok(events) => {
                if let Err(error) = write_sample(&state, &tracker, &events) {
                    let _ = write_error(&state, &error);
                    break;
                }
                let _ = fs::remove_file(state.join("sample-error.txt"));
                let samples = shared.samples.fetch_add(1, Ordering::Release) + 1;
                shared.events.fetch_add(
                    u64::try_from(events.len()).unwrap_or(u64::MAX),
                    Ordering::Release,
                );
                if let Err(error) = write_atomic(&state, "samples", samples.to_string().as_bytes())
                {
                    let _ = write_error(&state, &error);
                    break;
                }
            }
            Err(error) => {
                let _ = write_error(&state, &error);
            }
        }
        interruptible_wait(&shared, SAMPLE_INTERVAL);
    }
}

fn interruptible_wait(shared: &Shared, duration: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < duration {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn write_sample(state: &Path, tracker: &ProcessTracker, events: &[Event]) -> io::Result<()> {
    append_events(&state.join("events.jsonl"), events)?;
    write_atomic(state, "tree.json", &tree_json(tracker)?.into_bytes())?;
    Ok(())
}

fn append_events(path: &Path, events: &[Event]) -> io::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    rotate_if_full(path)?;
    let mut file = open_private(path, true)?;
    for event in events {
        let line = event
            .to_json()
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    Ok(())
}

fn rotate_if_full(path: &Path) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_EVENT_BYTES {
        return Ok(());
    }
    let previous = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("events.prev.jsonl");
    let _ = fs::remove_file(&previous);
    fs::rename(path, previous)?;
    Ok(())
}

fn tree_json(tracker: &ProcessTracker) -> io::Result<String> {
    let nodes = tracker
        .tree()
        .into_iter()
        .map(|node| {
            serde_json::json!({
                "pid": node.pid,
                "start_unix_ns": node.start_unix_ns.to_string(),
                "parent_pid": node.parent_pid,
                "parent_start_unix_ns": node.parent_start_unix_ns.map(|value| value.to_string()),
                "name": node.name,
                "image": node.image,
                "services": node.services,
                "threads": node.threads,
                "modules": node.modules.into_iter().map(|module| serde_json::json!({
                    "name": module.name,
                    "base": module.base.to_string(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&serde_json::json!({ "nodes": nodes }))
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))
}

fn write_error(state: &Path, error: &io::Error) -> io::Result<()> {
    let mut text: String = error
        .to_string()
        .chars()
        .filter(|character| !character.is_control())
        .take(240)
        .collect();
    if text.is_empty() {
        text.push_str("sample failed");
    }
    write_atomic(state, "sample-error.txt", text.as_bytes())
}

fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let temporary = dir.join(format!("{name}.tmp"));
    {
        let mut file = open_private(&temporary, false)?;
        file.write_all(bytes)?;
        file.flush()?;
    }
    let destination = dir.join(name);
    let _ = fs::remove_file(&destination);
    fs::rename(&temporary, &destination)?;
    Ok(())
}

fn open_private(path: &Path, append: bool) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
