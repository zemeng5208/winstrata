#![forbid(unsafe_code)]

//! StrataTag 标记进程。
//!
//! 宿主用现有帧协议启动它。标记判定和报文落盘由宿主完成。
//! 这个进程把判定写入私有目录。禁用时写释放标记并以 0 退出。

mod session;

use std::env;
use std::fs;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use winstrata_protocols::{
    read_plugin_frame, write_plugin_frame, Manifest, PluginFrame, PluginKind, Startup, Version,
};

use session::Session;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::from(1),
    }
}

fn run() -> io::Result<u8> {
    let mut manifest_path = None;
    let mut state_path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest_path = args.next(),
            "--state" => state_path = args.next(),
            _ => return Ok(1),
        }
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
    if manifest.id.as_str() != "strata.tag"
        || manifest.kind != PluginKind::Builtin
        || manifest.runtime.as_str() != "native"
    {
        return Ok(1);
    }
    if declined(&manifest_path) {
        return Ok(3);
    }
    let session = Session::start(&state_path);
    let code = exchange(&manifest, &state_path, &session)?;
    session.release()?;
    Ok(code)
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
        "stop" | "disable" => (reply(manifest, true, "ok", phase, phase, pid), Some(0)),
        "fault" => (reply(manifest, true, "ok", "faulted", phase, pid), Some(7)),
        "emit" => {
            let event_type = request.event_type.as_deref().unwrap_or("");
            if manifest
                .events
                .iter()
                .any(|item| item.as_str() == event_type)
            {
                (reply(manifest, true, "ok", event_type, phase, pid), None)
            } else {
                (
                    reply(
                        manifest,
                        false,
                        "rejected",
                        "event is not declared",
                        phase,
                        pid,
                    ),
                    None,
                )
            }
        }
        "invoke" => {
            let capability = request.capability.as_deref().unwrap_or("");
            if !manifest
                .capabilities
                .iter()
                .any(|item| item.as_str() == capability)
            {
                return (
                    reply(
                        manifest,
                        false,
                        "rejected",
                        "capability is not provided",
                        phase,
                        pid,
                    ),
                    None,
                );
            }
            let payload = request.payload.as_deref().unwrap_or("");
            let result = match capability {
                "tag.apply" => session.apply(payload),
                "tag.policy" => session.policy(payload),
                "tag.query" => session.query(payload),
                _ => Err("rejected"),
            };
            match result {
                Ok(detail) => (reply(manifest, true, "ok", &detail, phase, pid), None),
                Err(code) => (reply(manifest, false, code, code, phase, pid), None),
            }
        }
        "read_state" => match read_state(state, request.name.as_deref().unwrap_or("")) {
            Ok(text) => (reply(manifest, true, "ok", &text, phase, pid), None),
            Err(code) => (reply(manifest, false, code, code, phase, pid), None),
        },
        _ => (
            reply(
                manifest,
                false,
                "rejected",
                "operation is not supported",
                phase,
                pid,
            ),
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
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b'-')
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
