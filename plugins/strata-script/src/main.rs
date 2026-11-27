#![deny(unsafe_code)]

//! StrataScript 工作台进程。
//!
//! 宿主用现有帧协议启动它。源码、调试和触发都留在插件私有目录。
//! 预置目录保持为空。禁用和停止会先结束脚本子进程，再以 0 退出。

mod bench;
mod codec;
mod contain;
mod editor;
mod execute;

use std::env;
use std::fs;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use winstrata_protocols::{
    Manifest, PluginFrame, PluginKind, Startup, Version, read_plugin_frame, write_plugin_frame,
};

use bench::Bench;
use execute::Tools;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::from(1),
    }
}

struct Paths {
    python: Option<PathBuf>,
    python_prefix: Option<PathBuf>,
    node: Option<PathBuf>,
    node_prefix: Option<PathBuf>,
    shim: Option<PathBuf>,
}

fn run() -> io::Result<u8> {
    let mut manifest_path = None;
    let mut state_path = None;
    let mut paths = Paths {
        python: None,
        python_prefix: None,
        node: None,
        node_prefix: None,
        shim: None,
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--manifest" => {
                manifest_path = args.next();
                continue;
            }
            "--state" => {
                state_path = args.next();
                continue;
            }
            "--python" => &mut paths.python,
            "--python-prefix" => &mut paths.python_prefix,
            "--node" => &mut paths.node,
            "--node-prefix" => &mut paths.node_prefix,
            "--shim" => &mut paths.shim,
            _ => return Ok(1),
        };
        *value = args.next().map(PathBuf::from);
    }
    let manifest_path =
        PathBuf::from(manifest_path.ok_or_else(|| io::Error::other("missing manifest"))?);
    let state_path = PathBuf::from(state_path.ok_or_else(|| io::Error::other("missing state"))?);
    if !manifest_path.is_absolute() || !state_path.is_absolute() {
        return Ok(1);
    }
    let manifest_path = fs::canonicalize(&manifest_path)?;
    let state_path = fs::canonicalize(&state_path)?;
    if manifest_path.file_name().and_then(|name| name.to_str()) != Some("manifest.json")
        || !state_path.is_dir()
    {
        return Ok(1);
    }
    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest =
        Manifest::from_json(&manifest_text).map_err(|_| io::Error::other("invalid manifest"))?;
    if manifest.id.as_str() != "strata.script"
        || manifest.kind != PluginKind::Builtin
        || manifest.runtime.as_str() != "native"
    {
        return Ok(1);
    }
    if declined(&manifest_path) {
        return Ok(3);
    }
    let tools = Tools {
        python: absolute_file(paths.python)?,
        python_prefix: absolute_dir(paths.python_prefix)?,
        node: absolute_file(paths.node)?,
        node_prefix: absolute_dir(paths.node_prefix)?,
        shim: absolute_file(paths.shim)?,
    };
    let session = Session::start(&state_path, tools)?;
    let code = exchange(&manifest, &state_path, &session)?;
    session.release()?;
    Ok(code)
}

struct Session {
    bench: Arc<Mutex<Bench>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    state: PathBuf,
}

impl Session {
    fn start(state: &Path, tools: Tools) -> io::Result<Self> {
        let bench = Arc::new(Mutex::new(Bench::open(state, tools)?));
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let shared = Arc::clone(&bench);
        let thread = thread::Builder::new()
            .name("ws-script".to_owned())
            .spawn(move || {
                while !flag.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(50));
                    if flag.load(Ordering::Acquire) {
                        break;
                    }
                    let mut bench = shared.lock().unwrap_or_else(|error| error.into_inner());
                    bench.fire_due();
                }
            })?;
        Ok(Self {
            bench,
            stop,
            thread: Some(thread),
            state: state.to_path_buf(),
        })
    }

    fn invoke(&self, payload: &str) -> Result<String, &'static str> {
        let mut bench = self.bench.lock().unwrap_or_else(|error| error.into_inner());
        let value = bench.dispatch(payload);
        bench.fit(&value)
    }

    fn pause(&self) {
        self.bench
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pause();
    }

    fn resume(&self) {
        self.bench
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .resume();
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.bench
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .kill_child();
    }

    fn release(mut self) -> io::Result<()> {
        self.request_stop();
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| io::Error::other("script scheduler did not join"))?;
        }
        fs::write(self.state.join("released"), b"joined")?;
        Ok(())
    }
}

fn absolute_file(path: Option<PathBuf>) -> io::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.is_absolute() {
        return Err(io::Error::other("toolchain path is not absolute"));
    }
    let path = fs::canonicalize(path)?;
    if path.is_file() {
        Ok(Some(path))
    } else {
        Err(io::Error::other("toolchain path is not a file"))
    }
}

