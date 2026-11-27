use std::ffi::c_void;
use std::io::{self, ErrorKind, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_PENDING,
    ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, RevertToSelf, SECURITY_ATTRIBUTES, TOKEN_ELEVATION, TOKEN_QUERY,
    TOKEN_USER, TokenElevation, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, ImpersonateNamedPipeClient, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
    WaitForSingleObject,
};
use winstrata_kernel::DriverHost;

use super::{Endpoint, Privilege};
use crate::protocol::{read_frame, write_frame};
use crate::session::{self, Session};

const GENERIC_READ_WRITE: u32 =
    windows_sys::Win32::Foundation::GENERIC_READ | windows_sys::Win32::Foundation::GENERIC_WRITE;

pub(crate) struct Connection(OwnedHandle);

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        transfer(self.0.as_raw_handle(), buf.as_mut_ptr(), buf.len(), false)
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        transfer(
            self.0.as_raw_handle(),
            buf.as_ptr() as *mut u8,
            buf.len(),
            true,
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct LocalBlock(*mut c_void);

// 安全描述符只被移动到接受线程，不在线程之间共享。
unsafe impl Send for LocalBlock {}

impl Drop for LocalBlock {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // 指针来自要求 LocalFree 的 Windows 分配。
            unsafe { LocalFree(self.0) };
        }
    }
}

struct Event(HANDLE);

