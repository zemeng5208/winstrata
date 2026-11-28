//! Windows 上的插件私有文件。
//!
//! 描述符与 Core 的私有文件相同：所有者是当前用户，DACL 受保护，
//! 并且只有这一条不继承的完整控制。提升后的令牌默认会把新建文件
//! 的所有者写成 Administrators。Core 的 `open_existing` 会拒绝这种文件。
//! 这里在创建时写明所有者和 DACL，不放宽那项检查。

use std::fs::File;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{self, null_mut};

use windows_sys::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetSecurityDescriptorControl,
    GetSidIdentifierAuthority, GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation,
    INHERITED_ACE, OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_DISK,
    GetFileInformationByHandle, GetFileType, OPEN_EXISTING,
};
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

struct LocalBlock(*mut core::ffi::c_void);

impl Drop for LocalBlock {
    fn drop(&mut self) {
        // 指针来自要求调用 LocalFree 的 Windows 接口。
        unsafe { LocalFree(self.0) };
    }
}

struct User {
    buffer: Vec<usize>,
}

impl User {
    fn current() -> io::Result<Self> {
        let mut raw = null_mut();
        // 进程伪句柄不需要关闭。成功返回的令牌由 OwnedHandle 关闭。
        unsafe { check(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw))? };
        let token = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut bytes = 0;
        let first = unsafe {
            GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut bytes)
        };
        let error = io::Error::last_os_error();
        if first != 0 || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
            return Err(error);
        }
        if (bytes as usize) < size_of::<TOKEN_USER>() {
            return Err(io::Error::other("incomplete Windows token information"));
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        unsafe {
            check(GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                buffer.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
            ))?;
        }
        Ok(Self { buffer })
    }

    fn sid(&self) -> PSID {
        // 缓冲来自 TokenUser，且在这次比较结束前不再改变大小。
        unsafe { (*self.buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn text(&self) -> io::Result<String> {
        let mut raw = null_mut();
        unsafe { check(ConvertSidToStringSidW(self.sid(), &mut raw))? };
        let _allocation = LocalBlock(raw.cast());
        let mut len = 0usize;
        unsafe {
            while *raw.add(len) != 0 {
                len += 1;
            }
            String::from_utf16(std::slice::from_raw_parts(raw, len))
                .map_err(|_| io::Error::other("invalid SID text"))
        }
    }
}

fn private_descriptor(user: &User) -> io::Result<LocalBlock> {
    let sid = user.text()?;
    let sddl = format!("O:{sid}D:P(A;;FA;;;{sid})");
    if sddl.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows string contains NUL",
        ));
    }
    let wide: Vec<u16> = sddl.encode_utf16().chain([0]).collect();
    let mut raw = null_mut();
    // 显式所有者是当前用户。受保护的 DACL 不再并入父目录的继承项。
    unsafe {
        check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut raw,
            null_mut(),
        ))?;
    }
    Ok(LocalBlock(raw))
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let user = User::current()?;
    let descriptor = private_descriptor(&user)?;
    match create_new(path, descriptor.0) {
        Ok(mut file) => {
            if let Err(error) = verify_file(&user, &file) {
                drop(file);
                let _ = std::fs::remove_file(path);
                return Err(error);
            }
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let mut file = open_existing(path)?;
            verify_file(&user, &file)?;
            file.set_len(0)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn create_new(path: &Path, descriptor: *mut core::ffi::c_void) -> io::Result<File> {
    let name = wide_path(path)?;
    let attributes = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
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

fn open_existing(path: &Path) -> io::Result<File> {
    let name = wide_path(path)?;
    let raw = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(raw) })
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows path contains NUL",
        ));
    }
    name.push(0);
    Ok(name)
}

fn verify_file(user: &User, file: &File) -> io::Result<()> {
    let mut attempt = 0;
    loop {
        match verify_once(user, file) {
            Ok(()) => return Ok(()),
            // 1305 和 1336 是安全描述符暂时读不出来，比较失败仍然拒绝。
            Err(error) if matches!(error.raw_os_error(), Some(1305 | 1336)) && attempt < 3 => {
                std::thread::sleep(std::time::Duration::from_millis(1 << attempt));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

fn verify_once(user: &User, file: &File) -> io::Result<()> {
    use std::mem::MaybeUninit;
    let handle = file.as_raw_handle();
    // 在系统读取句柄信息期间，File 仍然有效。
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected a disk file",
        ));
    }
    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    if unsafe { GetFileInformationByHandle(handle, info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || info.nNumberOfLinks != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected a regular file with one link",
        ));
    }
    // 与 Core 对普通文件的私有 DACL 检查相同。
    user.verify(handle)
}

impl User {
    fn verify(&self, handle: HANDLE) -> io::Result<()> {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut descriptor = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut acl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let _allocation = LocalBlock(descriptor);
        let denied = || {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "storage must be owned by the current user with its private DACL",
            )
        };
        let mut control = 0;
        let mut revision = 0;
        unsafe {
            check(GetSecurityDescriptorControl(
                descriptor,
                &mut control,
                &mut revision,
            ))?;
            if owner.is_null() || EqualSid(owner, self.sid()) == 0 || acl.is_null() {
                return Err(denied());
            }
            let count = (*acl).AceCount;
            if count == 0 || count > 2 {
                return Err(denied());
            }
            let protected = control & SE_DACL_PROTECTED != 0;
            let mut saw_user = false;
            for index in 0..count {
                let mut raw = null_mut();
                check(GetAce(acl, u32::from(index), &mut raw))?;
                let header = &*raw.cast::<windows_sys::Win32::Security::ACE_HEADER>();
                if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
                    return Err(denied());
                }
                let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
                let sid = ptr::addr_of!(ace.SidStart).cast_mut().cast();
                let flags = u32::from(header.AceFlags);
                if EqualSid(sid, self.sid()) != 0 {
                    let expected =
                        (protected && flags == 0) || (!protected && flags == INHERITED_ACE);
                    if !expected
                        || ace.Mask != windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS
                    {
                        return Err(denied());
                    }
                    saw_user = true;
                } else if !app_container_package(sid) {
                    return Err(denied());
                }
            }
            if !saw_user {
                return Err(denied());
            }
        }
        Ok(())
    }
}

fn app_container_package(sid: PSID) -> bool {
    unsafe {
        let authority = GetSidIdentifierAuthority(sid);
        if authority.is_null() || (*authority).Value != [0, 0, 0, 0, 0, 15] {
            return false;
        }
        let count = GetSidSubAuthorityCount(sid);
        if count.is_null() || *count < 8 {
            return false;
        }
        let first = GetSidSubAuthority(sid, 0);
        !first.is_null() && *first == 2
    }
}

fn check(ok: i32) -> io::Result<()> {
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
