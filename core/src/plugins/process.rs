//! 每个插件一个子进程。监督线程只转发帧，不占用 Core 的共享工作者。

use std::io::{BufReader, ErrorKind};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub(super) fn native_command(runtime: &Path, manifest: &Path, state: &Path) -> Command {
    let mut command = Command::new(runtime);
    command
        .arg("--manifest")
        .arg(manifest)
        .arg("--state")
        .arg(state)
        .current_dir(state)
        .stderr(Stdio::null());
    // 绝对路径启动，不继承调用方环境，避免把其他插件的路径传进进程。
    // Windows 进程仍需要系统环境才能完成加载。
    #[cfg(unix)]
    command.env_clear();
    command
}
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use winstrata_protocols::{PluginFrame, PluginVersion, read_plugin_frame, write_plugin_frame};

use super::PluginError;

const EXCHANGE_WAIT: Duration = Duration::from_secs(3);

struct Job {
    body: String,
    reply: std::sync::mpsc::Sender<std::io::Result<String>>,
}

pub(super) trait Reap {
    fn id(&self) -> u32;
    fn try_wait(&mut self) -> std::io::Result<Option<i32>>;
    fn kill(&mut self);
    fn wait(&mut self) -> std::io::Result<i32>;
}

struct StdProc(Child);

impl Reap for StdProc {
    fn id(&self) -> u32 {
        self.0.id()
    }

    fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
        Ok(self.0.try_wait()?.map(|status| status.code().unwrap_or(1)))
    }

    fn kill(&mut self) {
        let _ = self.0.kill();
    }

    fn wait(&mut self) -> std::io::Result<i32> {
        Ok(self.0.wait()?.code().unwrap_or(1))
    }
}

pub(super) struct Live {
    proc: Box<dyn Reap>,
    jobs: std::sync::mpsc::Sender<Job>,
    thread: Option<JoinHandle<()>>,
    pub(super) version: PluginVersion,
    pub(super) subscriptions: Vec<crate::bus::Subscription>,
    pub(super) sequence: u64,
    pub(super) paused: bool,
}

impl Live {
    pub(super) fn spawn(mut command: Command, version: PluginVersion) -> Result<Self, PluginError> {
        command.stdin(Stdio::piped()).stdout(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            PluginError::new(
                "runtime_missing",
                format!("plugin runtime did not start: {error}"),
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginError::new("runtime_missing", "plugin runtime has no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginError::new("runtime_missing", "plugin runtime has no stdout"))?;
        Self::from_pipes(Box::new(StdProc(child)), stdin, stdout, version)
    }

    pub(super) fn from_pipes(
        proc: Box<dyn Reap>,
        stdin: impl std::io::Write + Send + 'static,
        stdout: impl std::io::Read + Send + 'static,
        version: PluginVersion,
    ) -> Result<Self, PluginError> {
        let (jobs, incoming) = std::sync::mpsc::channel();
        let thread = thread::Builder::new()
            .name("ws-plugin".to_owned())
            .spawn(move || supervise(stdin, BufReader::new(stdout), incoming))
            .map_err(|error| PluginError::new("runtime_missing", error.to_string()))?;
        Ok(Self {
            proc,
            jobs,
            thread: Some(thread),
            version,
            subscriptions: Vec::new(),
            sequence: 0,
            paused: false,
        })
    }

    pub(super) fn pid(&self) -> u32 {
        self.proc.id()
    }

    pub(super) fn exchange(&mut self, frame: &PluginFrame) -> Result<PluginFrame, PluginError> {
        let body = frame
            .to_json()
            .map_err(|error| PluginError::new("invalid_input", error.to_string()))?;
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.jobs
            .send(Job {
                body,
                reply: reply_tx,
            })
            .map_err(|_| PluginError::new("faulted", "plugin supervisor stopped"))?;
        match reply_rx.recv_timeout(EXCHANGE_WAIT) {
            Ok(Ok(body)) => PluginFrame::from_json(&body)
                .map_err(|error| PluginError::new("invalid_record", error.to_string())),
            Ok(Err(error)) => Err(Self::io_code(error)),
            Err(_) => Err(PluginError::new("unhealthy", "plugin timed out")),
        }
    }

    pub(super) fn wait_exit(&mut self) -> Result<i32, PluginError> {
        let started = Instant::now();
        loop {
            match self.proc.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if started.elapsed() > EXCHANGE_WAIT => {
                    self.proc.kill();
                    return self.proc.wait().map_err(Self::io_code);
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(Self::io_code(error)),
            }
        }
    }

    pub(super) fn finish(mut self) {
        drop(self.jobs);
        self.proc.kill();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = self.proc.wait();
    }

    fn io_code(error: std::io::Error) -> PluginError {
        let code = if error.kind() == ErrorKind::UnexpectedEof {
            "unavailable"
        } else {
            "faulted"
        };
        PluginError::new(code, error.to_string())
    }
}

fn supervise(
    mut stdin: impl std::io::Write,
    mut stdout: impl std::io::BufRead,
    incoming: std::sync::mpsc::Receiver<Job>,
) {
    while let Ok(job) = incoming.recv() {
        let result =
            write_plugin_frame(&mut stdin, &job.body).and_then(|()| read_plugin_frame(&mut stdout));
        if job.reply.send(result).is_err() {
            break;
        }
    }
}
