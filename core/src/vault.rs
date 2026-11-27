//! 每个工作区一份口令封装的数据密钥。密钥材料只留在本进程，调用方拿不到。
//!
//! 口令用 PBKDF2-HMAC-SHA256 派生，数据密钥用 AES-256-GCM 封装。记录的关联数据
//! 绑定用户、工作区、类别、分段和偏移，因此换位置或换工作区后不能解出明文。
//! Windows DPAPI 不能把同一份密文带到另一台电脑，所以可迁移的保险库不使用它。
//! 文件权限仍由现有的私有目录约束。

use std::fs;
use std::io::{self, ErrorKind, Read};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::crypto::{aead_open, aead_seal, hmac_sha256, pbkdf2, random_bytes, wipe};
use crate::files::{self, open_existing};
use crate::platform::Anchor;
use crate::storage::Config;

const VAULT_NAME: &str = "vault.wsv";
const DEVICE_NAME: &str = "device.wsd";
const VAULT_MAGIC: &[u8; 4] = b"WSV1";
const VAULT_VERSION: u32 = 1;
pub(crate) const PBKDF2_ITERS: u32 = 600_000;
pub(crate) const SEAL_MAGIC: &[u8; 4] = b"WSE1";
pub(crate) const SEAL_OVERHEAD: usize = 32;
const MAX_AUTO_LOCK: u32 = 7 * 24 * 60 * 60;
const PASSWORD_CHECK: &[u8] = b"winstrata-password-check-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protection {
    pub encrypted: bool,
    pub locked: bool,
    pub auto_lock_secs: u32,
}

struct SealInner {
    dek: [u8; 32],
    user: String,
    workspace: String,
    locked: bool,
    auto_lock: Duration,
    auto_lock_secs: u32,
    last_activity: Instant,
}

impl Drop for SealInner {
    fn drop(&mut self) {
        wipe(&mut self.dek);
    }
}

#[derive(Clone)]
pub(crate) struct Seal {
    inner: Arc<Mutex<SealInner>>,
}

impl Seal {
    pub(crate) fn lock(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        guard.locked = true;
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .locked
    }

    pub(crate) fn user(&self) -> String {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .user
            .clone()
    }

    pub(crate) fn workspace(&self) -> String {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .workspace
            .clone()
    }

    pub(crate) fn auto_lock_secs(&self) -> u32 {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .auto_lock_secs
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub(crate) fn export_dek(&self) -> io::Result<[u8; 32]> {
        if self.is_locked() {
            return Err(locked_error());
        }
        Ok(self.copy_dek())
    }

    fn copy_dek(&self) -> [u8; 32] {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .dek
    }

    pub(crate) fn replace_dek(&self, dek: [u8; 32]) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        wipe(&mut guard.dek);
        guard.dek = dek;
        guard.locked = false;
        guard.last_activity = Instant::now();
    }

    pub(crate) fn authorize_read(&self) -> io::Result<()> {
        let mut dek = self.key_for(true)?;
        wipe(&mut dek);
        Ok(())
    }

    fn key_for(&self, user_facing: bool) -> io::Result<[u8; 32]> {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if user_facing {
            if !guard.auto_lock.is_zero() && guard.last_activity.elapsed() >= guard.auto_lock {
                guard.locked = true;
            }
            if guard.locked {
                return Err(locked_error());
            }
            guard.last_activity = Instant::now();
        }
        Ok(guard.dek)
    }

    pub(crate) fn pack(
        &self,
        category: u8,
        segment: u32,
        offset: u64,
        plain: &[u8],
        user_facing: bool,
    ) -> io::Result<Vec<u8>> {
        let mut dek = self.key_for(user_facing)?;
        let result = pack_with(
            &dek,
            &self.user(),
            &self.workspace(),
            category,
            segment,
            offset,
            plain,
        );
        wipe(&mut dek);
        result
    }

    pub(crate) fn unpack(
        &self,
        category: u8,
        segment: u32,
        offset: u64,
        stored: &[u8],
        user_facing: bool,
    ) -> io::Result<Vec<u8>> {
        let mut dek = self.key_for(user_facing)?;
        let result = unpack_with(
            &dek,
            &self.user(),
            &self.workspace(),
            category,
            segment,
            offset,
            stored,
        );
        wipe(&mut dek);
        result
    }
}

pub(crate) fn locked_error() -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, "storage is locked")
}

