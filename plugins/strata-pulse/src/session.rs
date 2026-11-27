//! 脉冲插件的采样线程。
//!
//! 线程只读取计数器并保留历史。禁用时先让线程退出，再写释放标记。
//! 不新增帧操作，也不发布会把采集进程当成被观测对象的 `emit`。

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use winstrata_core::{Batch, Core, Point};
use winstrata_protocols::Id;

use crate::platform::Sampler;
use crate::sample::{Sample, Status, sample_document, update_peaks};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(200);
const MAX_HISTORY_BYTES: u64 = 16 * 1024 * 1024;

struct Shared {
    stop: AtomicBool,
    paused: AtomicBool,
    samples: AtomicU64,
    measured: AtomicU64,
    unsupported: AtomicU64,
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
            measured: AtomicU64::new(0),
            unsupported: AtomicU64::new(0),
        });
        let worker = Arc::clone(&shared);
        let directory = state.to_path_buf();
        let thread = thread::Builder::new()
            .name("ws-pulse".to_owned())
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
            "samples={} measured={} unsupported={}",
            self.shared.samples.load(Ordering::Acquire),
            self.shared.measured.load(Ordering::Acquire),
            self.shared.unsupported.load(Ordering::Acquire)
        )
    }

    /// 停止采样并等待线程结束后才写释放标记。进程此时仍未退出。
    pub fn release(mut self) -> io::Result<()> {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("pulse sampler thread panicked"))?;
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
    let mut sampler = Sampler::new();
    let mut peaks = load_peaks(&state);
    while !shared.stop.load(Ordering::Acquire) {
        if shared.paused.load(Ordering::Acquire) {
            interruptible_wait(&shared, SAMPLE_INTERVAL);
            continue;
        }
        let sample = sampler.sample();
        if sample.unix_ns < 0 {
            let _ = write_error(&state, &io::Error::other("clock is not available"));
            interruptible_wait(&shared, SAMPLE_INTERVAL);
            continue;
        }
        let measured = sample
            .readings
            .iter()
            .filter(|reading| reading.status == Status::Measured)
            .count();
        let unsupported = sample
            .readings
            .iter()
            .filter(|reading| reading.status == Status::Unsupported)
            .count();
        update_peaks(&mut peaks, &sample.readings);
        let store = store_metrics(&state, &sample);
        let ordinal = shared.samples.load(Ordering::Acquire) + 1;
        let document = sample_document(&sample, &peaks, store, ordinal);
        if let Err(error) = write_sample(&state, &document, &peaks, ordinal) {
            let _ = write_error(&state, &error);
            break;
        }
        let _ = fs::remove_file(state.join("sample-error.txt"));
        shared.samples.fetch_add(1, Ordering::Release);
        shared.measured.store(
            u64::try_from(measured).unwrap_or(u64::MAX),
            Ordering::Release,
        );
        shared.unsupported.store(
            u64::try_from(unsupported).unwrap_or(u64::MAX),
            Ordering::Release,
        );
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

fn store_metrics(state: &Path, sample: &Sample) -> &'static str {
    let mut batch = Batch::new();
    let mut any = false;
    for reading in &sample.readings {
        if reading.status != Status::Measured {
            continue;
        }
        let Some(value) = reading.value else {
            continue;
        };
        let Ok(series) = Id::new(format!("pulse.{}", reading.id)) else {
            return "failed";
        };
        let point = Point {
            unix_ns: sample.unix_ns,
            value,
        };
        if batch.append_metrics(series, vec![point]).is_err() {
            return "failed";
        }
        any = true;
    }
    if !any {
        return "private_only";
    }
    match Core::commit_plugin_metrics(state, batch) {
        Ok(_) => "committed",
        Err(error)
            if error.kind() == ErrorKind::PermissionDenied
                && error.to_string() == "storage is locked" =>
        {
            "locked"
        }
        Err(_) => "failed",
    }
}

fn write_sample(
    state: &Path,
    document: &str,
    peaks: &BTreeMap<String, i64>,
    ordinal: u64,
) -> io::Result<()> {
    append_line(&state.join("history.jsonl"), document)?;
    write_atomic(state, "latest.json", document.as_bytes())?;
    let peaks_text = serde_json::to_string(peaks)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))?;
    write_atomic(state, "peaks.json", peaks_text.as_bytes())?;
    write_atomic(state, "samples", ordinal.to_string().as_bytes())?;
    Ok(())
}

fn load_peaks(state: &Path) -> BTreeMap<String, i64> {
    let Ok(text) = fs::read_to_string(state.join("peaks.json")) else {
        return BTreeMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    rotate_if_full(path)?;
    let mut file = open_private(path, true)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn rotate_if_full(path: &Path) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_HISTORY_BYTES {
        return Ok(());
    }
    let previous = path
        .parent()
        .unwrap_or(Path::new("."))
        .join("history.prev.jsonl");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_commit_rejects_a_directory_outside_plugin_state() {
        let path =
            std::env::temp_dir().join(format!("winstrata-pulse-reject-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        let error = Core::commit_plugin_metrics(&path, Batch::new()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput);
        let _ = fs::remove_dir_all(&path);
    }
}
