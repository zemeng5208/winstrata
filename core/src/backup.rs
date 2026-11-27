//! 完整与增量备份。包内保存用户目录的普通文件和校验值，不解密载荷。
//!
//! 增量包只包含相对上一份完整备份发生变化的文件，并记录来源设备标识。
//! 恢复拒绝路径越界、符号链接和另一个用户的包。插件导出与时间范围裁剪不属于这里。

use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::files::{create_private, open_existing};
use crate::hybrid::{self, content_crc};
use crate::platform::Anchor;
use crate::storage::{self, Config};

const MAGIC: &[u8; 4] = b"WSB1";
const VERSION: u32 = 1;
const FULL: u32 = 1;
const INCREMENTAL: u32 = 2;
const MAX_FILES: usize = 100_000;
const MAX_PATH: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupKind {
    Full,
    Incremental,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupReport {
    pub kind: BackupKind,
    pub commit: u64,
    pub base_commit: u64,
    pub device_id: [u8; 16],
    pub files: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreReport {
    pub commit: u64,
    pub source_device: [u8; 16],
    pub files: u64,
}

#[derive(Clone)]
struct Item {
    path: String,
    offset: u64,
    len: u64,
    crc: u32,
    bytes: Vec<u8>,
}

struct Package {
    kind: u32,
    encrypted: bool,
    created_ns: u64,
    device_id: [u8; 16],
    commit: u64,
    base_commit: u64,
    user: String,
    workspace: String,
    files: Vec<Item>,
    deletions: Vec<String>,
}

pub(crate) struct Identity {
    pub(crate) user: String,
    pub(crate) workspace: String,
    pub(crate) device_id: [u8; 16],
    pub(crate) encrypted: bool,
}

pub(crate) fn write_full(
    dir: &Anchor,
    package: &Path,
    identity: &Identity,
) -> io::Result<BackupReport> {
    ensure_outside(dir, package)?;
    let files = snapshot(dir)?;
    let manifest = Package {
        kind: FULL,
        encrypted: identity.encrypted,
        created_ns: now_ns(),
        device_id: identity.device_id,
        commit: hybrid::journal_commit(dir)?,
        base_commit: 0,
        user: identity.user.clone(),
        workspace: identity.workspace.clone(),
        files,
        deletions: Vec::new(),
    };
    store(package, &manifest)
}

pub(crate) fn write_incremental(
    dir: &Anchor,
    package: &Path,
    base: &Path,
    identity: &Identity,
) -> io::Result<BackupReport> {
    ensure_outside(dir, package)?;
    let previous = load(base)?;
    if previous.kind != FULL
        || previous.user != identity.user
        || previous.workspace != identity.workspace
    {
        return Err(invalid(
            "incremental backup requires a full backup of this workspace",
        ));
    }
    let current = snapshot(dir)?;
    let commit = hybrid::journal_commit(dir)?;
    if commit < previous.commit {
        return Err(invalid("the workspace commit moved backwards"));
    }
    let mut files = Vec::new();
    let mut deletions = Vec::new();
    for item in &previous.files {
        if !current.iter().any(|file| file.path == item.path) {
            deletions.push(item.path.clone());
        }
    }
    for item in current {
        let unchanged = previous
            .files
            .iter()
            .any(|file| file.path == item.path && file.len == item.len && file.crc == item.crc);
        if !unchanged {
            files.push(item);
        }
    }
    let manifest = Package {
        kind: INCREMENTAL,
        encrypted: identity.encrypted,
        created_ns: now_ns(),
        device_id: identity.device_id,
        commit,
        base_commit: previous.commit,
        user: identity.user.clone(),
        workspace: identity.workspace.clone(),
        files,
        deletions,
    };
    store(package, &manifest)
}

pub(crate) fn verify(package: &Path) -> io::Result<BackupReport> {
    let manifest = load(package)?;
    report(&manifest)
}

pub(crate) fn restore_full(root: &Path, user: &str, package: &Path) -> io::Result<RestoreReport> {
    let manifest = load(package)?;
    if manifest.kind != FULL {
        return Err(input("this package is not a full backup"));
    }
    if manifest.user != user {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "backup belongs to a different user",
        ));
    }
    check_core(&manifest)?;
    let dir = storage::data_dir(root, user, true)?;
    if dir_not_empty(&dir)? {
        return Err(input("restore needs an empty user directory"));
    }
    let bytes = manifest
        .files
        .iter()
        .fold(0u64, |sum, file| sum.saturating_add(file.len));
    if dir.available()? < bytes.max(1) {
        return Err(io::Error::new(
            ErrorKind::StorageFull,
            "insufficient capacity for restore",
        ));
    }
    write_tree(&dir, &manifest.files)?;
    let config = Config::load(&dir, user)?;
    if config.workspace_id() != manifest.workspace {
        return Err(invalid(
            "restored workspace identifier does not match the package",
        ));
    }
    Ok(RestoreReport {
        commit: manifest.commit,
        source_device: manifest.device_id,
        files: manifest.files.len() as u64,
    })
}