pub(crate) fn is_locked(error: &io::Error) -> bool {
    error.kind() == ErrorKind::PermissionDenied && error.to_string() == "storage is locked"
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn is_password_error(error: &io::Error) -> bool {
    error.kind() == ErrorKind::PermissionDenied && error.to_string() == "password is not valid"
}

fn input(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

pub(crate) fn check_password(password: &str) -> io::Result<()> {
    if (8..=1024).contains(&password.len()) {
        Ok(())
    } else {
        Err(input("password must be 8 to 1024 bytes"))
    }
}

pub(crate) fn check_auto_lock(seconds: u32) -> io::Result<()> {
    if seconds <= MAX_AUTO_LOCK {
        Ok(())
    } else {
        Err(input("auto-lock must be from 0 seconds through 7 days"))
    }
}

pub(crate) struct Header {
    pub(crate) auto_lock_secs: u32,
    pub(crate) device_id: [u8; 16],
    pub(crate) user: String,
    pub(crate) workspace: String,
}

pub(crate) fn exists(dir: &Anchor) -> io::Result<bool> {
    match fs::symlink_metadata(dir.io_path().join(VAULT_NAME)) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(invalid("vault is not a regular file")),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn peek(dir: &Anchor) -> io::Result<Option<Header>> {
    if !exists(dir)? {
        return Ok(None);
    }
    let vault = read_vault(dir)?;
    Ok(Some(Header {
        auto_lock_secs: vault.auto_lock_secs,
        device_id: vault.device_id,
        user: vault.user,
        workspace: vault.workspace,
    }))
}

pub(crate) fn create(
    dir: &Anchor,
    config: &Config,
    user: &str,
    password: &str,
    auto_lock_secs: u32,
    device_id: [u8; 16],
) -> io::Result<Seal> {
    check_password(password)?;
    check_auto_lock(auto_lock_secs)?;
    if exists(dir)? {
        return Err(input("encryption is already enabled"));
    }
    let mut dek = [0u8; 32];
    random_bytes(&mut dek)?;
    let seal = install(user, config.workspace_id(), dek, auto_lock_secs)?;
    if let Err(error) = write_vault(
        dir,
        user,
        config.workspace_id(),
        password,
        auto_lock_secs,
        &dek,
        device_id,
    ) {
        wipe(&mut dek);
        return Err(error);
    }
    wipe(&mut dek);
    Ok(seal)
}

pub(crate) fn unlock(
    dir: &Anchor,
    config: &Config,
    user: &str,
    password: &str,
) -> io::Result<Seal> {
    check_password(password)?;
    let vault = read_vault(dir)?;
    if vault.user != user || vault.workspace != config.workspace_id() {
        return Err(invalid("vault identity does not match this workspace"));
    }
    let mut keys = derive(password, &vault)?;
    if !keys.password_matches {
        wipe(&mut keys.kek);
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "password is not valid",
        ));
    }
    let mut dek = unwrap_dek(&keys.kek, &vault)?;
    wipe(&mut keys.kek);
    let seal = install(user, config.workspace_id(), dek, vault.auto_lock_secs)?;
    wipe(&mut dek);
    Ok(seal)
}

pub(crate) fn change_password(
    dir: &Anchor,
    seal: &Seal,
    user: &str,
    workspace: &str,
    current: &str,
    next: &str,
) -> io::Result<()> {
    check_password(current)?;
    check_password(next)?;
    if seal.is_locked() {
        return Err(locked_error());
    }
    let mut dek = confirm(dir, user, workspace, current)?;
    let seconds = seal.auto_lock_secs();
    let device = peek(dir)?
        .ok_or_else(|| invalid("vault is missing"))?
        .device_id;
    let result = write_vault(dir, user, workspace, next, seconds, &dek, device);
    wipe(&mut dek);
    result
}

pub(crate) fn rewrap(
    dir: &Anchor,
    user: &str,
    workspace: &str,
    password: &str,
    dek: &[u8; 32],
    auto_lock_secs: u32,
    device_id: [u8; 16],
) -> io::Result<()> {
    check_password(password)?;
    let vault = read_vault(dir)?;
    if vault.user != user || vault.workspace != workspace {
        return Err(invalid("vault identity does not match this workspace"));
    }
    let mut keys = derive(password, &vault)?;
    let matches = keys.password_matches;
    wipe(&mut keys.kek);
    if !matches {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "password is not valid",
        ));
    }
    write_vault(
        dir,
        user,
        workspace,
        password,
        auto_lock_secs,
        dek,
        device_id,
    )
}

