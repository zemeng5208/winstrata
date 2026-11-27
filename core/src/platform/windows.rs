mod security;

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::Arc;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF, FILE_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE,
    OBJ_DONT_REPARSE, RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileAttributeTagInfo, FileIdInfo,
    GetDiskFreeSpaceExW, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
    GetVolumeInformationByHandleW, OPEN_EXISTING, READ_CONTROL, SYNCHRONIZE, VOLUME_NAME_GUID,
};
use windows_sys::Win32::System::Com::{CoCreateGuid, CoTaskMemFree};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::SystemServices::FILE_PERSISTENT_ACLS;
use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};

use security::PrivateAccess;

fn check(ok: i32) -> io::Result<()> {
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

// 调用方保证系统返回有效的 NUL 结尾 UTF-16 字符串，读取期间保持分配存活。
unsafe fn wide_os(raw: *const u16) -> OsString {
    let mut len = 0;
    unsafe {
        while *raw.add(len) != 0 {
            len += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(raw, len))
    }
}

pub(crate) fn current_id() -> io::Result<String> {
    security::current_id()
}

pub(crate) fn fresh_id() -> io::Result<String> {
    let mut guid = windows_sys::core::GUID::default();
    let result = unsafe { CoCreateGuid(&mut guid) };
    if result < 0 {
        return Err(io::Error::other(format!(
            "GUID generation failed (HRESULT {result:#x})"
        )));
    }
    Ok(format!(
        "{:08x}{:04x}{:04x}{:016x}",
        guid.data1,
        guid.data2,
        guid.data3,
        u64::from_be_bytes(guid.data4)
    ))
}

pub(crate) fn private_file_create(path: &Path) -> io::Result<File> {
    let mut name: Vec<_> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(invalid_path());
    }
    name.push(0);
    let access = PrivateAccess::current(false)?;
    let attributes = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: access.descriptor(),
        bInheritHandle: 0,
    };
    // 调用方持有私有父目录句柄；首次创建即设置所有者与 DACL，保留临时文件重命名能力。
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(raw) })
}

pub(crate) fn verify_private_file(file: &File) -> io::Result<()> {
    security::verify_file(file.as_raw_handle())
}

pub(crate) fn locator_parent() -> io::Result<PathBuf> {
    let mut raw = null_mut();
    // NULL 令牌代表当前用户；Shell 分配的返回缓冲无论成功失败都按其约定释放。
    let result = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut raw) };
    let path = if result < 0 {
        Err(io::Error::other(format!(
            "LocalAppData lookup failed (HRESULT {result:#x})"
        )))
    } else if raw.is_null() {
        Err(io::Error::other("LocalAppData lookup returned no folder"))
    } else {
        Ok(PathBuf::from(unsafe { wide_os(raw) }))
    };
    unsafe { CoTaskMemFree(raw.cast()) };
    path
}

fn invalid_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "expected an absolute local drive path with ordinary Windows names",
    )
}

fn valid_name(name: &[u16]) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 255
        || matches!(name.last(), Some(32 | 46))
        || name
            .iter()
            .any(|&n| n < 32 || matches!(n, 34 | 42 | 47 | 58 | 60 | 62 | 63 | 92 | 124))
    {
        return Err(invalid_path());
    }
    // Win32 设备名带扩展名仍保留；数字设备别名还涵盖三个上标数字。
    let text = String::from_utf16_lossy(name);
    let stem = text
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    if matches!(
        stem.as_str(),
        "AUX" | "CON" | "CONIN$" | "CONOUT$" | "NUL" | "PRN"
    ) || numbered.is_some_and(|n| {
        matches!(
            n,
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    }) {
        return Err(invalid_path());
    }
    Ok(())
}

#[derive(Debug)]
struct Pin {
    handle: OwnedHandle,
    _parent: Option<Arc<Pin>>,
}

#[derive(Clone, Debug)]
pub(crate) struct Anchor {
    pin: Arc<Pin>,
    _leaf_guard: Option<Arc<OwnedHandle>>,
    ntfs: bool,
    path: PathBuf,
}

const OPEN_RIGHTS: u32 = FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;

