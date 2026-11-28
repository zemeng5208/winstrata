//! 语言插件进程的操作系统隔离。
//!
//! Linux 在解释器加载时套用 Landlock，并由预加载库拒绝网络和新程序。
//! 规则无法建立时不启动进程。

#[cfg(unix)]
mod linux {
    use std::fs::File;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    use super::super::PluginError;

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

    pub struct Ruleset {
        fd: i32,
    }

    impl Drop for Ruleset {
        fn drop(&mut self) {
            unsafe { libc::close(self.fd) };
        }
    }

    impl Ruleset {
        pub fn fd(&self) -> i32 {
            self.fd
        }
    }

    pub fn prepare(
        read: &[impl AsRef<Path>],
        write: &[impl AsRef<Path>],
    ) -> Result<Ruleset, PluginError> {
        let abi = unsafe { libc::syscall(SYS_CREATE, 0, 0, VERSION_FLAG) };
        if abi < 1 {
            return Err(unavailable("landlock is not available"));
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
            return Err(unavailable("landlock ruleset was rejected"));
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
            add_path(ruleset.fd, path.as_ref(), read_access)?;
        }
        for path in write {
            add_path(ruleset.fd, path.as_ref(), write_access)?;
        }
        // 描述符需要穿过 exec，预加载库才能套用同一份规则。
        if unsafe { libc::fcntl(ruleset.fd, libc::F_SETFD, 0) } != 0 {
            return Err(unavailable("landlock descriptor could not be inherited"));
        }
        Ok(ruleset)
    }