pub(crate) fn restore_incremental(
    root: &Path,
    user: &str,
    base: &Path,
    package: &Path,
) -> io::Result<RestoreReport> {
    let base = load(base)?;
    let manifest = load(package)?;
    if base.kind != FULL || manifest.kind != INCREMENTAL {
        return Err(input(
            "incremental restore needs a full base and an incremental package",
        ));
    }
    if manifest.user != user || base.user != user || manifest.workspace != base.workspace {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "backup belongs to a different user",
        ));
    }
    if manifest.base_commit != base.commit {
        return Err(invalid(
            "incremental backup does not continue this full backup",
        ));
    }
    let dir = storage::data_dir(root, user, false)?;
    for item in &base.files {
        let path = file_path(&dir, &item.path)?;
        let current = fs::read(&path)
            .map_err(|_| invalid("restore destination does not match the base backup"))?;
        if current.len() as u64 != item.len || content_crc(&current) != item.crc {
            return Err(invalid(
                "restore destination does not match the base backup",
            ));
        }
    }
    for deletion in &manifest.deletions {
        if !base.files.iter().any(|file| file.path == *deletion) {
            return Err(invalid(
                "incremental backup deletes a file outside its base",
            ));
        }
    }
    let bytes = manifest
        .files
        .iter()
        .fold(0u64, |sum, file| sum.saturating_add(file.len));
    if dir.available()? < bytes.max(1) {
        return Err(io::Error::new(
            ErrorKind::StorageFull,
            "insufficient capacity for restore",
        ));
    }
    write_tree(&dir, &manifest.files)?;
    for deletion in &manifest.deletions {
        let path = file_path(&dir, deletion)?;
        fs::remove_file(path)?;
    }
    let config = Config::load(&dir, user)?;
    if config.workspace_id() != manifest.workspace {
        return Err(invalid(
            "restored workspace identifier does not match the package",
        ));
    }
    Ok(RestoreReport {
        commit: manifest.commit,
        source_device: manifest.device_id,
        files: manifest.files.len() as u64,
    })
}

fn report(manifest: &Package) -> io::Result<BackupReport> {
    Ok(BackupReport {
        kind: if manifest.kind == FULL {
            BackupKind::Full
        } else {
            BackupKind::Incremental
        },
        commit: manifest.commit,
        base_commit: manifest.base_commit,
        device_id: manifest.device_id,
        files: manifest.files.len() as u64,
        bytes: manifest
            .files
            .iter()
            .fold(0, |sum, file| sum.saturating_add(file.len)),
    })
}

fn store(package: &Path, manifest: &Package) -> io::Result<BackupReport> {
    if package.exists() {
        return Err(input("backup package already exists"));
    }
    let mut payload = Vec::new();
    let mut files = Vec::new();
    for item in &manifest.files {
        let offset = payload.len() as u64;
        payload.extend_from_slice(&item.bytes);
        files.push(Item {
            path: item.path.clone(),
            offset,
            len: item.len,
            crc: item.crc,
            bytes: Vec::new(),
        });
    }
    let mut header = Vec::new();
    put_u32(&mut header, manifest.kind);
    put_u32(&mut header, u32::from(manifest.encrypted));
    put_u64(&mut header, manifest.created_ns);
    header.extend_from_slice(&manifest.device_id);
    put_u64(&mut header, manifest.commit);
    put_u64(&mut header, manifest.base_commit);
    put_str(&mut header, &manifest.user)?;
    put_str(&mut header, &manifest.workspace)?;
    put_u32(&mut header, files.len() as u32);
    for file in &files {
        put_str(&mut header, &file.path)?;
        put_u64(&mut header, file.offset);
        put_u64(&mut header, file.len);
        put_u32(&mut header, file.crc);
    }
    put_u32(&mut header, manifest.deletions.len() as u32);
    for path in &manifest.deletions {
        put_str(&mut header, path)?;
    }
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    put_u32(&mut encoded, VERSION);
    put_u32(&mut encoded, header.len() as u32);
    encoded.extend_from_slice(&header);
    encoded.extend_from_slice(&payload);
    let mut file = create_private(package)?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    let mut stored = manifest.clone_header();
    stored.files = files;
    report(&stored)
}

