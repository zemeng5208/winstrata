use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use winstrata_kernel::DriverHost;

use crate::session::Session;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
use windows as platform;

pub(crate) use platform::Connection;

const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privilege {
    WindowsElevated,
    WindowsOrdinary,
    NotWindows,
}

impl Privilege {
    pub const fn can_manage_drivers(self) -> bool {
        matches!(self, Self::WindowsElevated)
    }
}

pub struct Endpoint {
    address: String,
    owned_file: Option<PathBuf>,
    owned_dir: Option<PathBuf>,
}

impl Endpoint {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn unique() -> io::Result<Self> {
        platform::unique_endpoint()
    }

    pub fn user_default() -> io::Result<Self> {
        platform::user_endpoint()
    }

    pub(crate) fn cleanup(&mut self) {
        if let Some(file) = self.owned_file.take() {
            let _ = std::fs::remove_file(file);
        }
        if let Some(dir) = self.owned_dir.take() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

pub fn current_user() -> io::Result<String> {
    platform::current_user()
}

pub fn current_privilege() -> io::Result<Privilege> {
    platform::current_privilege()
}

impl Endpoint {
    pub(crate) fn anonymous(&self) -> Self {
        Self {
            address: self.address.clone(),
            owned_file: None,
            owned_dir: None,
        }
    }
}

pub(crate) fn connect(endpoint: &Endpoint) -> io::Result<Connection> {
    platform::connect(&endpoint.address)
}

pub(crate) fn serve<H>(
    endpoint: &Endpoint,
    session: Session<H>,
    stop: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>>
where
    H: DriverHost + Send + 'static,
{
    platform::serve(&endpoint.address, session, stop)
}

pub(crate) fn next_name(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let number = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{number}", std::process::id())
}