pub(crate) fn file_key(file: &File) -> io::Result<(u64, u128)> {
    handle_key(file.as_raw_handle())
}

fn handle_key(handle: HANDLE) -> io::Result<(u64, u128)> {
    let mut id = FILE_ID_INFO::default();
    // 在持有对象句柄期间读取完整 128 位文件标识，避免丢失 ReFS 的标识高位。
    unsafe {
        check(GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut id as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        ))?;
    }
    Ok((
        id.VolumeSerialNumber,
        u128::from_le_bytes(id.FileId.Identifier),
    ))
}

pub(super) fn retry_unknown_revision<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 0;
    loop {
        match op() {
            Ok(value) => return Ok(value),
            // 1305 是安全描述符修订暂时读不出来。ACE 内容仍按原条件拒绝，不放宽。
            Err(error) if error.raw_os_error() == Some(1305) && attempt < 3 => {
                std::thread::sleep(std::time::Duration::from_millis(1 << attempt));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

// NTFS 只允许在空目录上建立重解析点。后代持有直接子项的防删除句柄后，祖先无法变空，
// 只需保留共享写句柄。非私有目录作为叶节点时仍保留严格句柄，供检查和下一次相对打开。
fn select_pins(
    first: OwnedHandle,
    relaxed: Option<OwnedHandle>,
    private: bool,
) -> io::Result<(OwnedHandle, Option<Arc<OwnedHandle>>)> {
    let Some(relaxed) = relaxed else {
        return Ok((first, None));
    };
    if handle_key(first.as_raw_handle())? != handle_key(relaxed.as_raw_handle())? {
        return Err(io::Error::other("directory identity changed"));
    }
    Ok((relaxed, (!private).then(|| Arc::new(first))))
}

fn verify_directory(handle: HANDLE) -> io::Result<()> {
    let mut attrs = FILE_ATTRIBUTE_TAG_INFO::default();
    unsafe {
        check(GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            (&mut attrs as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        ))?;
    }
    if attrs.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || attrs.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a directory without a reparse point",
        ));
    }
    Ok(())
}

