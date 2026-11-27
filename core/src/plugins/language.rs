//! 项目声明的 Python 与 Node 解释器，以及按语言启动的命令。

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use winstrata_protocols::Manifest;

use super::PluginError;
use super::process::Live;

#[derive(Clone, Debug)]
pub struct LanguageRequirement {
    pub python_minimum: (u16, u16, u16),
    pub node_exact: (u16, u16, u16),
}

#[derive(Clone, Debug)]
pub(super) struct Toolchain {
    pub(super) python: PathBuf,
    pub(super) python_prefix: PathBuf,
    pub(super) node: PathBuf,
    pub(super) node_prefix: PathBuf,
    pub(super) sdk: PathBuf,
    pub(super) shim: PathBuf,
}

impl Toolchain {
    pub(super) fn bind(
        python: &Path,
        node: &Path,
        sdk: &Path,
        shim: &Path,
        requirement: &LanguageRequirement,
    ) -> Result<Self, PluginError> {
        let python = canonicalize_file(python, "Python")?;
        let node = canonicalize_file(node, "Node.js")?;
        let sdk = canonicalize_dir(sdk, "SDK")?;
        for relative in [
            "python/winstrata_sdk/run.py",
            "javascript/run.mjs",
            "javascript/winstrata.mjs",
            "typescript/run.ts",
            "typescript/winstrata.ts",
        ] {
            if !sdk.join(relative).is_file() {
                return Err(PluginError::new(
                    "toolchain_missing",
                    format!("SDK is missing {relative}"),
                ));
            }
        }
        let python_text = command_text(Command::new(&python).arg("--version"))?;
        let python_version = parse_version(&python_text).ok_or_else(|| {
            PluginError::new("toolchain_mismatch", "Python version could not be read")
        })?;
        if !python_satisfies(python_version, requirement.python_minimum) {
            return Err(PluginError::new(
                "toolchain_mismatch",
                format!("Python {} is below the project minimum", python_text.trim()),
            ));
        }
        let node_text = command_text(Command::new(&node).arg("--version"))?;
        let node_version = parse_version(&node_text).ok_or_else(|| {
            PluginError::new("toolchain_mismatch", "Node.js version could not be read")
        })?;
        if node_version != requirement.node_exact {
            return Err(PluginError::new(
                "toolchain_mismatch",
                format!(
                    "Node.js {} does not match the project pin",
                    node_text.trim()
                ),
            ));
        }
        let prefix =
            command_text(Command::new(&python).args(["-c", "import sys; print(sys.base_prefix)"]))?;
        let python_prefix = canonicalize_dir(Path::new(prefix.trim()), "Python prefix")?;
        let node_root = command_text(Command::new(&node).args([
            "-e",
            "const path=require('path'); const dir=path.dirname(process.execPath); console.log(path.basename(dir)==='bin'?path.dirname(dir):dir)",
        ]))?;
        let node_prefix = canonicalize_dir(Path::new(node_root.trim()), "Node.js prefix")?;
        let shim = if cfg!(unix) {
            canonicalize_file(shim, "isolation shim")?
        } else {
            PathBuf::new()
        };
        Ok(Self {
            python,
            python_prefix,
            node,
            node_prefix,
            sdk,
            shim,
        })
    }
}

pub(super) fn supported_runtime(name: &str) -> bool {
    matches!(name, "native" | "python" | "typescript" | "javascript")
}

pub(super) fn language_runtime(name: &str) -> bool {
    matches!(name, "python" | "typescript" | "javascript")
}

pub(super) fn entry_name(runtime: &str) -> Option<&'static str> {
    match runtime {
        "python" => Some("plugin.py"),
        "javascript" => Some("plugin.mjs"),
        "typescript" => Some("plugin.ts"),
        _ => None,
    }
}

pub(super) fn shim_beside(runtime: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "winstrata_shim.dll"
    } else {
        "libwinstrata_shim.so"
    };
    runtime
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

fn python_satisfies(actual: (u16, u16, u16), minimum: (u16, u16, u16)) -> bool {
    actual.0 == minimum.0 && (actual.1, actual.2) >= (minimum.1, minimum.2)
}

fn parse_version(text: &str) -> Option<(u16, u16, u16)> {
    let start = text.find(|character: char| character.is_ascii_digit())?;
    let mut parts = text[start..].split(|character: char| !character.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn command_text(command: &mut Command) -> Result<String, PluginError> {
    let output = command.output().map_err(|error| {
        PluginError::new(
            "toolchain_missing",
            format!("interpreter did not start: {error}"),
        )
    })?;
    if !output.status.success() {
        return Err(PluginError::new(
            "toolchain_mismatch",
            "interpreter version command failed",
        ));
    }
    let mut text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    }
    if text.is_empty() {
        return Err(PluginError::new(
            "toolchain_mismatch",
            "interpreter returned an empty version",
        ));
    }
    Ok(text)
}

fn canonicalize_file(path: &Path, label: &str) -> Result<PathBuf, PluginError> {
    let path = fs::canonicalize(path).map_err(|error| {
        PluginError::new(
            "toolchain_missing",
            format!("{label} is not available: {error}"),
        )
    })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(PluginError::new(
            "toolchain_missing",
            format!("{label} is not a file"),
        ))
    }
}

