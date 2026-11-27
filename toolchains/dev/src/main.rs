use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod cross_probe;

const USAGE: &str = "Usage: cargo dev <doctor|init|build|verify|cross-probe>";

#[derive(Clone, Copy)]
enum Task {
    Doctor,
    Init,
    Build,
    Verify,
    CrossProbe,
}

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    let task = match arguments.as_slice() {
        [value] if value == "--help" || value == "-h" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [value] if value == "doctor" => Task::Doctor,
        [value] if value == "init" => Task::Init,
        [value] if value == "build" => Task::Build,
        [value] if value == "verify" => Task::Verify,
        [value] if value == "cross-probe" => Task::CrossProbe,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match workspace_root().and_then(|root| execute(&root, task)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("winstrata-dev: {error}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    // 由当前目录向上查找仓库根目录，避免将编译时路径写入开发工具。
    let current = env::current_dir().map_err(|error| format!("read working directory: {error}"))?;
    current
        .ancestors()
        .find(|path| {
            path.join("rust-toolchain.toml").is_file()
                && path.join("pnpm-workspace.yaml").is_file()
                && path.join("toolchains/dev/Cargo.toml").is_file()
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("WinStrata workspace not found above {}", current.display()))
}

fn execute(root: &Path, task: Task) -> Result<(), String> {
    let pnpm = pnpm_program();
    doctor(root, &pnpm)?;

    match task {
        Task::Doctor => Ok(()),
        Task::Init => {
            run(root, "cargo", &["fetch", "--locked"])?;
            run(root, &pnpm, &["install", "--frozen-lockfile"])
        }
        Task::Build => run(root, "cargo", &["build", "--workspace", "--locked"]),
        Task::Verify => {
            run(root, "cargo", &["test", "--workspace", "--locked"])?;
            run(
                root,
                &pnpm,
                &["install", "--frozen-lockfile", "--lockfile-only"],
            )
        }
        Task::CrossProbe => cross_probe::run_probe(root),
    }
}

fn doctor(root: &Path, pnpm: &OsStr) -> Result<(), String> {
    println!("Workspace: {}", root.display());
    let rust_pin = rust_channel(root)?;
    for program in ["rustc", "cargo"] {
        let version = output(root, program, &["--version"])?;
        let actual = version
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| format!("cannot read {program} version: {version}"))?;
        check_version(program, &rust_pin, actual)?;
    }

    // 用 Node 自带的 JSON 解析读取版本声明，避免额外的开发依赖。
    let version_report = output(
        root,
        "node",
        &[
            "--eval",
            r#"
const { engines, packageManager } = require('./package.json');
console.log(engines.node);
console.log(process.versions.node);
console.log(packageManager);
"#,
        ],
    )?;
    let versions: Vec<_> = version_report.lines().collect();
    let [node_pin, active_node, manager] = versions.as_slice() else {
        return Err("package.json must declare engines.node and packageManager".into());
    };
    check_version("Node.js", node_pin, active_node)?;
    let expected_pnpm = manager
        .strip_prefix("pnpm@")
        .ok_or_else(|| "package.json packageManager must pin pnpm".to_string())?;
    let actual_pnpm = output(root, pnpm, &["--version"])?;
    check_version("pnpm", expected_pnpm, &actual_pnpm)
}

fn rust_channel(root: &Path) -> Result<String, String> {
    let file = root.join("rust-toolchain.toml");
    let contents =
        fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(name, _)| name.trim() == "channel")
        .and_then(|(_, value)| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} must declare a channel", file.display()))
}

fn check_version(tool: &str, expected: &str, actual: &str) -> Result<(), String> {
    if expected != actual {
        return Err(format!("{tool} {expected} is required, found {actual}"));
    }
    println!("{tool}: {actual}");
    Ok(())
}

#[cfg(not(windows))]
fn pnpm_program() -> OsString {
    "pnpm".into()
}

#[cfg(windows)]
fn pnpm_program() -> OsString {
    // npm 和 Corepack 在 Windows 上可能只提供 .cmd 启动器。
    if let Some(paths) = env::var_os("PATH") {
        for directory in env::split_paths(&paths) {
            for name in ["pnpm.exe", "pnpm.cmd", "pnpm.bat"] {
                let candidate = directory.join(name);
                if candidate.is_file() {
                    return candidate.into_os_string();
                }
            }
        }
    }
    "pnpm".into()
}

fn command(root: &Path, program: &OsStr, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Command {
    let mut command = Command::new(program);
    command.current_dir(root).args(args);
    command
}

fn output(root: &Path, program: impl AsRef<OsStr>, args: &[&str]) -> Result<String, String> {
    capture(command(root, program.as_ref(), args))
}

fn capture(mut command: Command) -> Result<String, String> {
    let program = command.get_program().to_os_string();
    let result = command
        .output()
        .map_err(|error| format!("start {}: {error}", program.to_string_lossy()))?;
    if !result.status.success() {
        let diagnostics = String::from_utf8_lossy(&result.stderr);
        return Err(format!(
            "{} exited with {}: {}",
            program.to_string_lossy(),
            result.status,
            diagnostics.trim()
        ));
    }
    let value = String::from_utf8(result.stdout)
        .map_err(|error| format!("{} output: {error}", program.to_string_lossy()))?;
    Ok(value.trim().into())
}

fn run(
    root: &Path,
    program: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<(), String> {
    let program = program.as_ref();
    let status = command(root, program, args)
        .status()
        .map_err(|error| format!("start {}: {error}", program.to_string_lossy()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with {status}", program.to_string_lossy()))
    }
}