fn load(package: &Path) -> io::Result<Package> {
    let mut file = open_existing(package, false)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if bytes.len() < 12 || bytes.get(..4) != Some(MAGIC) {
        return Err(invalid("backup package is unreadable"));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes"));
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes")) as usize;
    if version != VERSION {
        return Err(invalid("backup package version is not supported"));
    }
    let header_end = 12usize
        .checked_add(header_len)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| invalid("backup package is unreadable"))?;
    let mut reader = Reader::new(&bytes[12..header_end]);
    let kind = reader.u32()?;
    let flags = reader.u32()?;
    if !matches!(kind, FULL | INCREMENTAL) || flags > 1 {
        return Err(invalid("backup package flags are not supported"));
    }
    let created_ns = reader.u64()?;
    let device_id = reader.array()?;
    let commit = reader.u64()?;
    let base_commit = reader.u64()?;
    let user = reader.str(256)?;
    let workspace = reader.str(64)?;
    let count = reader.u32()? as usize;
    if count > MAX_FILES {
        return Err(invalid("backup package lists too many files"));
    }
    let mut listed = Vec::with_capacity(count);
    let mut cursor = 0u64;
    for _ in 0..count {
        let path = reader.str(MAX_PATH)?;
        validate_relative(&path)?;
        let offset = reader.u64()?;
        let len = reader.u64()?;
        let crc = reader.u32()?;
        if offset != cursor {
            return Err(invalid("backup payload ranges are not contiguous"));
        }
        cursor = cursor
            .checked_add(len)
            .ok_or_else(|| invalid("backup payload is too large"))?;
        listed.push((path, offset, len, crc));
    }
    let deletions_len = reader.u32()? as usize;
    if deletions_len > MAX_FILES {
        return Err(invalid("backup package lists too many deletions"));
    }
    let mut deletions = Vec::with_capacity(deletions_len);
    for _ in 0..deletions_len {
        let path = reader.str(MAX_PATH)?;
        validate_relative(&path)?;
        deletions.push(path);
    }
    reader.finish()?;
    if bytes.len() - header_end != cursor as usize {
        return Err(invalid("backup payload length does not match its header"));
    }
    let payload = &bytes[header_end..];
    let mut files = Vec::with_capacity(listed.len());
    for (path, offset, len, crc) in listed {
        let start = offset as usize;
        let end = start + len as usize;
        let data = &payload[start..end];
        if content_crc(data) != crc {
            return Err(invalid("backup file checksum does not match"));
        }
        files.push(Item {
            path,
            offset,
            len,
            crc,
            bytes: data.to_vec(),
        });
    }
    Ok(Package {
        kind,
        encrypted: flags == 1,
        created_ns,
        device_id,
        commit,
        base_commit,
        user,
        workspace,
        files,
        deletions,
    })
}

fn snapshot(dir: &Anchor) -> io::Result<Vec<Item>> {
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn walk(dir: &Anchor, prefix: &str, files: &mut Vec<Item>) -> io::Result<()> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir.io_path())? {
        names.push(entry?.file_name());
    }
    names.sort();
    for name in names {
        let text = name
            .to_str()
            .filter(|text| {
                !text.is_empty() && !text.starts_with('.') && !text.contains(['/', '\\', '\0', ':'])
            })
            .ok_or_else(|| invalid("backup found a file name it cannot store"))?;
        let path = dir.io_path().join(&name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("backup does not follow links"));
        }
        if metadata.is_dir() {
            let child = dir.child(&name, false, true)?;
            let next = if prefix.is_empty() {
                format!("{text}/")
            } else {
                format!("{prefix}{text}/")
            };
            walk(&child, &next, files)?;
        } else if metadata.is_file() {
            let mut input = open_existing(&path, false)?;
            let mut bytes = Vec::new();
            input.read_to_end(&mut bytes)?;
            if files.len() == MAX_FILES {
                return Err(invalid("backup contains too many files"));
            }
            let relative = format!("{prefix}{text}");
            validate_relative(&relative)?;
            files.push(Item {
                crc: content_crc(&bytes),
                len: bytes.len() as u64,
                path: relative,
                offset: 0,
                bytes,
            });
        } else {
            return Err(invalid("backup requires regular files and directories"));
        }
    }
    Ok(())
}

