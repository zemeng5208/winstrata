use std::io;
use std::process::ExitCode;

use winstrata_desktop::{Bridge, asset_directory, preference_directory};
use winstrata_service::Endpoint;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("winstrata-desktop: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let directory = preference_directory()?;
    let locator = directory
        .parent()
        .ok_or_else(|| io::Error::other("preference directory has no parent"))?
        .to_path_buf();
    let endpoint = Endpoint::user_default().ok();
    let bridge = Bridge::start(directory, asset_directory(), locator, endpoint)?;
    let url = bridge.url();
    println!("{url}");
    #[cfg(windows)]
    {
        winstrata_desktop::run_window(&url, bridge.action());
    }
    #[cfg(not(windows))]
    {
        // Linux 开发宿主提供同一官方页面。窗口由 Windows WebView2 打开。
        let _ = std::io::Read::read(&mut io::stdin(), &mut [0]);
    }
    Ok(())
}
