use std::env;
use std::ffi::{CString, OsStr};
use std::fs::File;
use std::io::{self, Read};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) struct Anchor {
    directory: File,
    io_path: PathBuf,
}

impl Anchor {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Self::walk(path, false)
    }

    fn walk(path: &Path, create_state: bool) -> io::Result<Self> {
        let mut components = path.components();
        if components.next() != Some(Component::RootDir)
            || !components
                .clone()
                .all(|c| matches!(c, Component::Normal(_)))
            || path.as_os_str().as_bytes().contains(&0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected an absolute storage path without traversal",
            ));
        }
        let mut anchor = Self::from_directory(File::open("/")?);
        for component in components {
            anchor = anchor.state_component(component.as_os_str(), create_state)?;
        }
        Ok(anchor)
    }

    fn state_component(&self, name: &OsStr, create: bool) -> io::Result<Self> {
        match self.child(name, false, false) {
            // 已有的系统父目录可以共享；对新建目录及竞争创建者使用同一私有权限检查。
            Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                self.child(name, true, true)
            }
            result => result,
        }
    }

    fn from_directory(directory: File) -> Self {
        let io_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        Self { directory, io_path }
    }

    pub(crate) fn child(&self, name: &OsStr, create: bool, private: bool) -> io::Result<Self> {
        let bytes = name.as_bytes();
        if bytes.is_empty() || matches!(bytes, b"." | b"..") || bytes.contains(&b'/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected one directory name",
            ));
        }
        let name = CString::new(bytes)?;
        let parent = self.directory.as_raw_fd();
        let open = || {
            // O_PATH 允许穿过仅有执行权限的目录；NOFOLLOW 与 DIRECTORY 同时拒绝符号链接。
            let flags = libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_PATH;
            // SAFETY: 父句柄和以 NUL 结尾的名称在调用期间有效。
            let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
            if fd == -1 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: openat 返回的新句柄仅交由此 File 负责关闭。
            Ok(unsafe { File::from_raw_fd(fd) })
        };
        let directory = match open() {
            Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                let mode = if private { 0o700 } else { 0o755 };
                // O_PATH 不能用于 fsync。先固定可同步的父句柄，再创建目录项。
                let parent_to_sync = File::open(self.io_path())?;
                // SAFETY: 父句柄和名称有效；在目录创建时同时设置权限。
                if unsafe { libc::mkdirat(parent, name.as_ptr(), mode) } == -1 {
                    let error = io::Error::last_os_error();
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
                parent_to_sync.sync_all()?;
                open()?
            }
            result => result?,
        };
        if private {
            let metadata = directory.metadata()?;
            if metadata.uid() != effective_uid() || metadata.mode() & 0o077 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "directory must be private to the effective user",
                ));
            }
        }
        Ok(Self::from_directory(directory))
    }

    pub(crate) fn io_path(&self) -> &Path {
        &self.io_path
    }

    pub(crate) fn same_directory(&self, other: &Anchor) -> io::Result<bool> {
        let left = self.directory.metadata()?;
        let right = other.directory.metadata()?;
        Ok((left.dev(), left.ino()) == (right.dev(), right.ino()))
    }

    pub(crate) fn available(&self) -> io::Result<u64> {
        let mut result = MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: 句柄由 self 保持打开；仅在调用成功后读取系统完整填充的结构体。
        if unsafe { libc::fstatvfs(self.directory.as_raw_fd(), result.as_mut_ptr()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        let result = unsafe { result.assume_init() };
        let bytes = u128::from(result.f_bavail) * u128::from(result.f_frsize);
        u64::try_from(bytes).map_err(|_| io::Error::other("available capacity exceeds u64"))
    }
}

fn effective_uid() -> libc::uid_t {
    // SAFETY: geteuid 无参数且不会失败。
    unsafe { libc::geteuid() }
}

pub(crate) fn current_id() -> io::Result<String> {
    Ok(format!("uid-{}", effective_uid()))
}

pub(crate) fn fresh_id() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
}