fn volume_root(handle: HANDLE) -> io::Result<(PathBuf, bool)> {
    let mut flags = 0;
    let mut filesystem = [0u16; 32];
    unsafe {
        check(GetVolumeInformationByHandleW(
            handle,
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        ))?;
    }
    if flags & FILE_PERSISTENT_ACLS == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "persistent filesystem ACLs are required",
        ));
    }
    // 用已打开的卷句柄获取稳定地址，避免盘符重新映射。只接受本地卷根，不接受 SUBST 或网络别名。
    let mut buffer = [0u16; 128];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            handle,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            VOLUME_NAME_GUID,
        )
    };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let name = buffer.get(..len as usize).ok_or_else(invalid_path)?;
    let text = String::from_utf16(name).map_err(|_| invalid_path())?;
    let guid = text
        .strip_prefix(r"\\?\Volume{")
        .and_then(|name| name.strip_suffix(r"}\"));
    if !guid
        .is_some_and(|id| id.len() == 36 && id.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'))
    {
        return Err(invalid_path());
    }
    let len = filesystem
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(filesystem.len());
    let ntfs = String::from_utf16_lossy(&filesystem[..len]).eq_ignore_ascii_case("NTFS");
    Ok((PathBuf::from(text), ntfs))
}

impl Anchor {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let units: Vec<_> = path.as_os_str().encode_wide().collect();
        let separator = |c: &u16| matches!(*c, 47 | 92);
        if units.len() < 3
            || !matches!(units[0], 65..=90 | 97..=122)
            || units[1] != 58
            || !separator(&units[2])
        {
            return Err(invalid_path());
        }
        let root = [units[0], 58, 92, 0];
        let open = |share| -> io::Result<OwnedHandle> {
            let raw = unsafe {
                CreateFileW(
                    root.as_ptr(),
                    OPEN_RIGHTS,
                    share,
                    null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    null_mut(),
                )
            };
            if raw == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
        };
        let first = open(FILE_SHARE_READ)?;
        verify_directory(first.as_raw_handle())?;
        let (path, ntfs) = volume_root(first.as_raw_handle())?;
        let relaxed = ntfs
            .then(|| open(FILE_SHARE_READ | FILE_SHARE_WRITE))
            .transpose()?;
        let (handle, guard) = select_pins(first, relaxed, false)?;
        let mut anchor = Self {
            pin: Arc::new(Pin {
                handle,
                _parent: None,
            }),
            _leaf_guard: guard,
            ntfs,
            path,
        };
        let tail = &units[3..];
        if !tail.is_empty() {
            let tail = if separator(tail.last().unwrap()) {
                &tail[..tail.len() - 1]
            } else {
                tail
            };
            for name in tail.split(separator) {
                anchor = anchor.child(&OsString::from_wide(name), false, false)?;
            }
        }
        Ok(anchor)
    }

    pub(crate) fn child(&self, name: &OsStr, create: bool, private: bool) -> io::Result<Self> {
        retry_unknown_revision(|| self.open_child(name, create, private))
    }

    fn open_child(&self, name: &OsStr, create: bool, private: bool) -> io::Result<Self> {
        let mut units: Vec<_> = name.encode_wide().collect();
        valid_name(&units)?;
        let length = (units.len() * 2) as u16;
        let component = UNICODE_STRING {
            Length: length,
            MaximumLength: length,
            Buffer: units.as_mut_ptr(),
        };
        let access = private.then(|| PrivateAccess::current(true)).transpose()?;
        let attributes = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: self.pin.handle.as_raw_handle(),
            ObjectName: &component,
            Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
            SecurityDescriptor: access.as_ref().map_or(null(), |sd| sd.descriptor().cast()),
            SecurityQualityOfService: null(),
        };
        // 用同一个已固定父目录和经检查的单个名称打开；原子创建时即应用描述符。
        let open = |disposition, share| -> io::Result<OwnedHandle> {
            let mut result = IO_STATUS_BLOCK::default();
            let mut raw = null_mut();
            let status = unsafe {
                NtCreateFile(
                    &mut raw,
                    OPEN_RIGHTS | if private { READ_CONTROL } else { 0 },
                    &attributes,
                    &mut result,
                    null(),
                    0,
                    share,
                    disposition,
                    FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                    null(),
                    0,
                )
            };
            if status < 0 {
                return Err(io::Error::from_raw_os_error(
                    unsafe { RtlNtStatusToDosError(status) } as i32,
                ));
            }
            if raw.is_null() {
                return Err(io::Error::other("directory open returned no handle"));
            }
            Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
        };
        let first = open(
            if create { FILE_OPEN_IF } else { FILE_OPEN },
            FILE_SHARE_READ,
        )?;
        verify_directory(first.as_raw_handle())?;
        if let Some(access) = access {
            access.verify(first.as_raw_handle())?;
        }
        let relaxed = (private || self.ntfs)
            .then(|| open(FILE_OPEN, FILE_SHARE_READ | FILE_SHARE_WRITE))
            .transpose()?;
        let (handle, guard) = select_pins(first, relaxed, private)?;
        verify_directory(self.pin.handle.as_raw_handle())?;
        Ok(Self {
            pin: Arc::new(Pin {
                handle,
                _parent: Some(Arc::clone(&self.pin)),
            }),
            _leaf_guard: guard,
            ntfs: self.ntfs,
            path: self.path.join(name),
        })
    }

    pub(crate) fn same_directory(&self, other: &Self) -> io::Result<bool> {
        Ok(handle_key(self.pin.handle.as_raw_handle())?
            == handle_key(other.pin.handle.as_raw_handle())?)
    }

    pub(crate) fn io_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn available(&self) -> io::Result<u64> {
        let path: Vec<u16> = self.path.as_os_str().encode_wide().chain([0]).collect();
        let mut available = 0;
        // 第一项是当前调用者配额内的可用空间，可以小于卷的总空闲字节。
        unsafe {
            check(GetDiskFreeSpaceExW(
                path.as_ptr(),
                &mut available,
                null_mut(),
                null_mut(),
            ))?
        };
        Ok(available)
    }
}

#[cfg(test)]
mod tests;
