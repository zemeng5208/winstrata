use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

#[cfg(windows)]
use std::sync::atomic::Ordering;
#[cfg(windows)]
use std::sync::mpsc;
#[cfg(windows)]
use std::thread::JoinHandle;

#[cfg(windows)]
mod host {
    use super::*;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, LoadIconW,
        PostMessageW, RegisterClassW, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_LBUTTONDBLCLK,
        WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    const CALLBACK: u32 = 0x8000 + 16;
    static EVENTS: OnceLock<Arc<AtomicU8>> = OnceLock::new();
    static CLASS: &[u16] = &[
        b'W' as u16,
        b'i' as u16,
        b'n' as u16,
        b'S' as u16,
        b't' as u16,
        b'r' as u16,
        b'a' as u16,
        b't' as u16,
        b'a' as u16,
        b'T' as u16,
        b'r' as u16,
        b'a' as u16,
        b'y' as u16,
        0,
    ];

    pub struct Tray {
        hwnd: HWND,
        thread: Option<JoinHandle<()>>,
    }

    impl Drop for Tray {
        fn drop(&mut self) {
            unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub fn install(events: Arc<AtomicU8>) -> io::Result<Tray> {
        let (sender, receiver) = mpsc::channel::<Result<isize, io::Error>>();
        let thread = std::thread::Builder::new()
            .name("ws-tray".to_owned())
            .spawn(move || {
                let hwnd = match create_window(Arc::clone(&events)) {
                    Ok(hwnd) => hwnd,
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        return;
                    }
                };
                if sender.send(Ok(hwnd as isize)).is_err() {
                    unsafe { DestroyWindow(hwnd) };
                    return;
                }
                loop {
                    let mut message = unsafe { std::mem::zeroed() };
                    let status = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
                    if status == 0 || status == -1 {
                        break;
                    }
                    unsafe {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            })
            .map_err(|error| io::Error::other(error.to_string()))?;
        let hwnd = receiver
            .recv()
            .map_err(|_| io::Error::other("tray window did not start"))??
            as HWND;
        add_icon(hwnd)?;
        Ok(Tray {
            hwnd,
            thread: Some(thread),
        })
    }

    fn create_window(events: Arc<AtomicU8>) -> io::Result<HWND> {
        let _ = EVENTS.set(events);
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: CLASS.as_ptr(),
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                0,
                CLASS.as_ptr(),
                CLASS.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return Err(io::Error::last_os_error());
            }
            Ok(hwnd)
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == CALLBACK {
            let mouse = lparam as u32;
            let event = if mouse == WM_LBUTTONUP || mouse == WM_LBUTTONDBLCLK {
                1
            } else if mouse == WM_RBUTTONUP {
                2
            } else {
                0
            };
            if event != 0 {
                if let Some(slot) = EVENTS.get() {
                    slot.store(event, Ordering::Release);
                }
            }
            return 0;
        }
        if message == WM_CLOSE {
            let _ = remove_icon(hwnd);
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        if message == WM_DESTROY {
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn add_icon(hwnd: HWND) -> io::Result<()> {
        let mut data = icon_data(hwnd);
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &mut data) };
        if added == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn remove_icon(hwnd: HWND) -> io::Result<()> {
        let mut data = icon_data(hwnd);
        let removed = unsafe { Shell_NotifyIconW(NIM_DELETE, &mut data) };
        if removed == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = CALLBACK;
        data.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), 32512u16 as *const u16) };
        let tip = "WinStrata".encode_utf16().collect::<Vec<_>>();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        data
    }
}

#[cfg(windows)]
pub use host::{Tray, install};

#[cfg(not(windows))]
pub struct Tray;

#[cfg(not(windows))]
pub fn install(_events: Arc<AtomicU8>) -> io::Result<Tray> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the tray requires Windows",
    ))
}

/// 添加并立刻移除托盘图标，用来确认当前进程能否使用普通用户的托盘接口。
pub fn probe() -> io::Result<()> {
    let events = Arc::new(AtomicU8::new(0));
    drop(install(events)?);
    Ok(())
}