pub(crate) fn device_id(dir: &Anchor, user: &str, workspace: &str) -> io::Result<[u8; 16]> {
    if let Some(header) = peek(dir)? {
        if header.user != user || header.workspace != workspace {
            return Err(invalid("vault identity does not match this workspace"));
        }
        return Ok(header.device_id);
    }
    let path = dir.io_path().join(DEVICE_NAME);
    match open_existing(&path, false) {
        Ok(mut file) => {
            let mut bytes = [0u8; 16];
            file.read_exact(&mut bytes)?;
            let mut extra = [0u8; 1];
            if file.read(&mut extra)? != 0 {
                return Err(invalid("device identity has an unexpected length"));
            }
            Ok(bytes)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut bytes = [0u8; 16];
            random_bytes(&mut bytes)?;
            files::write_encoded(dir, DEVICE_NAME, &bytes)?;
            Ok(bytes)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn decode_record(
    seal: Option<&Seal>,
    category: u8,
    segment: u32,
    offset: u64,
    stored: &[u8],
    user_facing: bool,
) -> io::Result<Vec<u8>> {
    let marked = stored.starts_with(SEAL_MAGIC);
    match (seal, marked) {
        (Some(seal), true) => seal.unpack(category, segment, offset, stored, user_facing),
        (None, false) => Ok(stored.to_vec()),
        (Some(_), false) => Err(invalid("encrypted store contains a plaintext record")),
        (None, true) => Err(invalid("sealed record has no key")),
    }
}

pub(crate) fn pack_with(
    dek: &[u8; 32],
    user: &str,
    workspace: &str,
    category: u8,
    segment: u32,
    offset: u64,
    plain: &[u8],
) -> io::Result<Vec<u8>> {
    if plain.len().saturating_add(SEAL_OVERHEAD) > crate::hybrid::RECORD_LIMIT {
        return Err(input("sealed record exceeds the 1 MiB storage limit"));
    }
    let mut nonce = [0u8; 12];
    random_bytes(&mut nonce)?;
    let aad = record_aad(user, workspace, category, segment, offset);
    let body = aead_seal(dek, &nonce, &aad, plain)?;
    let mut out = Vec::with_capacity(SEAL_OVERHEAD + plain.len());
    out.extend_from_slice(SEAL_MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&body);
    Ok(out)
}

pub(crate) fn unpack_with(
    dek: &[u8; 32],
    user: &str,
    workspace: &str,
    category: u8,
    segment: u32,
    offset: u64,
    stored: &[u8],
) -> io::Result<Vec<u8>> {
    if stored.len() < SEAL_OVERHEAD || stored.get(..4) != Some(SEAL_MAGIC) {
        return Err(invalid("sealed record failed authentication"));
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&stored[4..16]);
    let aad = record_aad(user, workspace, category, segment, offset);
    aead_open(dek, &nonce, &aad, &stored[16..])
}

fn install(user: &str, workspace: &str, dek: [u8; 32], auto_lock_secs: u32) -> io::Result<Seal> {
    Ok(Seal {
        inner: Arc::new(Mutex::new(SealInner {
            dek,
            user: user.to_owned(),
            workspace: workspace.to_owned(),
            locked: false,
            auto_lock: Duration::from_secs(u64::from(auto_lock_secs)),
            auto_lock_secs,
            last_activity: Instant::now(),
        })),
    })
}

struct Derived {
    kek: [u8; 32],
    password_matches: bool,
}

struct VaultFile {
    auto_lock_secs: u32,
    salt: [u8; 16],
    nonce: [u8; 12],
    verifier: [u8; 32],
    wrapped: [u8; 48],
    device_id: [u8; 16],
    user: String,
    workspace: String,
}

fn derive(password: &str, vault: &VaultFile) -> io::Result<Derived> {
    let mut kek = [0u8; 32];
    pbkdf2(password.as_bytes(), &vault.salt, PBKDF2_ITERS, &mut kek)?;
    let mac = hmac_sha256(&kek, PASSWORD_CHECK)?;
    Ok(Derived {
        kek,
        password_matches: eq(&mac, &vault.verifier),
    })
}

fn confirm(dir: &Anchor, user: &str, workspace: &str, password: &str) -> io::Result<[u8; 32]> {
    let vault = read_vault(dir)?;
    if vault.user != user || vault.workspace != workspace {
        return Err(invalid("vault identity does not match this workspace"));
    }
    let mut keys = derive(password, &vault)?;
    if !keys.password_matches {
        wipe(&mut keys.kek);
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "password is not valid",
        ));
    }
    let dek = unwrap_dek(&keys.kek, &vault)?;
    wipe(&mut keys.kek);
    Ok(dek)
}

fn unwrap_dek(kek: &[u8; 32], vault: &VaultFile) -> io::Result<[u8; 32]> {
    let plain = aead_open(
        kek,
        &vault.nonce,
        &vault_aad(&vault.user, &vault.workspace),
        &vault.wrapped,
    )?;
    if plain.len() != 32 {
        return Err(invalid("vault key has an unexpected length"));
    }
    let mut dek = [0u8; 32];
    dek.copy_from_slice(&plain);
    Ok(dek)
}

fn write_vault(
    dir: &Anchor,
    user: &str,
    workspace: &str,
    password: &str,
    auto_lock_secs: u32,
    dek: &[u8; 32],
    device_id: [u8; 16],
) -> io::Result<()> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    random_bytes(&mut salt)?;
    random_bytes(&mut nonce)?;
    let mut kek = [0u8; 32];
    pbkdf2(password.as_bytes(), &salt, PBKDF2_ITERS, &mut kek)?;
    let verifier = hmac_sha256(&kek, PASSWORD_CHECK)?;
    let wrapped = aead_seal(&kek, &nonce, &vault_aad(user, workspace), dek)?;
    wipe(&mut kek);
    if wrapped.len() != 48 {
        return Err(invalid("vault key wrap has an unexpected length"));
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(VAULT_MAGIC);
    put_u32(&mut bytes, VAULT_VERSION);
    put_u32(&mut bytes, PBKDF2_ITERS);
    put_u32(&mut bytes, auto_lock_secs);
    bytes.extend_from_slice(&salt);
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&verifier);
    bytes.extend_from_slice(&wrapped);
    bytes.extend_from_slice(&device_id);
    put_str(&mut bytes, user)?;
    put_str(&mut bytes, workspace)?;
    files::write_encoded(dir, VAULT_NAME, &bytes)
}