pub(crate) fn locator_parent() -> io::Result<PathBuf> {
    if let Some(value) = env::var_os("XDG_STATE_HOME").filter(|x| !x.is_empty()) {
        let path = PathBuf::from(value);
        Anchor::walk(&path, true)?;
        return Ok(path);
    }
    let home = env::var_os("HOME")
        .filter(|x| !x.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "HOME or XDG_STATE_HOME is required",
            )
        })?;
    let home_dir = Anchor::open(&home)?;
    let local = home_dir.state_component(OsStr::new(".local"), true)?;
    local.state_component(OsStr::new("state"), true)?;
    Ok(home.join(".local/state"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            loop {
                let name = format!(
                    "winstrata-anchor-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                );
                let dir = env::temp_dir().join(name);
                match DirBuilder::new().mode(0o700).create(&dir) {
                    Ok(()) => return Self(dir),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn private_directory_checks_real_owner_mode_and_capacity() {
        let temp = TestDirectory::new();
        let base = Anchor::open(&temp.0).unwrap();
        let child = base.child(OsStr::new("private"), true, true).unwrap();
        let metadata = fs::metadata(child.io_path()).unwrap();
        assert_eq!(current_id().unwrap(), format!("uid-{}", metadata.uid()));
        assert_eq!(metadata.mode() & 0o077, 0);
        assert!(child.available().unwrap() > 0);
        fs::set_permissions(child.io_path(), fs::Permissions::from_mode(0o750)).unwrap();
        let error = base.child(OsStr::new("private"), false, true).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn creation_requires_a_syncable_parent_before_publishing_a_child() {
        let temp = TestDirectory::new();
        let base = Anchor::open(&temp.0).unwrap();
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o300)).unwrap();
        // 有些测试环境具备越过权限位的能力，按真实的句柄打开结果判断。
        let can_sync = File::open(&temp.0).is_ok();
        let created = base.child(OsStr::new("durable"), true, true);
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o700)).unwrap();
        if can_sync {
            created.unwrap();
        } else {
            assert_eq!(created.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
            assert!(!temp.0.join("durable").exists());
            base.child(OsStr::new("durable"), true, true).unwrap();
        }
    }

    #[test]
    fn marker_ids_are_fresh_lowercase_hex() {
        let first = fresh_id().unwrap();
        let second = fresh_id().unwrap();
        for id in [&first, &second] {
            assert_eq!(id.len(), 32);
            assert!(id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        }
        assert_ne!(first, second);
    }

    #[test]
    fn state_parent_creates_private_missing_components_and_rejects_redirects() {
        let temp = TestDirectory::new();
        let state = temp.0.join("app/state");
        assert_eq!(
            Anchor::open(&state).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        let opened = Anchor::walk(&state, true).unwrap();
        for path in [temp.0.join("app"), state.clone()] {
            let metadata = fs::metadata(path).unwrap();
            assert_eq!(metadata.uid(), effective_uid());
            assert_eq!(metadata.mode() & 0o077, 0);
        }
        assert_eq!(
            opened.directory.metadata().unwrap().ino(),
            fs::metadata(&state).unwrap().ino()
        );
        symlink(&state, temp.0.join("alias")).unwrap();
        assert!(Anchor::walk(&temp.0.join("alias/new"), true).is_err());
        assert!(!state.join("new").exists());
        assert!(Anchor::walk(&temp.0.join("wrong/../elsewhere"), true).is_err());
        assert!(!temp.0.join("wrong").exists());
    }

    #[test]
    fn follows_pinned_directory_after_rename_and_refuses_links_or_traversal() {
        let temp = TestDirectory::new();
        let base = Anchor::open(&temp.0).unwrap();
        let held = base.child(OsStr::new("private"), true, true).unwrap();
        let moved = temp.0.join("moved");
        let old = temp.0.join("private");
        fs::rename(&old, &moved).unwrap();
        fs::create_dir(&old).unwrap();
        assert!(held.same_directory(&Anchor::open(&moved).unwrap()).unwrap());
        assert!(!held.same_directory(&Anchor::open(&old).unwrap()).unwrap());
        fs::write(held.io_path().join("marker"), b"held").unwrap();
        assert_eq!(fs::read(moved.join("marker")).unwrap(), b"held");
        assert!(!old.join("marker").exists());

        symlink(&moved, temp.0.join("alias")).unwrap();
        fs::create_dir(moved.join("real-child")).unwrap();
        assert!(Anchor::open(&moved.join("real-child")).is_ok());
        assert!(base.child(OsStr::new("alias"), true, true).is_err());
        assert!(Anchor::open(&temp.0.join("alias/real-child")).is_err());
        assert!(Anchor::open(&temp.0.join("moved/../private")).is_err());
        assert!(Anchor::open(Path::new("relative")).is_err());
        for bad_name in ["", ".", "..", "moved/name", "name/", "/absolute", "nul\0"] {
            assert!(base.child(OsStr::new(bad_name), true, false).is_err());
        }
    }
}