    fn add_path(ruleset: i32, path: &Path, access: u64) -> Result<(), PluginError> {
        let file = open_path(path)?;
        let metadata = file
            .metadata()
            .map_err(|error| unavailable(error.to_string()))?;
        // 文件规则不能带目录权限，否则内核拒绝字符设备。
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
            Err(unavailable(format!("landlock rejected {}", path.display())))
        }
    }

    fn open_path(path: &Path) -> Result<File, PluginError> {
        let mut bytes = path.as_os_str().as_bytes().to_vec();
        bytes.push(0);
        let fd = unsafe { libc::open(bytes.as_ptr().cast(), libc::O_PATH | libc::O_CLOEXEC) };
        if fd < 0 {
            Err(unavailable(format!(
                "isolation path is missing: {}",
                path.display()
            )))
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    fn unavailable(detail: impl ToString) -> PluginError {
        PluginError::new("isolation_unavailable", detail.to_string())
    }
}

#[cfg(unix)]
pub(super) use linux::prepare as prepare_linux;

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{
        CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, LocalFree, SetHandleInformation, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Security::Authorization::{
        EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, REVOKE_ACCESS,
        SE_FILE_OBJECT, SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID,
        TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::Isolation::{
        CreateAppContainerProfile, DeleteAppContainerProfile,
    };
    use windows_sys::Win32::Security::{
        CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, FreeSid, GetSecurityDescriptorControl,
        OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED,
        SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, TOKEN_DUPLICATE, TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_EXECUTE,
        FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_ALWAYS,
    };
    use windows_sys::Win32::System::Environment::{
        CreateEnvironmentBlock, DestroyEnvironmentBlock,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
        DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
        GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
        STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
        WaitForSingleObject,
    };
    use winstrata_protocols::PluginVersion;

    use super::super::PluginError;
    use super::super::process::{Live, Reap};

    struct Guard {
        process: OwnedHandle,
        thread: OwnedHandle,
        job: OwnedHandle,
        profile: Vec<u16>,
        sid: PSID,
        granted: Vec<(PathBuf, bool)>,
        pid: u32,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            // job 句柄保持到这里，关闭时由 KILL_ON_JOB_CLOSE 结束仍在作业中的进程。
            let _ = raw_handle(&self.job);
            let _ = raw_handle(&self.thread);
            unsafe {
                TerminateProcess(self.as_raw_process(), 1);
            }
            revoke_all(&self.granted, self.sid);
            unsafe {
                DeleteAppContainerProfile(self.profile.as_ptr());
                FreeSid(self.sid);
            }
        }
    }

    impl Guard {
        fn as_raw_process(&self) -> HANDLE {
            use std::os::windows::io::AsRawHandle;
            self.process.as_raw_handle()
        }
    }

    impl Reap for Guard {
        fn id(&self) -> u32 {
            self.pid
        }

        fn try_wait(&mut self) -> std::io::Result<Option<i32>> {
            let process = self.as_raw_process();
            let state = unsafe { WaitForSingleObject(process, 0) };
            if state == WAIT_TIMEOUT {
                return Ok(None);
            }
            if state != WAIT_OBJECT_0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut code = 0u32;
            if unsafe { GetExitCodeProcess(process, &mut code) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Some(code as i32))
        }

        fn kill(&mut self) {
            unsafe { TerminateProcess(self.as_raw_process(), 1) };
        }

        fn wait(&mut self) -> std::io::Result<i32> {
            let process = self.as_raw_process();
            if unsafe { WaitForSingleObject(process, 3_000) } != WAIT_OBJECT_0 {
                unsafe { TerminateProcess(process, 1) };
            }
            let mut code = 0u32;
            if unsafe { GetExitCodeProcess(process, &mut code) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(code as i32)
        }
    }

    struct Profile {
        name: Vec<u16>,
        sid: PSID,
        granted: Vec<(PathBuf, bool)>,
    }

    impl Drop for Profile {
        fn drop(&mut self) {
            if !self.sid.is_null() {
                revoke_all(&self.granted, self.sid);
                unsafe {
                    DeleteAppContainerProfile(self.name.as_ptr());
                    FreeSid(self.sid);
                    self.sid = null_mut();
                }
            }
        }
    }

    impl Profile {
        fn into_parts(mut self) -> (Vec<u16>, PSID, Vec<(PathBuf, bool)>) {
            let sid = self.sid;
            self.sid = null_mut();
            (
                std::mem::take(&mut self.name),
                sid,
                std::mem::take(&mut self.granted),
            )
        }
    }

    pub fn spawn_windows(
        command: &Command,
        state: &Path,
        python_prefix: Option<&Path>,
        node_prefix: Option<&Path>,
        read: &[impl AsRef<Path>],
        version: PluginVersion,
    ) -> Result<Live, PluginError> {
        let (name, sid) = container_profile()?;
        let mut profile = Profile {
            name,
            sid,
            granted: Vec::new(),
        };
        if let Some(prefix) = python_prefix {
            grant_python_prefix(prefix, profile.sid, &mut profile.granted)?;
        }
        if let Some(prefix) = node_prefix {
            grant_node_prefix(prefix, profile.sid, &mut profile.granted)?;
        }
        for path in read {
            grant_tree(path.as_ref(), profile.sid, false, &mut profile.granted)?;
        }
        grant_tree(state, profile.sid, true, &mut profile.granted)?;
        let job = job()?;
        let (stdin, stdout, child_in, child_out, child_err) = pipes(state)?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = child_in;
        startup.StartupInfo.hStdOutput = child_out;
        startup.StartupInfo.hStdError = child_err;
        let mut list_size = 0usize;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut list_size);
        }
        let mut list = vec![0u8; list_size];
        if unsafe {
            InitializeProcThreadAttributeList(list.as_mut_ptr().cast(), 1, 0, &mut list_size)
        } == 0
        {
            return Err(io_error("process attribute list"));
        }
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        let mut capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        if unsafe {
            UpdateProcThreadAttribute(
                list.as_mut_ptr().cast(),
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                &mut capabilities as *mut SECURITY_CAPABILITIES as *const _,
                size_of::<SECURITY_CAPABILITIES>(),
                null_mut(),
                null(),
            )
        } == 0
        {
            unsafe { DeleteProcThreadAttributeList(list.as_mut_ptr().cast()) };
            return Err(io_error("AppContainer attribute"));
        }
        startup.lpAttributeList = list.as_mut_ptr().cast();
        let mut token = null_mut();
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE,
                &mut token,
            )
        } == 0
        {
            unsafe { DeleteProcThreadAttributeList(list.as_mut_ptr().cast()) };
            return Err(io_error("current user token"));
        }
        let mut environment = null_mut();
        // 空令牌配出来的环境会让 CreateProcess 返回 203。计划 004 用当前用户令牌避免了这一点。
        let built = unsafe { CreateEnvironmentBlock(&mut environment, token, 0) };
        unsafe { CloseHandle(token) };
        if built == 0 {
            unsafe { DeleteProcThreadAttributeList(list.as_mut_ptr().cast()) };
            return Err(io_error("process environment"));
        }
        let program = wide(command.get_program());
        let mut cmdline = command_line(command);
        let directory = wide(state.as_os_str());
        let mut info = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                program.as_ptr(),
                cmdline.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment,
                directory.as_ptr(),
                &startup as *const STARTUPINFOEXW as *const _,
                &mut info,
            )
        };
        unsafe {
            DestroyEnvironmentBlock(environment);
            DeleteProcThreadAttributeList(list.as_mut_ptr().cast());
            CloseHandle(child_in);
            CloseHandle(child_out);
            CloseHandle(child_err);
        }
        if created == 0 {
            return Err(io_error("language process"));
        }
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread) };
        if unsafe { AssignProcessToJobObject(raw_handle(&job), process.as_raw_handle()) } == 0 {
            unsafe { TerminateProcess(process.as_raw_handle(), 1) };
            return Err(io_error("job assignment"));
        }
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            unsafe { TerminateProcess(process.as_raw_handle(), 1) };
            return Err(io_error("resume"));
        }
        let (profile, sid, granted) = profile.into_parts();
        let guard = Guard {
            process,
            thread,
            job,
            profile,
            sid,
            granted,
            pid: info.dwProcessId,
        };
        Live::from_pipes(Box::new(guard), stdin, stdout, version)
    }

    fn raw_handle(handle: &OwnedHandle) -> HANDLE {
        use std::os::windows::io::AsRawHandle;
        handle.as_raw_handle()
    }

    fn container_profile() -> Result<(Vec<u16>, PSID), PluginError> {
        let name = format!(
            "ws015{:x}{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.subsec_nanos())
                .unwrap_or(0)
        );
        let profile = wide(OsStr::new(&name));
        let mut sid = null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                profile.as_ptr(),
                profile.as_ptr(),
                profile.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if hr < 0 {
            unsafe { DeleteAppContainerProfile(profile.as_ptr()) };
            let hr = unsafe {
                CreateAppContainerProfile(
                    profile.as_ptr(),
                    profile.as_ptr(),
                    profile.as_ptr(),
                    null(),
                    0,
                    &mut sid,
                )
            };
            if hr < 0 {
                return Err(PluginError::new(
                    "isolation_unavailable",
                    format!("AppContainer profile was rejected: {hr}"),
                ));
            }
        }
        Ok((profile, sid))
    }

    fn is_volume_root(path: &Path) -> bool {
        path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        })
    }

    fn grant_tree(
        root: &Path,
        sid: PSID,
        write: bool,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), PluginError> {
        let root = std::fs::canonicalize(root).map_err(|error| {
            PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
        })?;
        if is_volume_root(&root) {
            return Err(PluginError::new(
                "isolation_unavailable",
                "sandbox grant would cover a volume root",
            ));
        }
        let mut visited = 0u32;
        grant_one(&root, sid, write, true, granted)?;
        if root.is_dir() {
            walk_grant(&root, &root, sid, write, &mut visited, granted)?;
        }
        Ok(())
    }

    /// 只对前缀根目录及其顶层文件做非继承授权，避免把宿主的第三方扩展或缓存暴露给隔离容器。
    fn grant_prefix_files(
        prefix: &Path,
        sid: PSID,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<PathBuf, PluginError> {
        let prefix = std::fs::canonicalize(prefix).map_err(|error| {
            PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
        })?;
        if is_volume_root(&prefix) {
            return Err(PluginError::new(
                "isolation_unavailable",
                "sandbox grant would cover a volume root",
            ));
        }
        grant_one(&prefix, sid, false, false, granted)?;
        for entry in std::fs::read_dir(&prefix).map_err(|error| {
            PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
            })?;
            if metadata.file_type().is_symlink() || metadata.is_dir() {
                continue;
            }
            grant_one(&path, sid, false, false, granted)?;
        }
        Ok(prefix)
    }

    fn grant_node_prefix(
        prefix: &Path,
        sid: PSID,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), PluginError> {
        grant_prefix_files(prefix, sid, granted).map(|_| ())
    }

    fn grant_python_prefix(
        prefix: &Path,
        sid: PSID,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), PluginError> {
        let prefix = grant_prefix_files(prefix, sid, granted)?;
        let dlls = prefix.join("DLLs");
        if dlls.is_dir() {
            grant_tree(&dlls, sid, false, granted)?;
        }
        let lib = prefix.join("Lib");
        if lib.is_dir() {
            grant_one(&lib, sid, false, false, granted)?;
            for entry in std::fs::read_dir(&lib).map_err(|error| {
                PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
            })? {
                let entry = entry.map_err(|error| {
                    PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
                })?;
                let path = entry.path();
                let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                    PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
                })?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                if metadata.is_dir() {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("");
                    if name.eq_ignore_ascii_case("site-packages")
                        || name.eq_ignore_ascii_case("__pycache__")
                    {
                        continue;
                    }
                    grant_tree(&path, sid, false, granted)?;
                } else {
                    grant_one(&path, sid, false, false, granted)?;
                }
            }
        }
        Ok(())
    }

    fn walk_grant(
        dir: &Path,
        root: &Path,
        sid: PSID,
        write: bool,
        visited: &mut u32,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), PluginError> {
        for entry in std::fs::read_dir(dir).map_err(|error| {
            PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
        })? {
            let entry = entry.map_err(|error| {
                PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
            })?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                PluginError::new("isolation_unavailable", format!("sandbox path: {error}"))
            })?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            let Ok(canonical) = std::fs::canonicalize(&path) else {
                continue;
            };
            if !canonical.starts_with(root) {
                continue;
            }
            *visited += 1;
            if *visited > 20_000 {
                return Err(PluginError::new(
                    "isolation_unavailable",
                    "sandbox grant exceeded the file limit",
                ));
            }
            // 未受保护的后代通过根目录的 OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE 自动继承，仅补授受保护节点。
            if is_dacl_protected(&path)? {
                grant_one(&path, sid, write, metadata.is_dir(), granted)?;
            }
            if metadata.is_dir() {
                walk_grant(&path, root, sid, write, visited, granted)?;
            }
        }
        Ok(())
    }

    fn is_dacl_protected(path: &Path) -> Result<bool, PluginError> {
        let name = wide(path.as_os_str());
        let mut descriptor = null_mut();
        let mut old_acl = null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_acl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(PluginError::new(
                "isolation_unavailable",
                format!("security descriptor was not read: {status}"),
            ));
        }
        let mut control = 0;
        let mut revision = 0;
        let protected = unsafe {
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) != 0
                && control & SE_DACL_PROTECTED != 0
        };
        unsafe { LocalFree(descriptor.cast()) };
        Ok(protected)
    }

    fn grant_one(
        path: &Path,
        sid: PSID,
        write: bool,
        inheritable: bool,
        granted: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), PluginError> {
        let directory = path.is_dir();
        let name = wide(path.as_os_str());
        let mut descriptor = null_mut();
        let mut old_acl = null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_acl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(PluginError::new(
                "isolation_unavailable",
                format!("security descriptor was not read: {status}"),
            ));
        }
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: if write {
                FILE_ALL_ACCESS
            } else {
                FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
            },
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: if inheritable && directory {
                OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
            } else {
                0
            },
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: sid.cast(),
            },
        };
        let mut new_acl = null_mut();
        let merged = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut new_acl) };
        let mut control = 0;
        let mut revision = 0;
        let protected = unsafe {
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) != 0
                && control & SE_DACL_PROTECTED != 0
        };
        unsafe { LocalFree(descriptor.cast()) };
        if merged != 0 {
            return Err(PluginError::new(
                "isolation_unavailable",
                format!("sandbox grant was not merged: {merged}"),
            ));
        }
        // 只写 DACL 会清掉 SE_DACL_PROTECTED。私有目录随后会因不再受保护而被拒绝。
        let mut information = DACL_SECURITY_INFORMATION;
        if protected {
            information |= PROTECTED_DACL_SECURITY_INFORMATION;
        }
        let written = unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                information,
                null_mut(),
                null_mut(),
                new_acl,
                null(),
            )
        };
        unsafe { LocalFree(new_acl.cast()) };
        if written == 0 {
            granted.push((path.to_path_buf(), protected));
            Ok(())
        } else {
            Err(PluginError::new(
                "isolation_unavailable",
                format!("sandbox grant was rejected: {written}"),
            ))
        }
    }

    fn revoke_all(granted: &[(PathBuf, bool)], sid: PSID) {
        if sid.is_null() {
            return;
        }
        for (path, protected) in granted.iter().rev() {
            if !path.exists() {
                continue;
            }
            let name = wide(path.as_os_str());
            let mut descriptor = null_mut();
            let mut old_acl = null_mut();
            let status = unsafe {
                GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    &mut old_acl,
                    null_mut(),
                    &mut descriptor,
                )
            };
            if status != 0 {
                continue;
            }
            let entry = EXPLICIT_ACCESS_W {
                grfAccessPermissions: 0,
                grfAccessMode: REVOKE_ACCESS,
                grfInheritance: 0,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: null_mut(),
                    MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_UNKNOWN,
                    ptstrName: sid.cast(),
                },
            };
            let mut new_acl = null_mut();
            let merged = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut new_acl) };
            unsafe { LocalFree(descriptor.cast()) };
            if merged != 0 {
                continue;
            }
            let mut information = DACL_SECURITY_INFORMATION;
            if *protected {
                information |= PROTECTED_DACL_SECURITY_INFORMATION;
            }
            unsafe {
                SetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    information,
                    null_mut(),
                    null_mut(),
                    new_acl,
                    null(),
                );
                LocalFree(new_acl.cast());
            }
        }
    }

    fn job() -> Result<OwnedHandle, PluginError> {
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(io_error("job"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            unsafe { CloseHandle(handle) };
            return Err(io_error("job limits"));
        }
        Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
    }

    fn pipes(state: &Path) -> Result<(File, File, HANDLE, HANDLE, HANDLE), PluginError> {
        let inherit = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let mut child_in = null_mut();
        let mut parent_write = null_mut();
        let mut parent_read = null_mut();
        let mut child_out = null_mut();
        if unsafe { CreatePipe(&mut child_in, &mut parent_write, &inherit, 0) } == 0
            || unsafe { CreatePipe(&mut parent_read, &mut child_out, &inherit, 0) } == 0
        {
            return Err(io_error("plugin pipes"));
        }
        unsafe {
            SetHandleInformation(parent_write, HANDLE_FLAG_INHERIT, 0);
            SetHandleInformation(parent_read, HANDLE_FLAG_INHERIT, 0);
        }
        let log = wide(state.join("stderr.log").as_os_str());
        let child_err = unsafe {
            CreateFileW(
                log.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &inherit,
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if child_err.is_null() || child_err == (-1isize as HANDLE) {
            return Err(io_error("plugin stderr"));
        }
        Ok((
            unsafe { File::from_raw_handle(parent_write) },
            unsafe { File::from_raw_handle(parent_read) },
            child_in,
            child_out,
            child_err,
        ))
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    fn command_line(command: &Command) -> Vec<u16> {
        let mut parts = Vec::new();
        parts.push(quote(command.get_program()));
        for arg in command.get_args() {
            parts.push(quote(arg));
        }
        wide(OsStr::new(&parts.join(" ")))
    }

    fn quote(value: &OsStr) -> String {
        let text = value.to_string_lossy();
        format!("\"{}\"", text.replace('"', "\\\""))
    }

    fn io_error(step: &str) -> PluginError {
        PluginError::new(
            "isolation_unavailable",
            format!("{step}: {}", std::io::Error::last_os_error()),
        )
    }
}

#[cfg(windows)]
pub(super) use windows::spawn_windows;