impl Event {
    fn new() -> io::Result<Self> {
        let handle = unsafe { CreateEventW(null(), 1, 0, null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn current_user() -> io::Result<String> {
    let token = process_token(TOKEN_QUERY)?;
    token_sid(&token)
}

pub(super) fn current_privilege() -> io::Result<Privilege> {
    let token = process_token(TOKEN_QUERY)?;
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut bytes = 0;
    unsafe {
        check(GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut bytes,
        ))?;
    }
    Ok(if elevation.TokenIsElevated == 0 {
        Privilege::WindowsOrdinary
    } else {
        Privilege::WindowsElevated
    })
}

pub(super) fn unique_endpoint() -> io::Result<Endpoint> {
    Ok(Endpoint {
        address: format!(r"\\.\pipe\{}", super::next_name("winstrata")),
        owned_file: None,
        owned_dir: None,
    })
}

pub(super) fn user_endpoint() -> io::Result<Endpoint> {
    Ok(Endpoint {
        address: format!(r"\\.\pipe\WinStrata-{}", current_user()?),
        owned_file: None,
        owned_dir: None,
    })
}

pub(super) fn connect(address: &str) -> io::Result<Connection> {
    let name = wide(address)?;
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(Connection(unsafe { OwnedHandle::from_raw_handle(handle) }))
}

pub(super) fn serve<H>(
    address: &str,
    session: Session<H>,
    stop: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>>
where
    H: DriverHost + Send + 'static,
{
    let descriptor = user_descriptor()?;
    let name = address.to_owned();
    let shared = Arc::new(Mutex::new(session));
    Ok(thread::spawn(move || {
        accept_loop(&name, &descriptor, shared, &stop)
    }))
}

fn accept_loop<H>(
    name: &str,
    descriptor: &LocalBlock,
    shared: Arc<Mutex<Session<H>>>,
    stop: &AtomicBool,
) where
    H: DriverHost,
{
    while !stop.load(Ordering::Acquire) {
        let pipe = match create_pipe(name, descriptor.0) {
            Ok(pipe) => pipe,
            Err(_) => {
                lock(&shared).record_fault();
                break;
            }
        };
        match connect_pipe(pipe.as_raw_handle(), 200) {
            Err(error) if error.kind() == ErrorKind::TimedOut => continue,
            Err(_) => {
                lock(&shared).record_fault();
                break;
            }
            Ok(()) if stop.load(Ordering::Acquire) => break,
            Ok(()) => match peer_user(pipe.as_raw_handle()) {
                Ok(peer) => conversation(Connection(pipe), &shared, &peer),
                Err(_) => lock(&shared).record_fault(),
            },
        }
    }
}

fn conversation<H>(mut pipe: Connection, shared: &Mutex<Session<H>>, peer: &str)
where
    H: DriverHost,
{
    loop {
        let input = match read_frame(&mut pipe) {
            Ok(input) => input,
            Err(_) => return,
        };
        let output = {
            let mut session = lock(shared);
            session::dispatch(&mut session, peer, &input)
        };
        if write_frame(&mut pipe, &output).is_err() {
            return;
        }
    }
}

fn lock<H: DriverHost>(shared: &Mutex<Session<H>>) -> std::sync::MutexGuard<'_, Session<H>> {
    match shared.lock() {
        Ok(guard) => guard,
        Err(poison) => {
            let mut guard = poison.into_inner();
            guard.record_fault();
            guard
        }
    }
}

fn create_pipe(name: &str, descriptor: *mut c_void) -> io::Result<OwnedHandle> {
    let name = wide(name)?;
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_BYTE | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            16 * 1024,
            16 * 1024,
            2_000,
            &security,
        )
    };
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

fn connect_pipe(handle: HANDLE, timeout_ms: u32) -> io::Result<()> {
    let event = Event::new()?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event.0;
    let immediate = unsafe { ConnectNamedPipe(handle, &mut overlapped) };
    if immediate != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    if error == ERROR_PIPE_CONNECTED {
        return Ok(());
    }
    if error != ERROR_IO_PENDING {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    finish_overlapped(handle, &overlapped, event.0, timeout_ms)?;
    Ok(())
}

fn transfer(handle: HANDLE, bytes: *mut u8, len: usize, write: bool) -> io::Result<usize> {
    if len == 0 {
        return Ok(0);
    }
    let event = Event::new()?;
    let mut overlapped = OVERLAPPED::default();
    overlapped.hEvent = event.0;
    let mut transferred = 0u32;
    // 调用方的缓冲和栈上的 OVERLAPPED 在操作完成前保持有效。
    let immediate = unsafe {
        if write {
            WriteFile(
                handle,
                bytes.cast_const(),
                len as u32,
                &mut transferred,
                &mut overlapped,
            )
        } else {
            ReadFile(handle, bytes, len as u32, &mut transferred, &mut overlapped)
        }
    };
    if immediate != 0 {
        return Ok(transferred as usize);
    }
    let error = unsafe { GetLastError() };
    if error == ERROR_BROKEN_PIPE {
        return Ok(0);
    }
    if error != ERROR_IO_PENDING {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    finish_overlapped(
        handle,
        &overlapped,
        event.0,
        super::IO_TIMEOUT.as_millis() as u32,
    )
}

fn finish_overlapped(
    handle: HANDLE,
    overlapped: &OVERLAPPED,
    event: HANDLE,
    timeout_ms: u32,
) -> io::Result<usize> {
    let waited = unsafe { WaitForSingleObject(event, timeout_ms) };
    if waited == WAIT_TIMEOUT {
        unsafe { CancelIoEx(handle, overlapped) };
        let mut ignored = 0;
        unsafe { GetOverlappedResult(handle, overlapped, &mut ignored, 1) };
        return Err(io::Error::new(
            ErrorKind::TimedOut,
            "pipe operation timed out",
        ));
    }
    if waited != WAIT_OBJECT_0 {
        return Err(io::Error::last_os_error());
    }
    let mut transferred = 0u32;
    let ok = unsafe { GetOverlappedResult(handle, overlapped, &mut transferred, 0) };
    if ok == 0 {
        let error = unsafe { GetLastError() };
        if error == ERROR_BROKEN_PIPE {
            return Ok(0);
        }
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    Ok(transferred as usize)
}

fn peer_user(pipe: HANDLE) -> io::Result<String> {
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _revert = RevertOnDrop;
    let mut raw = null_mut();
    unsafe {
        check(OpenThreadToken(
            GetCurrentThread(),
            TOKEN_QUERY,
            1,
            &mut raw,
        ))?;
    }
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    token_sid(&token)
}

struct RevertOnDrop;

impl Drop for RevertOnDrop {
    fn drop(&mut self) {
        unsafe { RevertToSelf() };
    }
}

fn process_token(access: u32) -> io::Result<OwnedHandle> {
    let mut raw = null_mut();
    unsafe { check(OpenProcessToken(GetCurrentProcess(), access, &mut raw))? };
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

fn token_sid(token: &OwnedHandle) -> io::Result<String> {
    let mut bytes = 0u32;
    let first =
        unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut bytes) };
    if first != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::last_os_error());
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
        // SID 位于本次查询缓冲内，复制成字符串后才释放缓冲。
        let sid = (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid;
        sid_text(sid)
    }
}

fn sid_text(sid: *mut c_void) -> io::Result<String> {
    let mut raw = null_mut();
    unsafe { check(ConvertSidToStringSidW(sid, &mut raw))? };
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

fn user_descriptor() -> io::Result<LocalBlock> {
    let sddl = wide(&format!("D:P(A;;GA;;;{})", current_user()?))?;
    let mut raw = null_mut();
    unsafe {
        check(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut raw,
            null_mut(),
        ))?;
    }
    Ok(LocalBlock(raw))
}

fn wide(value: &str) -> io::Result<Vec<u16>> {
    if value.contains('\0') {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Windows string contains NUL",
        ));
    }
    Ok(value.encode_utf16().chain([0]).collect())
}

fn check(ok: i32) -> io::Result<()> {
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
