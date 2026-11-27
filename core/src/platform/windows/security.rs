use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{self, null_mut};

use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
    GetSecurityDescriptorControl, GetSidIdentifierAuthority, GetSidSubAuthority,
    GetSidSubAuthorityCount, GetTokenInformation, INHERITED_ACE, OBJECT_INHERIT_ACE,
    OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::{check, wide_os};

pub(super) struct LocalBlock(pub(super) *mut c_void);

impl Drop for LocalBlock {
    fn drop(&mut self) {
        // 指针均来自声明要求调用 LocalFree 的 Windows 接口。
        unsafe { LocalFree(self.0) };
    }
}

struct User {
    buffer: Vec<usize>,
}

impl User {
    fn current() -> io::Result<Self> {
        let mut raw = null_mut();
        // 系统进程伪句柄无需关闭；成功返回的令牌句柄仅由 OwnedHandle 负责。
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
        // TOKEN_USER 包含指针，内核填充的可变长缓冲必须按指针大小对齐。
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
        // 缓冲来自 TokenUser 查询且不再改变大小；SID 在缓冲释放前有效。
        unsafe { (*self.buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn text(&self) -> io::Result<String> {
        let mut raw = null_mut();
        unsafe { check(ConvertSidToStringSidW(self.sid(), &mut raw))? };
        let _allocation = LocalBlock(raw.cast());
        unsafe { wide_os(raw) }
            .into_string()
            .map_err(|_| io::Error::other("invalid SID text"))
    }
}

pub(super) fn current_id() -> io::Result<String> {
    User::current()?.text()
}

pub(super) struct PrivateAccess {
    user: User,
    descriptor: LocalBlock,
}

impl PrivateAccess {
    pub(super) fn current(directory: bool) -> io::Result<Self> {
        let user = User::current()?;
        let sid = user.text()?;
        let flags = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("O:{sid}D:P(A;{flags};FA;;;{sid})\0")
            .encode_utf16()
            .collect();
        let mut raw = null_mut();
        // 当前用户为显式所有者。唯一的允许项向文件和目录继承；阻止来自共享父目录的权限。
        unsafe {
            check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut raw,
                null_mut(),
            ))?;
        }
        Ok(Self {
            user,
            descriptor: LocalBlock(raw),
        })
    }

    pub(super) fn descriptor(&self) -> *mut c_void {
        self.descriptor.0
    }

    pub(super) fn verify(&self, handle: HANDLE) -> io::Result<()> {
        self.user.verify(handle, true)
    }
}

pub(super) fn verify_file(handle: HANDLE) -> io::Result<()> {
    User::current()?.verify(handle, false)
}

pub(crate) struct ObservedSecurity {
    pub owner: String,
    pub sddl: Option<String>,
}

pub(crate) fn path_security(path: &Path) -> io::Result<ObservedSecurity> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    name.push(0);
    let mut owner = null_mut();
    let mut descriptor = null_mut();
    // 返回的描述符自包含所有者与 DACL；LocalFree 释放整块，不能单独释放 SID。
    let status = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _allocation = LocalBlock(descriptor);
    if owner.is_null() || descriptor.is_null() {
        return Err(io::Error::other("security lookup omitted the owner"));
    }
    Ok(ObservedSecurity {
        owner: sid_text(owner)?,
        sddl: sddl_text(descriptor).ok(),
    })
}

fn sid_text(sid: PSID) -> io::Result<String> {
    let mut raw = null_mut();
    unsafe { check(ConvertSidToStringSidW(sid, &mut raw))? };
    let _allocation = LocalBlock(raw.cast());
    unsafe { wide_os(raw) }
        .into_string()
        .map_err(|_| io::Error::other("invalid SID text"))
}

fn sddl_text(descriptor: *mut c_void) -> io::Result<String> {
    let mut raw = null_mut();
    unsafe {
        check(ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut raw,
            null_mut(),
        ))?;
    }
    let _allocation = LocalBlock(raw.cast());
    unsafe { wide_os(raw) }
        .into_string()
        .map_err(|_| io::Error::other("invalid security descriptor text"))
}

impl User {
    fn verify(&self, handle: HANDLE, directory: bool) -> io::Result<()> {
        super::retry_unknown_revision(|| self.verify_once(handle, directory))
    }

    fn verify_once(&self, handle: HANDLE, directory: bool) -> io::Result<()> {
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut descriptor = null_mut();
        // 这些指针都引用同一自包含的有效安全描述符，读取完成后统一释放。
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
            // 私有 DACL 仍只有当前用户这一条完整控制。语言沙箱可以再加一条
            // 本插件 AppContainer 的允许项。Everyone、Users 或其他 SID 仍然拒绝。
            let count = (*acl).AceCount;
            if count == 0 || count > 2 {
                return Err(denied());
            }
            let protected = control & SE_DACL_PROTECTED != 0;
            let mut saw_user = false;
            for index in 0..count {
                let mut raw = null_mut();
                check(GetAce(acl, u32::from(index), &mut raw))?;
                // 先确认 ACE 类型；可变长 SID 从 SidStart 开始，边界由 GetSecurityInfo 返回的系统 ACL 保证。
                let header = &*raw.cast::<windows_sys::Win32::Security::ACE_HEADER>();
                if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
                    return Err(denied());
                }
                let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
                let sid = ptr::addr_of!(ace.SidStart).cast_mut().cast();
                let flags = u32::from(header.AceFlags);
                if EqualSid(sid, self.sid()) != 0 {
                    let expected_flags = if directory {
                        protected && flags == (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE)
                    } else {
                        (protected && flags == 0) || (!protected && flags == INHERITED_ACE)
                    };
                    if !expected_flags || ace.Mask != FILE_ALL_ACCESS {
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
        // S-1-15-2-1 是所有应用包。配置文件 SID 还有一段哈希，子权限更多。
        if count.is_null() || *count < 8 {
            return false;
        }
        let first = GetSidSubAuthority(sid, 0);
        !first.is_null() && *first == 2
    }
}
