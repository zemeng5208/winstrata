//! Windows 窗口、托盘和 WebView2。
//!
//! 控制器槽位对照 Microsoft.Web.WebView2 1.0.2903.40 的 `WebView2.h`。
//! 找不到已安装的加载器时，窗口仍在，并写明页面地址。界面不因此取得服务权限。

use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,     CREATESTRUCTW, DispatchMessageW, GWLP_USERDATA, GetClientRect, GetMessageW, GetWindowLongPtrW,
    IDI_APPLICATION,
    LoadIconW, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOW, SetTimer, SetWindowLongPtrW,
    ShowWindow, TranslateMessage, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_PAINT, WM_TIMER, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};

use crate::bridge::{HIDE, QUIT};

const TRAY_MESSAGE: u32 = 0x8001;

struct WindowData {
    origin: String,
    action: Arc<AtomicU8>,
}

pub fn run_window(url: &str, action: Arc<AtomicU8>) {
    unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32) };
    let origin = url.split("/?").next().unwrap_or(url).to_owned();
    let data = Box::new(WindowData {
        origin,
        action,
    });
    let class_name = wide("WinStrataOfficialGui");
    let title = wide("WinStrata");
    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: null_mut(),
            lpszClassName: class_name.as_ptr(),
            hbrBackground: (COLOR_WINDOW + 1) as _,
            hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
            ..std::mem::zeroed()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            800,
            null_mut(),
            null_mut(),
            null_mut(),
            Box::into_raw(data).cast(),
        );
        if hwnd.is_null() {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);
        attach_webview(hwnd, url);
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CREATE {
        let created = lparam as *const CREATESTRUCTW;
        let pointer = unsafe { (*created).lpCreateParams } as *mut WindowData;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, pointer as isize) };
        unsafe { SetTimer(hwnd, 1, 200, None) };
        return 0;
    }
    let data = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowData;
    match message {
        WM_CLOSE => {
            if data.is_null() {
                return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
            }
            let data = unsafe { &*data };
            let decision = request_close(&data.origin);
            match decision.as_str() {
                "hide" => {
                    hide_to_tray(hwnd);
                    0
                }
                "exit" => {
                    unsafe { DestroyWindow(hwnd) };
                    0
                }
                _ => 0,
            }
        }
        WM_TIMER => {
            if data.is_null() {
                return 0;
            }
            let data = unsafe { &*data };
            match data.action.swap(0, Ordering::AcqRel) {
                HIDE => hide_to_tray(hwnd),
                QUIT => {
                    unsafe { DestroyWindow(hwnd) };
                }
                _ => {}
            }
            0
        }
        TRAY_MESSAGE => {
            unsafe { ShowWindow(hwnd, SW_SHOW) };
            0
        }
        WM_DESTROY => {
            remove_tray(hwnd);
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_PAINT => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn request_close(origin: &str) -> String {
    let address = origin.trim_start_matches("http://");
    let mut stream = match std::net::TcpStream::connect(address) {
        Ok(stream) => stream,
        Err(_) => return "ask".to_owned(),
    };
    let _ = stream.write_all(b"GET /close-request HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    let mut buffer = String::new();
    let _ = std::io::Read::read_to_string(&mut stream, &mut buffer);
    if buffer.contains("\"action\":\"hide\"") {
        "hide".to_owned()
    } else if buffer.contains("\"action\":\"exit\"") {
        "exit".to_owned()
    } else {
        "ask".to_owned()
    }
}

fn hide_to_tray(hwnd: HWND) {
    unsafe { ShowWindow(hwnd, SW_HIDE) };
    let mut tip = [0u16; 128];
    let text = wide("WinStrata");
    tip[..text.len().min(127)].copy_from_slice(&text[..text.len().min(127)]);
    let mut icon = unsafe { std::mem::zeroed::<NOTIFYICONDATAW>() };
    icon.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    icon.hWnd = hwnd;
    icon.uID = 1;
    icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    icon.uCallbackMessage = TRAY_MESSAGE;
    icon.hIcon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
    icon.szTip = tip;
    unsafe { Shell_NotifyIconW(NIM_ADD, &icon) };
}

fn remove_tray(hwnd: HWND) {
    let mut icon = unsafe { std::mem::zeroed::<NOTIFYICONDATAW>() };
    icon.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    icon.hWnd = hwnd;
    icon.uID = 1;
    unsafe { Shell_NotifyIconW(NIM_DELETE, &icon) };
}

fn attach_webview(hwnd: HWND, url: &str) {
    let Some(loader) = find_loader() else {
        return;
    };
    let wide_loader = wide(loader.to_string_lossy().as_ref());
    let module = unsafe { LoadLibraryW(wide_loader.as_ptr()) };
    if module.is_null() {
        return;
    }
    let symbol = std::ffi::CString::new("CreateCoreWebView2EnvironmentWithOptions").ok();
    let Some(symbol) = symbol else { return };
    let create = unsafe { GetProcAddress(module, symbol.as_ptr().cast()) };
    let Some(create) = create else { return };
    let create: CreateEnvironment = unsafe { std::mem::transmute(create) };
    let profile = std::env::temp_dir().join("winstrata-official-gui");
    let _ = std::fs::create_dir_all(&profile);
    let profile_wide = wide(profile.to_string_lossy().as_ref());
    let url_wide = wide(url);
    let handler = Handler::environment(hwnd, url_wide);
    unsafe { create(null_mut(), profile_wide.as_ptr(), null_mut(), handler) };
}

fn find_loader() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let beside = dir.join("WebView2Loader.dll");
            if beside.is_file() {
                return Some(beside);
            }
        }
    }
    let roots = [
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramFiles"),
    ];
    for root in roots.into_iter().flatten() {
        let application = Path::new(&root)
            .join("Microsoft")
            .join("EdgeWebView")
            .join("Application");
        let Ok(entries) = std::fs::read_dir(&application) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("WebView2Loader.dll");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

type CreateEnvironment = unsafe extern "system" fn(
    *const u16,
    *const u16,
    *mut c_void,
    *mut Handler,
) -> i32;

#[repr(C)]
struct Handler {
    vtbl: *const HandlerVtbl,
    refs: AtomicU32,
    hwnd: HWND,
    url: Vec<u16>,
    environment: bool,
}

#[repr(C)]
struct HandlerVtbl {
    query: unsafe extern "system" fn(*mut Handler, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut Handler) -> u32,
    release: unsafe extern "system" fn(*mut Handler) -> u32,
    invoke: unsafe extern "system" fn(*mut Handler, i32, *mut c_void) -> i32,
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const ENV_HANDLER: Guid = Guid {
    data1: 0x4e8a3389,
    data2: 0xc9d8,
    data3: 0x4bd2,
    data4: [0xb6, 0xb5, 0x12, 0x4f, 0xee, 0x6c, 0xc1, 0x4d],
};
const CONTROLLER_HANDLER: Guid = Guid {
    data1: 0x6c4819f3,
    data2: 0xc9b7,
    data3: 0x4260,
    data4: [0x81, 0x27, 0xc9, 0xf5, 0xbd, 0xe7, 0xf6, 0x8c],
};
const IUNKNOWN: Guid = Guid {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
};

impl Handler {
    fn environment(hwnd: HWND, url: Vec<u16>) -> *mut Self {
        let handler = Box::new(Self {
            vtbl: &ENV_VTBL,
            refs: AtomicU32::new(1),
            hwnd,
            url,
            environment: true,
        });
        Box::into_raw(handler)
    }
}

static ENV_VTBL: HandlerVtbl = HandlerVtbl {
    query: query_interface,
    add_ref: add_ref,
    release: release,
    invoke: invoke_environment,
};
static CONTROLLER_VTBL: HandlerVtbl = HandlerVtbl {
    query: query_interface,
    add_ref: add_ref,
    release: release,
    invoke: invoke_controller,
};

unsafe extern "system" fn query_interface(
    this: *mut Handler,
    iid: *const Guid,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() || this.is_null() {
        return 0x8000_0003u32 as i32;
    }
    let guid = unsafe { &*iid };
    let handler = unsafe { &*this };
    let matches = same_guid(guid, &IUNKNOWN)
        || (handler.environment && same_guid(guid, &ENV_HANDLER))
        || (!handler.environment && same_guid(guid, &CONTROLLER_HANDLER));
    if !matches {
        unsafe { *out = null_mut() };
        return 0x8000_4002u32 as i32;
    }
    unsafe { *out = this.cast() };
    unsafe { add_ref(this) };
    0
}

unsafe extern "system" fn add_ref(this: *mut Handler) -> u32 {
    unsafe { (*this).refs.fetch_add(1, Ordering::Relaxed) + 1 }
}

unsafe extern "system" fn release(this: *mut Handler) -> u32 {
    let remaining = unsafe { (*this).refs.fetch_sub(1, Ordering::Release) - 1 };
    if remaining == 0 {
        unsafe { drop(Box::from_raw(this)) };
    }
    remaining
}

unsafe extern "system" fn invoke_environment(this: *mut Handler, result: i32, environment: *mut c_void) -> i32 {
    if result < 0 || environment.is_null() || this.is_null() {
        return 0;
    }
    let handler = unsafe { &*this };
    let controller = Box::into_raw(Box::new(Handler {
        vtbl: &CONTROLLER_VTBL,
        refs: AtomicU32::new(1),
        hwnd: handler.hwnd,
        url: handler.url.clone(),
        environment: false,
    }));
    type CreateController = unsafe extern "system" fn(*mut c_void, HWND, *mut Handler) -> i32;
    let vtbl = unsafe { *(environment as *const *const CreateController) };
    unsafe { (*vtbl.add(3))(environment, handler.hwnd, controller) }
}

unsafe extern "system" fn invoke_controller(this: *mut Handler, result: i32, controller: *mut c_void) -> i32 {
    if result < 0 || controller.is_null() || this.is_null() {
        return 0;
    }
    let handler = unsafe { &*this };
    let mut bounds = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(handler.hwnd, &mut bounds) };
    type PutBool = unsafe extern "system" fn(*mut c_void, i32) -> i32;
    type PutBounds = unsafe extern "system" fn(*mut c_void, RECT) -> i32;
    type GetWebView = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    type Navigate = unsafe extern "system" fn(*mut c_void, *const u16) -> i32;
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    let vtbl = unsafe { *(controller as *const *const usize) };
    let put_visible: PutBool = unsafe { std::mem::transmute(*vtbl.add(4)) };
    let put_bounds: PutBounds = unsafe { std::mem::transmute(*vtbl.add(6)) };
    let get_webview: GetWebView = unsafe { std::mem::transmute(*vtbl.add(25)) };
    unsafe {
        put_bounds(controller, bounds);
        put_visible(controller, 1);
    }
    let mut webview = null_mut();
    if unsafe { get_webview(controller, &mut webview) } >= 0 && !webview.is_null() {
        let view_vtbl = unsafe { *(webview as *const *const Navigate) };
        unsafe { (*view_vtbl.add(5))(webview, handler.url.as_ptr()) };
        let release: Release = unsafe { std::mem::transmute(*view_vtbl.add(2)) };
        unsafe { release(webview) };
    }
    0
}

fn same_guid(left: &Guid, right: &Guid) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

use std::io::Write;
