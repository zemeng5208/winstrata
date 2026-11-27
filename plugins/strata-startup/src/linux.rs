//! Linux 读取 XDG 自启动、cron 文件和 systemd timer 文件。
//!
//! 不调用 crontab、systemctl 或桌面启动程序。子进程环境被清空，家目录来自 passwd。
//! 这些文件不是 Windows 启动文件夹、Run 键或任务计划程序。

use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use super::{Layout, from_layout};

pub(super) fn capture() -> io::Result<super::StartupSnapshot> {
    let device = identity(
        &fs::read_to_string("/etc/machine-id")
            .map_err(|error| io::Error::new(error.kind(), "cannot read the machine id"))?,
    )?;
    let boot = identity(
        &fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|error| io::Error::new(error.kind(), "cannot read the boot id"))?,
    )?;
    let config = user_config_dir().ok();
    let (user_crontabs, user_cron_denied) = user_crontabs();
    let mut layout = Layout {
        autostart_system: xdg_system_autostart(),
        autostart_user: config
            .as_ref()
            .map(|dir| vec![dir.join("autostart")])
            .unwrap_or_default(),
        autostart_unreadable: config.is_none(),
        system_crontabs: vec![PathBuf::from("/etc/crontab")],
        cron_directories: vec![PathBuf::from("/etc/cron.d")],
        user_crontabs,
        periodic: ["hourly", "daily", "weekly", "monthly"]
            .into_iter()
            .map(|label| {
                (
                    label.to_owned(),
                    PathBuf::from(format!("/etc/cron.{label}")),
                )
            })
            .collect(),
        system_timer_dirs: vec![
            PathBuf::from("/usr/lib/systemd/system"),
            PathBuf::from("/lib/systemd/system"),
            PathBuf::from("/run/systemd/system"),
            PathBuf::from("/etc/systemd/system"),
        ],
        system_timer_wants: vec![
            PathBuf::from("/usr/lib/systemd/system/timers.target.wants"),
            PathBuf::from("/lib/systemd/system/timers.target.wants"),
            PathBuf::from("/run/systemd/system/timers.target.wants"),
            PathBuf::from("/etc/systemd/system/timers.target.wants"),
        ],
        user_timer_dirs: vec![PathBuf::from("/etc/systemd/user")],
        user_timer_wants: vec![PathBuf::from("/etc/systemd/user/timers.target.wants")],
        cron_unreadable: user_cron_denied,
        ..Layout::default()
    };
    if let Some(config) = config {
        layout.user_timer_dirs.push(config.join("systemd/user"));
        layout
            .user_timer_wants
            .push(config.join("systemd/user/timers.target.wants"));
    } else {
        layout.user_timers_unreadable = true;
    }
    from_layout(&device, &boot, &layout, super::unix_now()?)
}

pub(super) fn user_config_dir() -> io::Result<PathBuf> {
    let uid = unsafe { libc::geteuid() };
    let mut pwd = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut buf = vec![0u8; 16 * 1024];
    let mut result = std::ptr::null_mut();
    let code = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            buf.as_mut_ptr().cast(),
            buf.len(),
            &mut result,
        )
    };
    if code != 0 || result.is_null() {
        return Err(io::Error::new(
            ErrorKind::NotFound,
            "cannot resolve the current user home",
        ));
    }
    let home = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) };
    let home = home
        .to_str()
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "home directory is not utf-8"))?;
    if home.is_empty() || !home.starts_with('/') {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "home directory is not absolute",
        ));
    }
    Ok(PathBuf::from(home).join(".config"))
}

fn xdg_system_autostart() -> Vec<PathBuf> {
    let dirs = std::env::var_os("XDG_CONFIG_DIRS");
    let raw = dirs
        .as_ref()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("/etc/xdg");
    raw.split(':')
        .filter(|dir| dir.starts_with('/') && !dir.contains('\0'))
        .map(|dir| PathBuf::from(dir).join("autostart"))
        .collect()
}

fn user_crontabs() -> (Vec<PathBuf>, bool) {
    let mut paths = Vec::new();
    let mut denied = false;
    for dir in ["/var/spool/cron/crontabs", "/var/spool/cron"] {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => {
                denied = true;
                continue;
            }
        };
        for entry in entries.flatten() {
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.is_file()
                || (meta.file_type().is_symlink()
                    && entry.metadata().is_ok_and(|item| item.is_file()))
            {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    (paths, denied)
}

fn identity(value: &str) -> io::Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 256
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "machine or boot identity is empty or not a single token",
        ));
    }
    Ok(value.to_owned())
}
