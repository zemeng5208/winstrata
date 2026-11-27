//! 用真实解释器运行和调试用户脚本。
//!
//! 调试辅助文件写在私有 debug 目录，不是预置脚本。子进程的标准输出不接插件帧通道。

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::contain;

pub(crate) const RUN_LIMIT: Duration = Duration::from_millis(1200);
const DEBUG_WAIT: Duration = Duration::from_millis(1000);

pub(crate) struct Tools {
    pub python: Option<PathBuf>,
    pub python_prefix: Option<PathBuf>,
    pub node: Option<PathBuf>,
    pub node_prefix: Option<PathBuf>,
    pub shim: Option<PathBuf>,
}

pub(crate) struct ChildIo {
    pub child: Child,
    pub output: PathBuf,
    pub error: PathBuf,
}

pub(crate) struct Captured {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub(crate) enum Pause {
    Line(u32),
    Exited(i32),
}

pub(crate) fn run_source(
    tools: &Tools,
    state: &Path,
    language: &str,
    source: &Path,
    modules: &[String],
    allow_private: bool,
) -> Result<Captured, String> {
    if !modules.is_empty() {
        module_check(tools, state, language, modules)?;
    }
    let mut child = spawn_command(
        tools,
        state,
        language,
        direct_command(tools, language, source)?,
        allow_private,
        false,
    )?;
    match wait_child(&mut child.child, RUN_LIMIT) {
        Ok(Some(code)) => Ok(read_captured(&child, code)),
        Ok(None) => Err("timeout".to_owned()),
        Err(_) => Err("faulted".to_owned()),
    }
}

pub(crate) fn spawn_debug(
    tools: &Tools,
    state: &Path,
    language: &str,
    source: &Path,
    line: u32,
    allow_private: bool,
) -> Result<ChildIo, String> {
    prepare_debug(state, language, source, line)?;
    let command = debug_command(tools, state, language)?;
    spawn_command(tools, state, language, command, allow_private, true)
}

pub(crate) fn wait_pause(state: &Path, child: &mut Child) -> Result<Pause, String> {
    let pause = state.join("debug").join("pause.json");
    let started = Instant::now();
    let mut saw_partial = false;
    loop {
        if pause.is_file() {
            match read_pause_line(&pause) {
                Some(line) => return Ok(Pause::Line(line)),
                None => saw_partial = true,
            }
        }
        if let Some(status) = child.try_wait().map_err(|_| "faulted".to_owned())? {
            return Ok(Pause::Exited(status.code().unwrap_or(1)));
        }
        if started.elapsed() > DEBUG_WAIT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if saw_partial { "faulted" } else { "not_paused" }.to_owned());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_pause_line(pause: &Path) -> Option<u32> {
    let text = fs::read_to_string(pause).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let line = value["line"].as_u64()? as u32;
    let _ = fs::remove_file(pause);
    Some(line)
}

pub(crate) fn signal_debug(state: &Path, action: &str) -> io::Result<()> {
    fs::write(state.join("debug").join("control"), action)
}

pub(crate) fn read_captured(child: &ChildIo, code: i32) -> Captured {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Ok(mut file) = File::open(&child.output) {
        let _ = file.read_to_end(&mut stdout);
    }
    if let Ok(mut file) = File::open(&child.error) {
        let _ = file.read_to_end(&mut stderr);
    }
    stdout.truncate(4096);
    stderr.truncate(4096);
    Captured { code, stdout, stderr }
}

fn module_check(tools: &Tools, state: &Path, language: &str, modules: &[String]) -> Result<(), String> {
    for name in modules {
        if !module_token(name) {
            return Err("dependency_missing".to_owned());
        }
        let probe = if language == "python" {
            format!("import {name}")
        } else {
            format!("require({name:?})")
        };
        let mut command = if language == "python" {
            let python = tools.python.as_ref().ok_or("toolchain_missing")?;
            let mut command = Command::new(python);
            command.arg("-I").arg("-B").arg("-c").arg(probe);
            command
        } else {
            let node = tools.node.as_ref().ok_or("toolchain_missing")?;
            let mut command = Command::new(node);
            command.arg("-e").arg(probe);
            command
        };
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = spawn_command(tools, state, language, command, false, false)?;
        match wait_child(&mut child.child, RUN_LIMIT) {
            Ok(Some(0)) => {}
            Ok(Some(_)) | Ok(None) => return Err("dependency_missing".to_owned()),
            Err(_) => return Err("faulted".to_owned()),
        }
    }
    Ok(())
}

fn spawn_command(
    tools: &Tools,
    state: &Path,
    language: &str,
    mut command: Command,
    allow_private: bool,
    debug: bool,
) -> Result<ChildIo, String> {
    let private = state.join("private");
    let debug_dir = state.join("debug");
    fs::create_dir_all(&private).map_err(|_| "faulted")?;
    fs::create_dir_all(&debug_dir).map_err(|_| "faulted")?;
    let output = debug_dir.join("stdout.txt");
    let error = debug_dir.join("stderr.txt");
    if command.get_program().is_empty() {
        return Err("toolchain_missing".to_owned());
    }
    // 模块探测已经把标准流设成 null。其余启动在这里接私有日志，避免写入插件帧。
    if debug || output_unset(&command) {
        let stdout = File::create(&output).map_err(|_| "faulted")?;
        let stderr = File::create(&error).map_err(|_| "faulted")?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
    }
    command.current_dir(&private);
    // Unix 上清掉插件环境，避免把宿主路径传进用户脚本。Windows 仍需要系统环境才能启动解释器。
    #[cfg(unix)]
    command.env_clear().env("LC_ALL", "C.UTF-8");
    let mut read = vec![state.to_path_buf()];
    if let Some(prefix) = if language == "python" {
        tools.python_prefix.clone()
    } else {
        tools.node_prefix.clone()
    } {
        read.push(prefix);
    }
    // 解释器本体可能在前缀目录之外。目录规则才带执行权限。
    for program in [&tools.python, &tools.node] {
        if let Some(directory) = program.as_ref().and_then(|path| path.parent()) {
            read.push(directory.to_path_buf());
        }
    }
    for optional in [
        "/lib",
        "/lib64",
        "/usr/lib",
        "/usr/lib64",
        "/lib/x86_64-linux-gnu",
        "/usr/lib/x86_64-linux-gnu",
        "/dev/urandom",
        "/dev/null",
        "/etc/ld.so.cache",
        "/etc/ssl",
    ] {
        let path = PathBuf::from(optional);
        if path.exists() {
            read.push(path);
        }
    }
    let mut write = Vec::new();
    if debug {
        write.push(debug_dir);
    }
    if allow_private {
        write.push(private);
    }
    let child = contain::spawn_contained(&mut command, &read, &write, tools.shim.as_deref()).map_err(
        |error| {
            if error.to_string().contains("isolation_unavailable") {
                "isolation_unavailable".to_owned()
            } else {
                "faulted".to_owned()
            }
        },
    )?;
    Ok(ChildIo { child, output, error })
}

fn output_unset(command: &Command) -> bool {
    let _ = command;
    true
}

fn direct_command(tools: &Tools, language: &str, source: &Path) -> Result<Command, String> {
    if language == "python" {
        let python = tools.python.as_ref().ok_or("toolchain_missing")?;
        let mut command = Command::new(python);
        command.arg("-I").arg("-B").arg(source);
        Ok(command)
    } else {
        let node = tools.node.as_ref().ok_or("toolchain_missing")?;
        let mut command = Command::new(node);
        if language == "typescript" {
            command.arg("--experimental-strip-types");
        }
        command.arg(source);
        Ok(command)
    }
}

fn debug_command(tools: &Tools, state: &Path, language: &str) -> Result<Command, String> {
    let job = state.join("debug").join("job.json");
    if language == "python" {
        let python = tools.python.as_ref().ok_or("toolchain_missing")?;
        let mut command = Command::new(python);
        command
            .arg("-I")
            .arg("-B")
            .arg(state.join("debug").join("debug.py"))
            .arg(job);
        Ok(command)
    } else {
        let node = tools.node.as_ref().ok_or("toolchain_missing")?;
        let mut command = Command::new(node);
        command
            .arg("--experimental-strip-types")
            .arg(state.join("debug").join("debug.mjs"))
            .arg(job);
        Ok(command)
    }
}

fn prepare_debug(state: &Path, language: &str, source: &Path, line: u32) -> Result<(), String> {
    let debug = state.join("debug");
    fs::create_dir_all(&debug).map_err(|_| "faulted")?;
    for name in ["pause.json", "control", "stop", "stdout.txt", "stderr.txt"] {
        let _ = fs::remove_file(debug.join(name));
    }
    let wrapped = debug.join(if language == "typescript" {
        "wrapped.ts"
    } else {
        "wrapped.mjs"
    });
    if language != "python" {
        let source_text = fs::read_to_string(source).map_err(|_| "faulted")?;
        let lines: Vec<String> = source_text
            .split('\n')
            .enumerate()
            .map(|(index, line)| format!("await __ws_line({}); {line}", index + 1))
            .collect();
        fs::write(&wrapped, lines.join("\n") + "\n").map_err(|_| "faulted")?;
    }
    let job = json!({
        "source": source,
        "wrapped": wrapped,
        "breaks": [line],
        "pause": debug.join("pause.json"),
        "control": debug.join("control"),
        "stop": debug.join("stop"),
    });
    fs::write(debug.join("job.json"), serde_json::to_vec(&job).map_err(|_| "faulted")?)
        .map_err(|_| "faulted")?;
    fs::write(debug.join("debug.py"), PYTHON_DEBUG).map_err(|_| "faulted")?;
    fs::write(debug.join("debug.mjs"), JS_DEBUG).map_err(|_| "faulted")?;
    Ok(())
}

fn wait_child(child: &mut Child, limit: Duration) -> io::Result<Option<i32>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status.code().unwrap_or(1)));
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn module_token(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

const PYTHON_DEBUG: &str = r#"import json, os, sys, time
job = json.load(open(sys.argv[1], encoding="utf-8"))
target = job["source"]
breaks = set(job["breaks"])
step = False

def tracer(frame, event, arg):
    global step
    if event != "line" or frame.f_code.co_filename != target:
        return tracer
    line = frame.f_lineno
    if line in breaks or step:
        step = False
        with open(job["pause"], "w", encoding="utf-8") as handle:
            json.dump({"line": line}, handle)
        while True:
            if os.path.exists(job["stop"]):
                raise SystemExit(0)
            if os.path.exists(job["control"]):
                text = open(job["control"], encoding="utf-8").read().strip()
                os.remove(job["control"])
                if text == "step":
                    step = True
                    return tracer
                if text == "continue":
                    return tracer
                raise SystemExit(0)
            time.sleep(0.02)
    return tracer

sys.settrace(tracer)
source = open(target, encoding="utf-8").read()
exec(compile(source, target, "exec"), {"__name__": "__main__"})
"#;

const JS_DEBUG: &str = r#"import fs from 'node:fs'
import { pathToFileURL } from 'node:url'
const job = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const breaks = new Set(job.breaks)
let step = false
globalThis.__ws_line = (line) => {
  if (!breaks.has(line) && !step) return
  step = false
  fs.writeFileSync(job.pause, JSON.stringify({ line }))
  for (;;) {
    if (fs.existsSync(job.stop)) process.exit(0)
    if (fs.existsSync(job.control)) {
      const text = fs.readFileSync(job.control, 'utf8').trim()
      fs.rmSync(job.control)
      if (text === 'step') { step = true; return }
      if (text === 'continue') return
      process.exit(0)
    }
    const until = Date.now() + 20
    while (Date.now() < until) {}
  }
}
await import(pathToFileURL(job.wrapped).href)
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_pause_file_is_read_after_it_is_complete() {
        let root = std::env::temp_dir().join(format!(
            "winstrata-pause-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("debug")).unwrap();
        let pause = root.join("debug").join("pause.json");
        fs::write(&pause, b"").unwrap();
        let mut child = linger();
        let state = root.clone();
        let reader = thread::spawn(move || {
            let result = wait_pause(&state, &mut child);
            let _ = child.kill();
            let _ = child.wait();
            result
        });
        thread::sleep(Duration::from_millis(60));
        fs::write(&pause, br#"{"line":2}"#).unwrap();
        let pause_result = reader.join().unwrap();
        let _ = fs::remove_dir_all(&root);
        match pause_result {
            Ok(Pause::Line(line)) => assert_eq!(line, 2),
            Ok(Pause::Exited(code)) => panic!("child exited {code}"),
            Err(code) => panic!("{code}"),
        }
    }

    fn linger() -> Child {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("ping");
            command.args(["-n", "6", "127.0.0.1"]);
            command
        } else {
            let mut command = Command::new("sleep");
            command.arg("3");
            command
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn().expect("linger")
    }
}
