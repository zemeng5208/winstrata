#![allow(unsafe_code)]

//! 用户脚本子进程的隔离。
//!
//! Linux 复用计划 015 的 Landlock 和预加载库。规则无法建立时不启动脚本。
//! Windows 的 AppContainer 仍由宿主的语言插件路径持有，本进程不复制那套实现。

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub(crate) fn spawn_contained(
    command: &mut Command,
    read: &[PathBuf],
    write: &[PathBuf],
    shim: Option<&Path>,
) -> io::Result<Child> {
    #[cfg(unix)]
    {
        unix::spawn(command, read, write, shim)
    }
    #[cfg(not(unix))]
    {
        let _ = (read, write, shim);
        command.spawn()
    }
}

#[cfg(unix)]
mod unix {
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};

    const SYS_CREATE: libc::c_long = 444;
    const SYS_ADD: libc::c_long = 445;
    const VERSION_FLAG: libc::c_long = 1;
    const RULE_PATH: libc::c_long = 1;
    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_REFER: u64 = 1 << 13;
    const FS_TRUNCATE: u64 = 1 << 14;
    const FS_IOCTL_DEV: u64 = 1 << 15;
    const NET_BIND_TCP: u64 = 1 << 0;
    const NET_CONNECT_TCP: u64 = 1 << 1;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    #[repr(C)]
    struct PathBeneath {
        allowed_access: u64,
        parent_fd: i32,
        _pad: u32,
    }

    struct Ruleset {
        fd: i32,
    }

    impl Drop for Ruleset {
        fn drop(&mut self) {
            unsafe { libc::close(self.fd) };
        }
    }

    pub(super) fn spawn(
        command: &mut Command,
        read: &[PathBuf],
        write: &[PathBuf],
        shim: Option<&Path>,
    ) -> io::Result<Child> {
        let Some(shim) = shim.filter(|path| path.is_file()) else {
            return Err(io::Error::other("isolation_unavailable"));
        };
        let ruleset = prepare(read, write).map_err(|error| io::Error::other(error))?;
        command.env("LD_PRELOAD", shim);
        command.env("WINSTRATA_LANDLOCK_FD", ruleset.fd.to_string());
        let child = command.spawn()?;
        drop(ruleset);
        Ok(child)
    }

    fn prepare(read: &[PathBuf], write: &[PathBuf]) -> Result<Ruleset, String> {
        let abi = unsafe { libc::syscall(SYS_CREATE, 0, 0, VERSION_FLAG) };
        if abi < 1 {
            return Err("isolation_unavailable".to_owned());
        }
        let mut handled = FS_EXECUTE
            | FS_WRITE_FILE
            | FS_READ_FILE
            | FS_READ_DIR
            | FS_REMOVE_DIR
            | FS_REMOVE_FILE
            | FS_MAKE_DIR
            | FS_MAKE_REG;
        if abi >= 2 {
            handled |= FS_REFER;
        }
        if abi >= 3 {
            handled |= FS_TRUNCATE;
        }
        if abi >= 5 {
            handled |= FS_IOCTL_DEV;
        }
        let net = if abi >= 4 {
            NET_BIND_TCP | NET_CONNECT_TCP
        } else {
            0
        };
        let attr = RulesetAttr {
            handled_access_fs: handled,
            handled_access_net: net,
        };
        let fd = unsafe {
            libc::syscall(
                SYS_CREATE,
                &attr as *const RulesetAttr,
                size_of::<RulesetAttr>() as libc::c_long,
                0,
            )
        };
        if fd < 0 {
            return Err("isolation_unavailable".to_owned());
        }
        let ruleset = Ruleset { fd: fd as i32 };
        let read_access = (FS_EXECUTE | FS_READ_FILE | FS_READ_DIR) & handled;
        let write_access = (read_access
            | FS_WRITE_FILE
            | FS_REMOVE_DIR
            | FS_REMOVE_FILE
            | FS_MAKE_DIR
            | FS_MAKE_REG
            | FS_REFER
            | FS_TRUNCATE)
            & handled;
        for path in read {
            add_path(ruleset.fd, path, read_access)?;
        }
        for path in write {
            add_path(ruleset.fd, path, write_access)?;
        }
        if unsafe { libc::fcntl(ruleset.fd, libc::F_SETFD, 0) } != 0 {
            return Err("isolation_unavailable".to_owned());
        }
        Ok(ruleset)
    }

    fn add_path(ruleset: i32, path: &Path, access: u64) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let file = open_path(path)?;
        let metadata = file.metadata().map_err(|_| "isolation_unavailable".to_owned())?;
        let access = if metadata.is_dir() {
            access
        } else {
            access & FS_READ_FILE
        };
        if access == 0 {
            return Ok(());
        }
        let attr = PathBeneath {
            allowed_access: access,
            parent_fd: file.as_raw_fd(),
            _pad: 0,
        };
        let result = unsafe {
            libc::syscall(
                SYS_ADD,
                ruleset as libc::c_long,
                RULE_PATH,
                &attr as *const PathBeneath,
                0,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err("isolation_unavailable".to_owned())
        }
    }

    fn open_path(path: &Path) -> Result<File, String> {
        let mut bytes = path.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        let fd = unsafe { libc::open(bytes.as_ptr().cast(), libc::O_PATH | libc::O_CLOEXEC) };
        if fd < 0 {
            Err("isolation_unavailable".to_owned())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
}
