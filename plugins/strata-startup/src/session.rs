#![forbid(unsafe_code)]

//! 启动项插件的采样线程。
//!
//! 线程只读取启动项和计划任务。禁用时先让线程退出，再写释放标记。
//! 最近一次完整清单留在 inventory.json，下一次启动用它继续归因，而不是把旧项重新记成初次出现。

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use winstrata_protocols::Event;
use winstrata_strata_startup::{Kind, StartupTracker, capture, decode_inventory, encode_inventory};

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
            .name("ws-startup".to_owned())
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

    pub fn release(mut self) -> io::Result<()> {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("startup sampler thread panicked"))?;
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
    let mut tracker = StartupTracker::new();
    match fs::read_to_string(state.join("inventory.json")) {
        Ok(text) if text.is_empty() => tracker.note_unreadable_history(),
        Ok(text) => match decode_inventory(&text) {
            Ok((device, facts)) => tracker.resume(&device, facts),
            Err(_) => tracker.note_unreadable_history(),
        },
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => tracker.note_unreadable_history(),
    }
    while !shared.stop.load(Ordering::Acquire) {
        if shared.paused.load(Ordering::Acquire) {
            interruptible_wait(&shared, SAMPLE_INTERVAL);
            continue;
        }
        match capture().and_then(|snapshot| {
            let summary = summary(&snapshot);
            tracker.observe(&snapshot).map(|events| (events, summary))
        }) {
            Ok((events, summary)) => {
                if let Err(error) = append_events(&state.join("events.jsonl"), &events) {
                    let _ = write_error(&state, &error);
                    break;
                }
                if let Some((device, facts)) = tracker.export() {
                    match encode_inventory(&device, &facts) {
                        Ok(encoded) => {
                            if let Err(error) =
                                write_atomic(&state, "inventory.json", encoded.as_bytes())
                            {
                                let _ = write_error(&state, &error);
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = write_error(&state, &error);
                            break;
                        }
                    }
                }
                let _ = fs::remove_file(state.join("sample-error.txt"));
                if let Err(error) = write_atomic(&state, "state.json", summary.as_bytes()) {
                    let _ = write_error(&state, &error);
                    break;
                }
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

fn summary(snapshot: &winstrata_strata_startup::StartupSnapshot) -> String {
    let startup = snapshot
        .facts
        .iter()
        .filter(|fact| fact.kind == Kind::Startup)
        .count();
    let tasks = snapshot
        .facts
        .iter()
        .filter(|fact| fact.kind == Kind::Task)
        .count();
    format!(
        "{{\"startup\":{startup},\"tasks\":{tasks},\"truncated\":{},\"permission\":{},\"failure\":{}}}",
        snapshot.truncated,
        !snapshot.unreadable_scopes.is_empty(),
        snapshot.source_failure.is_some()
    )
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