fn write_tree(dir: &Anchor, files: &[Item]) -> io::Result<()> {
    for file in files {
        validate_relative(&file.path)?;
        let mut parts = file.path.split('/');
        let name = parts
            .next_back()
            .ok_or_else(|| invalid("backup path is absolute or contains traversal"))?;
        let mut nested = None;
        for part in parts {
            let parent = nested.as_ref().unwrap_or(dir);
            nested = Some(parent.child(std::ffi::OsStr::new(part), true, true)?);
        }
        let parent = nested.as_ref().unwrap_or(dir);
        let path = parent.io_path().join(name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        let mut output = create_private(&path)?;
        output.write_all(&file.bytes)?;
        output.sync_all()?;
    }
    Ok(())
}

fn file_path(dir: &Anchor, relative: &str) -> io::Result<PathBuf> {
    validate_relative(relative)?;
    let mut path = dir.io_path().to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    let root = dir.io_path();
    if !path.starts_with(root) {
        return Err(invalid("backup path escapes the user directory"));
    }
    Ok(path)
}

fn validate_relative(path: &str) -> io::Result<()> {
    if path.is_empty()
        || path.len() > MAX_PATH
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains(['\\', '\0', ':'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid("backup path is absolute or contains traversal"));
    }
    Ok(())
}

fn check_core(manifest: &Package) -> io::Result<()> {
    let core = manifest
        .files
        .iter()
        .find(|file| file.path == "core.json")
        .ok_or_else(|| invalid("backup does not contain the workspace configuration"))?;
    let head: CoreHead = serde_json::from_slice(&core.bytes)
        .map_err(|_| invalid("backup configuration is invalid"))?;
    if head.version != 1 || head.owner != manifest.user || head.workspace_id != manifest.workspace {
        return Err(invalid("backup configuration does not match its manifest"));
    }
    Ok(())
}

fn ensure_outside(dir: &Anchor, package: &Path) -> io::Result<()> {
    if package.starts_with(dir.io_path()) {
        return Err(input("backup package must stay outside the user directory"));
    }
    Ok(())
}

fn dir_not_empty(dir: &Anchor) -> io::Result<bool> {
    Ok(fs::read_dir(dir.io_path())?.next().transpose()?.is_some())
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn input(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, value: &str) -> io::Result<()> {
    let len = u32::try_from(value.len()).map_err(|_| input("backup text is too long"))?;
    put_u32(out, len);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn u32(&mut self) -> io::Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
    }

    fn u64(&mut self) -> io::Result<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
    }

    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn str(&mut self, max: usize) -> io::Result<String> {
        let len = self.u32()? as usize;
        if len > max {
            return Err(invalid("backup text is too long"));
        }
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid("backup text is not UTF-8"))
    }

    fn finish(&self) -> io::Result<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(invalid("backup header contains trailing bytes"))
        }
    }

    fn take(&mut self, len: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| invalid("backup package is unreadable"))?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
}

impl Package {
    fn clone_header(&self) -> Self {
        Self {
            kind: self.kind,
            encrypted: self.encrypted,
            created_ns: self.created_ns,
            device_id: self.device_id,
            commit: self.commit,
            base_commit: self.base_commit,
            user: self.user.clone(),
            workspace: self.workspace.clone(),
            files: Vec::new(),
            deletions: self.deletions.clone(),
        }
    }
}

#[derive(Deserialize)]
struct CoreHead {
    version: u32,
    owner: String,
    workspace_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn traversal_is_rejected_before_any_restore_write() {
        let temp = std::env::temp_dir().join(format!(
            "winstrata-backup-{}",
            crate::platform::fresh_id().unwrap()
        ));
        fs::create_dir(&temp).unwrap();
        let package = temp.join("bad.wsb");
        let mut header = Vec::new();
        put_u32(&mut header, FULL);
        put_u32(&mut header, 0);
        put_u64(&mut header, 1);
        header.extend_from_slice(&[7u8; 16]);
        put_u64(&mut header, 1);
        put_u64(&mut header, 0);
        put_str(&mut header, "user").unwrap();
        put_str(&mut header, "workspace-id").unwrap();
        put_u32(&mut header, 1);
        put_str(&mut header, "../escaped.txt").unwrap();
        put_u64(&mut header, 0);
        put_u64(&mut header, 4);
        put_u32(&mut header, content_crc(b"nope"));
        put_u32(&mut header, 0);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        put_u32(&mut encoded, VERSION);
        put_u32(&mut encoded, header.len() as u32);
        encoded.extend_from_slice(&header);
        encoded.extend_from_slice(b"nope");
        let mut file = crate::files::create_private(&package).unwrap();
        file.write_all(&encoded).unwrap();
        file.sync_all().unwrap();
        let error = restore_full(&temp, "user", &package).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!temp.join("escaped.txt").exists());
        assert!(!temp.join("user").exists());
        fs::remove_dir_all(&temp).unwrap();
    }
}
