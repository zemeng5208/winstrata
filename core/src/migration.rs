use std::collections::{BTreeMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::platform::Anchor;

const STAGING: &str = ".winstrata-migration.partial";

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Directory,
    File,
}

fn entries(dir: &Anchor) -> io::Result<BTreeMap<OsString, Kind>> {
    fs::read_dir(dir.io_path())?
        .map(|entry| {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes()
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err(invalid("reparse points are not migration inputs"));
                }
            }
            let kind = if metadata.is_dir() {
                Kind::Directory
            } else if metadata.is_file() {
                Kind::File
            } else {
                return Err(invalid("migration requires regular files and directories"));
            };
            Ok((entry.file_name(), kind))
        })
        .collect()
}

fn directory_at(root: &Anchor, relative: &Path, private: bool) -> io::Result<Option<Anchor>> {
    let mut current = None;
    for component in relative.iter() {
        current = Some(
            current
                .as_ref()
                .unwrap_or(root)
                .child(component, false, private)?,
        );
    }
    Ok(current)
}

#[cfg(target_os = "linux")]
fn file_identity(file: &File) -> io::Result<((u64, u64), u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(((metadata.dev(), metadata.ino()), metadata.nlink()))
}

#[cfg(windows)]
fn file_identity(file: &File) -> io::Result<((u64, u64), u64)> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
    let mut info = MaybeUninit::uninit();
    // SAFETY: file 拥有有效句柄，仅在 API 成功后读取完整结构体。
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Ok((
        (info.dwVolumeSerialNumber.into(), index),
        info.nNumberOfLinks.into(),
    ))
}

fn directory_id(dir: &Anchor) -> io::Result<(u64, u64)> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        };
        options
            .access_mode(FILE_READ_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    }
    Ok(file_identity(&options.open(dir.io_path())?)?.0)
}

// 使用卷和文件标识检测别名、目录环及嵌套目标，避免依赖大小写或路径拼写。
fn inspect_tree(
    root: &Anchor,
    ids: &mut HashSet<(u64, u64)>,
    destination: Option<&Anchor>,
) -> io::Result<u64> {
    let mut missing_bytes = 0u64;
    let mut pending = vec![PathBuf::new()];
    while let Some(path) = pending.pop() {
        let child = directory_at(root, &path, destination.is_none())?;
        let dir = child.as_ref().unwrap_or(root);
        if !ids.insert(directory_id(dir)?) {
            return Err(invalid(
                "migration trees overlap or contain directory aliases",
            ));
        }
        let existing = if let Some(target) = destination {
            match directory_at(target, &path, true) {
                Ok(peer) => entries(peer.as_ref().unwrap_or(target))?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
                Err(error) => return Err(error),
            }
        } else {
            BTreeMap::new()
        };
        for (name, kind) in entries(dir)? {
            if destination.is_some() && name.eq_ignore_ascii_case(STAGING) {
                return Err(invalid(
                    "source contains the reserved migration staging name",
                ));
            }
            if kind == Kind::Directory {
                pending.push(path.join(name));
            } else if destination.is_some() {
                let file = regular(&dir.io_path().join(&name))?;
                if !existing.contains_key(&name) {
                    missing_bytes = missing_bytes
                        .checked_add(file.metadata()?.len())
                        .ok_or_else(|| invalid("migration size exceeds supported capacity"))?;
                }
            } else {
                crate::files::open_existing(&dir.io_path().join(name), false)?;
            }
        }
    }
    Ok(missing_bytes)
}

fn regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(invalid("file is a reparse point"));
        }
    }
    if !metadata.is_file() || file_identity(&file)?.1 != 1 {
        return Err(invalid("migration requires unaliased regular files"));
    }
    Ok(file)
}

