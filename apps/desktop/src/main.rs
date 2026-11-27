use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use winstrata_desktop::Desktop;
use winstrata_service::Endpoint;

fn main() -> ExitCode {
    let mut prefs = None;
    let mut locator = None;
    let mut service = None;
    let mut runtime = None;
    let mut assets = None;
    let mut host_script = None;
    let mut window = false;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        let value = match argument.as_str() {
            "--help" | "-h" => {
                println!(
                    "Usage: winstrata-desktop [--prefs file] [--locator dir] [--service address] [--runtime file] [--assets dir] [--host-script file] [--window]"
                );
                return ExitCode::SUCCESS;
            }
            "--window" => {
                window = true;
                continue;
            }
            "--prefs" | "--locator" | "--service" | "--runtime" | "--assets" | "--host-script" => {
                match arguments.next() {
                    Some(value) => value,
                    None => {
                        eprintln!("missing value for {argument}");
                        return ExitCode::from(2);
                    }
                }
            }
            _ => {
                eprintln!("unrecognized argument");
                return ExitCode::from(2);
            }
        };
        match argument.as_str() {
            "--prefs" => prefs = Some(PathBuf::from(value)),
            "--locator" => locator = Some(PathBuf::from(value)),
            "--service" => service = Some(value),
            "--runtime" => runtime = Some(PathBuf::from(value)),
            "--assets" => assets = Some(PathBuf::from(value)),
            "--host-script" => host_script = Some(PathBuf::from(value)),
            _ => {}
        }
    }

    if window && !cfg!(windows) {
        eprintln!("WebView2 window is not available on this operating system");
        return ExitCode::from(2);
    }
    let service = service.or_else(|| Endpoint::user_address().ok());
    let bundle = assets.unwrap_or_else(|| PathBuf::from("target/plan016-ui"));
    let mut running = match (Desktop {
        prefs: prefs.unwrap_or_else(default_prefs),
        locator,
        service,
        runtime,
        ui_root: winstrata_desktop::package_root(),
        bundle,
        tray: window,
    })
    .serve()
    {
        Ok(running) => running,
        Err(error) => {
            eprintln!("desktop host failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}", running.url(false));
    if !window {
        let mut byte = [0u8; 1];
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut byte);
        running.shutdown();
        return ExitCode::SUCCESS;
    }
    let script = host_script
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("windows/host.ps1"));
    let script = script.to_string_lossy().to_string();
    let url = running.url(false);
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-Sta", "-File", &script, "-Url", &url])
        .status();
    running.shutdown();
    match status {
        Ok(status) if status.code() == Some(2) => {
            eprintln!("WebView2 environment unverified");
            ExitCode::from(2)
        }
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(error) => {
            eprintln!("window host failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn default_prefs() -> PathBuf {
    if let Some(state) = env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(state).join("WinStrata/gui.json");
    }
    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(".local/state/WinStrata/gui.json");
    }
    if let Some(local) = env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
        return PathBuf::from(local).join("WinStrata/gui.json");
    }
    PathBuf::from("gui.json")
}