pub(super) fn launch(
    tools: &Toolchain,
    manifest: &Manifest,
    manifest_path: &Path,
    state: &Path,
    plugins_root: &Path,
) -> Result<Live, PluginError> {
    let runtime = manifest.runtime.as_str();
    let package = manifest_path.parent().ok_or_else(|| {
        PluginError::new(
            "invalid_package",
            "plugin manifest has no package directory",
        )
    })?;
    let modules = read_modules(package)?;
    let python = runtime == "python";
    let private = state.join(if python { "py" } else { "node" });
    fs::create_dir_all(&private)?;
    let catalog = catalog_dir(plugins_root, python, &modules)?;
    let (program, prefix, script) = if python {
        let environment = state.join("env");
        ensure_venv(&tools.python, &environment)?;
        let program = if cfg!(windows) {
            environment.join("Scripts").join("python.exe")
        } else {
            environment.join("bin").join("python")
        };
        (
            program,
            tools.python_prefix.clone(),
            tools.sdk.join("python/winstrata_sdk/run.py"),
        )
    } else if runtime == "typescript" {
        (
            tools.node.clone(),
            tools.node_prefix.clone(),
            tools.sdk.join("typescript/run.ts"),
        )
    } else {
        (
            tools.node.clone(),
            tools.node_prefix.clone(),
            tools.sdk.join("javascript/run.mjs"),
        )
    };
    let mut command = Command::new(&program);
    if runtime == "typescript" {
        command.arg("--experimental-strip-types");
    }
    if python {
        command.args(["-s", "-B", "-E"]);
    }
    command.arg(&script);
    command.arg("--manifest").arg(manifest_path);
    command.arg("--state").arg(state);
    if python {
        command.arg("--package").arg(package);
        command.arg("--private").arg(&private);
    } else {
        let entry = entry_name(runtime).ok_or_else(|| {
            PluginError::new("unsupported_runtime", "plugin runtime is not supported")
        })?;
        command.arg("--entry").arg(package.join(entry));
        command.arg("--private").arg(&private);
    }
    if let Some(catalog) = &catalog {
        command.arg("--catalog").arg(catalog);
    }
    command.current_dir(state);
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(state.join("stderr.log"))
        .map_err(|error| PluginError::new("environment_failed", error.to_string()))?;
    command.stderr(Stdio::from(log));

    let mut read = vec![prefix, tools.sdk.clone(), package.to_path_buf()];
    for optional in [
        "/lib",
        "/lib64",
        "/usr/lib",
        "/usr/lib64",
        "/dev/urandom",
        "/dev/null",
        "/etc/ld.so.cache",
        "/etc/ssl",
    ] {
        let path = Path::new(optional);
        if path.exists() {
            read.push(path.to_path_buf());
        }
    }
    if let Some(catalog) = catalog {
        read.push(catalog);
    }

    #[cfg(unix)]
    {
        command.env_clear();
        command.env("LC_ALL", "C.UTF-8");
        command.env("LD_PRELOAD", &tools.shim);
        let ruleset = super::sandbox::prepare_linux(&read, &[state])?;
        command.env("WINSTRATA_LANDLOCK_FD", ruleset.fd().to_string());
        let live = Live::spawn(command, manifest.plugin_version)?;
        drop(ruleset);
        return Ok(live);
    }
    #[cfg(windows)]
    {
        let _ = &tools.shim;
        super::sandbox::spawn_windows(&command, state, &read, manifest.plugin_version)
    }
}

fn catalog_dir(
    plugins_root: &Path,
    python: bool,
    modules: &[String],
) -> Result<Option<PathBuf>, PluginError> {
    if modules.is_empty() {
        return Ok(None);
    }
    let dir = plugins_root
        .join("catalog")
        .join(if python { "python" } else { "node" });
    let suffix = if python { "py" } else { "mjs" };
    for name in modules {
        if !dir.join(format!("{name}.{suffix}")).is_file() {
            return Err(PluginError::new(
                "dependency_missing",
                format!("project module {name} is not installed"),
            ));
        }
    }
    fs::canonicalize(&dir).map(Some).map_err(|error| {
        PluginError::new(
            "dependency_missing",
            format!("project module directory is not available: {error}"),
        )
    })
}

#[derive(Deserialize)]
struct ModuleFile {
    modules: Vec<String>,
}

fn read_modules(package: &Path) -> Result<Vec<String>, PluginError> {
    let path = package.join("modules.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| PluginError::new("invalid_manifest", error.to_string()))?;
    let file: ModuleFile = serde_json::from_str(&text)
        .map_err(|_| PluginError::new("invalid_manifest", "modules.json is invalid"))?;
    for name in &file.modules {
        if !module_name(name) {
            return Err(PluginError::new(
                "invalid_manifest",
                "module name is not a project token",
            ));
        }
    }
    Ok(file.modules)
}

fn module_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn ensure_venv(python: &Path, directory: &Path) -> Result<(), PluginError> {
    if directory.join("pyvenv.cfg").is_file() {
        return Ok(());
    }
    if directory.exists() {
        fs::remove_dir_all(directory)?;
    }
    let status = Command::new(python)
        .arg("-m")
        .arg("venv")
        .arg("--without-pip")
        .arg(directory)
        .status()
        .map_err(|error| PluginError::new("environment_failed", error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PluginError::new(
            "environment_failed",
            "Python environment could not be created",
        ))
    }
}

fn canonicalize_dir(path: &Path, label: &str) -> Result<PathBuf, PluginError> {
    let path = fs::canonicalize(path).map_err(|error| {
        PluginError::new(
            "toolchain_missing",
            format!("{label} is not available: {error}"),
        )
    })?;
    if path.is_dir() {
        Ok(path)
    } else {
        Err(PluginError::new(
            "toolchain_missing",
            format!("{label} is not a directory"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parser_reads_python_and_node_text() {
        assert_eq!(parse_version("Python 3.12.3\n"), Some((3, 12, 3)));
        assert_eq!(parse_version("v24.22.3"), Some((24, 22, 3)));
        assert!(python_satisfies((3, 12, 3), (3, 11, 0)));
        assert!(!python_satisfies((3, 10, 9), (3, 11, 0)));
        assert!(!python_satisfies((4, 0, 0), (3, 11, 0)));
    }
}