fn absolute_dir(path: Option<PathBuf>) -> io::Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.is_absolute() {
        return Err(io::Error::other("toolchain path is not absolute"));
    }
    let path = fs::canonicalize(path)?;
    if path.is_dir() {
        Ok(Some(path))
    } else {
        Err(io::Error::other("toolchain path is not a directory"))
    }
}

fn exchange(manifest: &Manifest, state: &Path, session: &Session) -> io::Result<u8> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();
    let mut phase = "running".to_owned();
    loop {
        let body = read_plugin_frame(&mut reader)?;
        let request =
            PluginFrame::from_json(&body).map_err(|_| io::Error::other("invalid frame"))?;
        let (response, exit) = handle(manifest, state, session, &mut phase, &request);
        let encoded = response
            .to_json()
            .map_err(|_| io::Error::other("invalid response"))?;
        write_plugin_frame(&mut stdout, &encoded)?;
        stdout.flush()?;
        if let Some(code) = exit {
            return Ok(code);
        }
    }
}

fn declined(manifest_path: &Path) -> bool {
    let path = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("startup.json");
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    match Startup::from_json(&text) {
        Ok(startup) => !startup.ready,
        Err(_) => true,
    }
}

fn handle(
    manifest: &Manifest,
    state: &Path,
    session: &Session,
    phase: &mut String,
    request: &PluginFrame,
) -> (PluginFrame, Option<u8>) {
    let pid = std::process::id();
    match request.op.as_str() {
        "ping" => (reply(manifest, true, "ok", phase, phase, pid), None),
        "pause" => {
            session.pause();
            *phase = "paused".to_owned();
            (reply(manifest, true, "ok", phase, phase, pid), None)
        }
        "resume" => {
            session.resume();
            *phase = "running".to_owned();
            (reply(manifest, true, "ok", phase, phase, pid), None)
        }
        "stop" | "disable" => {
            session.request_stop();
            (reply(manifest, true, "ok", phase, phase, pid), Some(0))
        }
        "fault" => {
            session.request_stop();
            (reply(manifest, true, "ok", "faulted", phase, pid), Some(7))
        }
        "emit" => {
            let event_type = request.event_type.as_deref().unwrap_or("");
            if manifest.events.iter().any(|item| item.as_str() == event_type) {
                (reply(manifest, true, "ok", event_type, phase, pid), None)
            } else {
                (
                    reply(manifest, false, "rejected", "event is not declared", phase, pid),
                    None,
                )
            }
        }
        "invoke" => {
            let capability = request.capability.as_deref().unwrap_or("");
            if capability != "script.workbench"
                || !manifest.capabilities.iter().any(|item| item.as_str() == capability)
            {
                return (
                    reply(manifest, false, "rejected", "capability is not provided", phase, pid),
                    None,
                );
            }
            match session.invoke(request.payload.as_deref().unwrap_or("")) {
                Ok(detail) => (reply(manifest, true, "ok", &detail, phase, pid), None),
                Err(code) => (reply(manifest, false, code, code, phase, pid), None),
            }
        }
        "read_state" => match read_state(state, request.name.as_deref().unwrap_or("")) {
            Ok(text) => (reply(manifest, true, "ok", &text, phase, pid), None),
            Err(code) => (reply(manifest, false, code, code, phase, pid), None),
        },
        _ => (
            reply(manifest, false, "rejected", "operation is not supported", phase, pid),
            None,
        ),
    }
}

fn reply(
    manifest: &Manifest,
    ok: bool,
    code: &str,
    detail: &str,
    phase: &str,
    pid: u32,
) -> PluginFrame {
    PluginFrame {
        version: Version::CURRENT,
        op: "result".to_owned(),
        ok: Some(ok),
        code: Some(code.to_owned()),
        detail: Some(bound(detail)),
        phase: Some(phase.to_owned()),
        capability: None,
        event_type: None,
        name: None,
        payload: None,
        pid: Some(pid),
        plugin_version: Some(manifest.plugin_version),
    }
}

fn bound(value: &str) -> String {
    let mut text: String = value
        .chars()
        .filter(|character| !character.is_control())
        .take(1024)
        .collect();
    if text.is_empty() {
        text.push(' ');
    }
    text
}

fn read_state(state: &Path, name: &str) -> Result<String, &'static str> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 40
        || !bytes[0].is_ascii_lowercase()
        || !bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'.' | b'_' | b'-')
        })
        || name.contains("..")
    {
        return Err("rejected");
    }
    let path = state.join(name);
    if path.parent() != Some(state) {
        return Err("rejected");
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| "missing")?;
    if !metadata.file_type().is_file() || metadata.len() > 1024 {
        return Err("rejected");
    }
    let text = fs::read_to_string(&path).map_err(|_| "missing")?;
    if text.chars().any(char::is_control) {
        return Err("rejected");
    }
    Ok(text)
}