fn read_vault(dir: &Anchor) -> io::Result<VaultFile> {
    let mut file = open_existing(&dir.io_path().join(VAULT_NAME), false)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 || bytes.get(..4) != Some(VAULT_MAGIC) {
        return Err(invalid("vault is unreadable"));
    }
    let mut reader = bytes[4..].iter().copied();
    let version = take_u32(&mut reader)?;
    let iterations = take_u32(&mut reader)?;
    let auto_lock_secs = take_u32(&mut reader)?;
    if version != VAULT_VERSION || iterations != PBKDF2_ITERS || auto_lock_secs > MAX_AUTO_LOCK {
        return Err(invalid("vault parameters are not supported"));
    }
    let salt = take_array(&mut reader)?;
    let nonce = take_array(&mut reader)?;
    let verifier = take_array(&mut reader)?;
    let wrapped = take_array(&mut reader)?;
    let device_id = take_array(&mut reader)?;
    let user = take_str(&mut reader, 256)?;
    let workspace = take_str(&mut reader, 64)?;
    if reader.next().is_some() {
        return Err(invalid("vault contains trailing bytes"));
    }
    Ok(VaultFile {
        auto_lock_secs,
        salt,
        nonce,
        verifier,
        wrapped,
        device_id,
        user,
        workspace,
    })
}

fn record_aad(user: &str, workspace: &str, category: u8, segment: u32, offset: u64) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(b"winstrata-record-v1");
    aad.push(0);
    aad.extend_from_slice(user.as_bytes());
    aad.push(0);
    aad.extend_from_slice(workspace.as_bytes());
    aad.push(0);
    aad.push(category);
    aad.extend_from_slice(&segment.to_le_bytes());
    aad.extend_from_slice(&offset.to_le_bytes());
    aad
}

fn vault_aad(user: &str, workspace: &str) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(b"winstrata-vault-v1");
    aad.push(0);
    aad.extend_from_slice(user.as_bytes());
    aad.push(0);
    aad.extend_from_slice(workspace.as_bytes());
    aad
}

fn eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, value: &str) -> io::Result<()> {
    let len = u32::try_from(value.len()).map_err(|_| input("vault identity is too long"))?;
    put_u32(out, len);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn take_u32(bytes: &mut impl Iterator<Item = u8>) -> io::Result<u32> {
    let raw = take_array::<4>(bytes)?;
    Ok(u32::from_le_bytes(raw))
}

fn take_array<const N: usize>(bytes: &mut impl Iterator<Item = u8>) -> io::Result<[u8; N]> {
    let mut out = [0u8; N];
    for slot in &mut out {
        *slot = bytes.next().ok_or_else(|| invalid("vault is unreadable"))?;
    }
    Ok(out)
}

fn take_str(bytes: &mut impl Iterator<Item = u8>, max: usize) -> io::Result<String> {
    let len = take_u32(bytes)? as usize;
    if len > max {
        return Err(invalid("vault identity is too long"));
    }
    let mut raw = Vec::with_capacity(len);
    for _ in 0..len {
        raw.push(bytes.next().ok_or_else(|| invalid("vault is unreadable"))?);
    }
    String::from_utf8(raw).map_err(|_| invalid("vault identity is not UTF-8"))
}