fn compare(source: &Path, other: &Path, prefix: bool) -> io::Result<()> {
    let mut source = regular(source)?;
    let mut other = crate::files::open_existing(other, false)?;
    let mut actual = [0; 16 * 1024];
    let mut expected = [0; 16 * 1024];
    loop {
        let count = other.read(&mut actual)?;
        if count == 0 {
            break;
        }
        source.read_exact(&mut expected[..count])?;
        if actual[..count] != expected[..count] {
            return Err(invalid("migration file contents differ"));
        }
    }
    if !prefix && source.read(&mut expected[..1])? != 0 {
        return Err(invalid("migration file lengths differ"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn publish(stage: &Path, target: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let stage = CString::new(stage.as_os_str().as_bytes())?;
    let target = CString::new(target.as_os_str().as_bytes())?;
    // SAFETY: 两条路径均以 NUL 结尾；内核原子地发布文件并禁止覆盖。
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            stage.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn publish(stage: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
    let stage: Vec<_> = stage.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: 缓冲区在调用期间有效；不传替换现有文件或跨卷复制标志。
    if unsafe { MoveFileExW(stage.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn copy_new(source: &Path, stage: &Path, target: &Path) -> io::Result<()> {
    let mut input = regular(source)?;
    let mut output = crate::files::create_private(stage)?;
    io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    compare(source, stage, false)?;
    publish(stage, target)
}

fn transfer_directory(
    source: &Anchor,
    destination: &Anchor,
    verify_only: bool,
) -> io::Result<Vec<OsString>> {
    let src = entries(source)?;
    let mut dst = entries(destination)?;
    let stage_name = OsStr::new(STAGING);
    let mut stage = match dst.remove(stage_name) {
        None => None,
        Some(Kind::Directory) if !verify_only => Some(destination.child(stage_name, false, true)?),
        _ => return Err(invalid("unexpected migration staging entry")),
    };
    if dst.iter().any(|(name, kind)| src.get(name) != Some(kind)) {
        return Err(invalid("destination contains unexpected migration data"));
    }
    if verify_only && dst.len() != src.len() {
        return Err(invalid("destination is incomplete"));
    }
    let staged = match &stage {
        Some(dir) => entries(dir)?,
        None => BTreeMap::new(),
    };
    if staged
        .iter()
        .any(|(name, kind)| *kind != Kind::File || src.get(name) != Some(&Kind::File))
    {
        return Err(invalid("staging contains unexpected migration data"));
    }

    let mut children = Vec::new();
    for (name, kind) in src {
        if name.eq_ignore_ascii_case(STAGING) {
            return Err(invalid("reserved source entry"));
        }
        let target = destination.io_path().join(&name);
        if kind == Kind::Directory {
            destination.child(&name, !verify_only, true)?;
            children.push(name);
            continue;
        }
        let original = source.io_path().join(&name);
        if dst.contains_key(&name) {
            compare(&original, &target, false)?;
        }
        if staged.contains_key(&name) {
            let partial = stage
                .as_ref()
                .expect("staging was enumerated")
                .io_path()
                .join(&name);
            // 只删除与对应源文件前缀逐字节一致的本次迁移残留，从不就地改写已有文件。
            compare(&original, &partial, true)?;
            fs::remove_file(partial)?;
        }
        if !dst.contains_key(&name) {
            if stage.is_none() {
                stage = Some(destination.child(stage_name, true, true)?);
            }
            let partial = stage
                .as_ref()
                .expect("staging was opened")
                .io_path()
                .join(&name);
            copy_new(&original, &partial, &target)?;
        }
    }
    if stage.is_some() {
        drop(stage);
        fs::remove_dir(destination.io_path().join(stage_name))?;
    }
    #[cfg(target_os = "linux")]
    if !verify_only {
        File::open(destination.io_path())?.sync_all()?;
    }
    Ok(children)
}

pub(crate) fn copy_and_verify(source: &Anchor, destination: &Anchor) -> io::Result<()> {
    let mut ids = HashSet::new();
    let needed = inspect_tree(source, &mut ids, Some(destination))?;
    inspect_tree(destination, &mut ids, None)?;
    if needed > destination.available()? {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "insufficient capacity for migration",
        ));
    }
    // 两次独立枚举，复制完成后再次从磁盘核验所有名称、类型和字节。
    for verify_only in [false, true] {
        let mut pending = vec![PathBuf::new()];
        while let Some(path) = pending.pop() {
            let src = directory_at(source, &path, false)?;
            let dst = directory_at(destination, &path, true)?;
            for name in transfer_directory(
                src.as_ref().unwrap_or(source),
                dst.as_ref().unwrap_or(destination),
                verify_only,
            )? {
                pending.push(path.join(name));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn write_private(path: &Path, bytes: &[u8]) {
        crate::files::create_private(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    fn with_workspaces(test: impl FnOnce(&Anchor, &Anchor)) {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let temp = std::env::temp_dir();
        let name = format!(
            "winstrata-migration-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let root = Anchor::open(&temp).unwrap();
        let path = temp.join(&name);
        assert!(!path.exists());
        {
            let scratch = root.child(OsStr::new(&name), true, true).unwrap();
            let source = scratch.child(OsStr::new("from"), true, true).unwrap();
            let destination = scratch.child(OsStr::new("to"), true, true).unwrap();
            test(&source, &destination);
        }
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn resumes_a_matching_partial_and_retains_the_source_and_empty_directories() {
        with_workspaces(|source, target| {
            let bytes: Vec<u8> = (0..80_000).map(|x| (x % 251) as u8).collect();
            fs::write(source.io_path().join("data"), &bytes).unwrap();
            source.child(OsStr::new("empty"), true, true).unwrap();
            let partial = target.child(OsStr::new(STAGING), true, true).unwrap();
            write_private(&partial.io_path().join("data"), &bytes[..23_456]);
            drop(partial);
            assert_eq!(
                inspect_tree(source, &mut HashSet::new(), Some(target)).unwrap(),
                bytes.len() as u64
            );
            copy_and_verify(source, target).unwrap();
            assert_eq!(
                inspect_tree(source, &mut HashSet::new(), Some(target)).unwrap(),
                0
            );
            copy_and_verify(source, target).unwrap();
            assert_eq!(fs::read(source.io_path().join("data")).unwrap(), bytes);
            assert_eq!(fs::read(target.io_path().join("data")).unwrap(), bytes);
            assert!(target.io_path().join("empty").is_dir());
            assert!(!target.io_path().join(STAGING).exists());
        });
    }

    #[test]
    fn rejects_different_bytes_and_unknown_entries_without_overwriting() {
        with_workspaces(|source, target| {
            let target_path = target.io_path().join("log");
            fs::write(source.io_path().join("log"), b"one").unwrap();
            write_private(&target_path, b"two");
            assert!(copy_and_verify(source, target).is_err());
            assert_eq!(fs::read(&target_path).unwrap(), b"two");
            fs::remove_file(&target_path).unwrap();
            let partial = target.child(OsStr::new(STAGING), true, true).unwrap();
            write_private(&partial.io_path().join("log"), b"x");
            assert!(copy_and_verify(source, target).is_err());
            assert_eq!(fs::read(partial.io_path().join("log")).unwrap(), b"x");
            fs::remove_file(partial.io_path().join("log")).unwrap();
            drop(partial);
            write_private(&target.io_path().join("unknown"), b"keep");
            assert!(copy_and_verify(source, target).is_err());
            assert_eq!(fs::read(target.io_path().join("unknown")).unwrap(), b"keep");
        });
    }

    #[test]
    fn rejects_overlapping_roots_and_hard_link_aliases() {
        with_workspaces(|source, target| {
            let nested = source.child(OsStr::new("nested"), true, true).unwrap();
            assert!(copy_and_verify(source, source).is_err());
            assert!(copy_and_verify(source, &nested).is_err());
            assert!(copy_and_verify(&nested, source).is_err());
            fs::write(source.io_path().join("file"), b"keep").unwrap();
            fs::hard_link(source.io_path().join("file"), target.io_path().join("file")).unwrap();
            assert!(copy_and_verify(source, target).is_err());
            assert_eq!(fs::read(source.io_path().join("file")).unwrap(), b"keep");
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refuses_symbolic_links_in_either_tree() {
        use std::os::unix::fs::symlink;
        with_workspaces(|source, target| {
            let link = source.io_path().join("link");
            symlink(target.io_path(), &link).unwrap();
            assert!(copy_and_verify(source, target).is_err());
            fs::remove_file(link).unwrap();
            symlink(source.io_path(), target.io_path().join("link")).unwrap();
            assert!(copy_and_verify(source, target).is_err());
        });
    }
}
