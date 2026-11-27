use crate::platform::Anchor;
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// 这些文件只保存定位器与控制配置，不能用于日志或观测数据。
const MAX_JSON_BYTES: usize = 64 * 1024;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn control_path(dir: &Anchor, name: &str) -> io::Result<PathBuf> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with('.')
        || device
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid control file name",
        ));
    }
    Ok(dir.io_path().join(name))
}

#[cfg(target_os = "linux")]
fn verify(file: &File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(invalid("expected a regular file with one link"));
    }
    // SAFETY: geteuid 无参数且不会失败。
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "control file must be private to the current user",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn verify(file: &File) -> io::Result<()> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_TYPE_DISK, GetFileInformationByHandle, GetFileType,
    };
    let handle = file.as_raw_handle();
    // SAFETY: 在系统读取句柄信息期间，File 仍然有效。
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Err(invalid("expected a disk file"));
    }
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: 仅在 API 成功填充结构体后读取其内容。
    if unsafe { GetFileInformationByHandle(handle, info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || info.nNumberOfLinks != 1
    {
        return Err(invalid("expected a regular file with one link"));
    }
    crate::platform::verify_private_file(file)
}

pub(crate) fn open_existing(path: &Path, write: bool) -> io::Result<File> {
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid("control file is not a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // 符号链接在打开时被拒绝；对被替换为管道的路径也不会阻塞。
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    verify(&file)?;
    Ok(file)
}

#[cfg(target_os = "linux")]
pub(crate) fn create_private(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(windows)]
pub(crate) use crate::platform::private_file_create as create_private;

pub(crate) fn verify_session(dir: &Anchor, held: &File) -> io::Result<()> {
    let current = open_existing(&control_path(dir, "session.lock")?, false)?;
    #[cfg(windows)]
    let same = crate::platform::file_key(held)? == crate::platform::file_key(&current)?;
    #[cfg(target_os = "linux")]
    let same = {
        use std::os::unix::fs::MetadataExt;
        let a = held.metadata()?;
        let b = current.metadata()?;
        (a.dev(), a.ino()) == (b.dev(), b.ino())
    };
    if same {
        Ok(())
    } else {
        Err(invalid("locator session lock was replaced"))
    }
}

pub(crate) fn lock(dir: &Anchor, name: &str) -> io::Result<File> {
    let path = control_path(dir, name)?;
    let file = match create_private(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => open_existing(&path, true)?,
        Err(error) => return Err(error),
    };
    file.try_lock()?;
    Ok(file)
}

struct Temporary {
    path: PathBuf,
    file: File,
    cleanup: bool,
}

impl Temporary {
    fn create(dir: &Anchor) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..64 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = dir
                .io_path()
                .join(format!(".winstrata-{}-{id}.tmp", std::process::id()));
            match create_private(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file,
                        cleanup: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary file names are in use",
        ))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        if self.cleanup {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn read_json<T: DeserializeOwned>(dir: &Anchor, name: &str) -> io::Result<T> {
    let file = open_existing(&control_path(dir, name)?, false)?;
    let mut bytes = Vec::new();
    file.take(MAX_JSON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(invalid("control file is too large"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

struct JsonBuffer(Vec<u8>);

impl Write for JsonBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_JSON_BYTES - self.0.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "control file is too large",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn encode_json<T: Serialize + ?Sized>(value: &T) -> io::Result<Vec<u8>> {
    let mut encoded = JsonBuffer(Vec::new());
    serde_json::to_writer(&mut encoded, value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(encoded.0)
}

pub(crate) fn write_json<T: Serialize + ?Sized>(
    dir: &Anchor,
    name: &str,
    value: &T,
) -> io::Result<()> {
    write_encoded(dir, name, &encode_json(value)?)
}

#[cfg(windows)]
pub(crate) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<_> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: 两条路径均由相同私有目录和受检文件名生成，缓冲区在调用时有效。
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) use fs::rename as replace_file;

pub(crate) fn write_encoded(dir: &Anchor, name: &str, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "control file is too large",
        ));
    }
    let dest = control_path(dir, name)?;
    match open_existing(&dest, false) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // 临时文件与目标同目录；写入或同步失败时保留旧版本。
    let mut temp = Temporary::create(dir)?;
    temp.file.write_all(bytes)?;
    temp.file.sync_all()?;
    replace_file(&temp.path, &dest)?;
    temp.cleanup = false;
    #[cfg(target_os = "linux")]
    File::open(dir.io_path())?.sync_all()?;
    Ok(())
}

pub(crate) fn probe(dir: &Anchor) -> io::Result<()> {
    let mut temp = Temporary::create(dir)?;
    temp.file.write_all(b"WinStrata storage check\n")?;
    temp.file.sync_all()?;
    fs::remove_file(&temp.path)?;
    temp.cleanup = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn in_private_directory(test: impl FnOnce(&Anchor)) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir();
        let name = format!(
            "winstrata-files-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let base = Anchor::open(&parent).unwrap();
        let directory = base.child(OsStr::new(&name), true, true).unwrap();
        test(&directory);
        drop(directory);
        fs::remove_dir(parent.join(name)).unwrap();
    }

    #[test]
    fn bounded_json_replacement_and_exclusive_lock() {
        in_private_directory(|dir| {
            let name = "state.json";
            write_json(dir, name, &1_u64).unwrap();
            write_json(dir, name, &2_u64).unwrap();
            assert_eq!(read_json::<u64>(dir, name).unwrap(), 2);
            assert_eq!(
                write_json(dir, name, &"x".repeat(MAX_JSON_BYTES))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            let invalid_key = std::collections::BTreeMap::from([((1, 2), "value")]);
            assert_eq!(
                write_json(dir, name, &invalid_key).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(read_json::<u64>(dir, name).unwrap(), 2);
            let file = dir.io_path().join(name);
            fs::write(&file, vec![b' '; MAX_JSON_BYTES + 1]).unwrap();
            assert_eq!(
                read_json::<u64>(dir, name).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            let held = lock(dir, "account.lock").unwrap();
            assert_eq!(
                lock(dir, "account.lock").unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            drop(held);
            drop(lock(dir, "account.lock").unwrap());
            probe(dir).unwrap();
            assert_eq!(fs::read_dir(dir.io_path()).unwrap().count(), 2);
            for invalid in ["../state", "NUL.txt", "state.json:stream", "one\\two", "."] {
                assert!(write_json(dir, invalid, &0_u64).is_err());
            }
            fs::remove_file(file).unwrap();
            fs::remove_file(dir.io_path().join("account.lock")).unwrap();
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_nonprivate_files_links_and_fifos() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::{PermissionsExt, symlink};
        in_private_directory(|dir| {
            let real = dir.io_path().join("state.json");
            let alias = dir.io_path().join("alias.json");
            write_json(dir, "state.json", &42_u64).unwrap();
            assert_eq!(
                fs::metadata(&real).unwrap().permissions().mode() & 0o777,
                0o600
            );
            symlink(&real, &alias).unwrap();
            assert!(write_json(dir, "alias.json", &0_u64).is_err());
            fs::remove_file(&alias).unwrap();
            fs::hard_link(&real, &alias).unwrap();
            assert!(read_json::<u64>(dir, "alias.json").is_err());
            assert!(write_json(dir, "state.json", &0_u64).is_err());
            fs::remove_file(&alias).unwrap();
            assert_eq!(read_json::<u64>(dir, "state.json").unwrap(), 42);
            fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
            assert!(read_json::<u64>(dir, "state.json").is_err());
            fs::remove_file(&real).unwrap();
            let fifo = CString::new(alias.as_os_str().as_bytes()).unwrap();
            // SAFETY: 路径为以 NUL 结尾的缓冲区，调用期间有效。
            assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
            assert!(read_json::<u64>(dir, "alias.json").is_err());
            fs::remove_file(&alias).unwrap();
        });
    }
}
