use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use winstrata_service::Endpoint;

use crate::observe::observe_at;
use crate::prefs::{ClosePreference, Language, Preferences, Theme};

const ACTION_HIDE: u8 = 1;
const ACTION_QUIT: u8 = 2;

struct Shared {
    directory: PathBuf,
    assets: PathBuf,
    locator: PathBuf,
    endpoint: Option<Endpoint>,
    preferences: Preferences,
    close_pending: bool,
    token: String,
}

pub struct Bridge {
    address: SocketAddr,
    token: String,
    stop: Arc<AtomicBool>,
    action: Arc<AtomicU8>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn start(
        directory: PathBuf,
        assets: PathBuf,
        locator: PathBuf,
        endpoint: Option<Endpoint>,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let token = random_token()?;
        let preferences = Preferences::load(&directory)?;
        let shared = Arc::new(Mutex::new(Shared {
            directory,
            assets,
            locator,
            endpoint,
            preferences,
            close_pending: false,
            token: token.clone(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let action = Arc::new(AtomicU8::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_action = Arc::clone(&action);
        let thread = thread::spawn(move || accept_loop(listener, shared, thread_stop, thread_action));
        Ok(Self {
            address,
            token,
            stop,
            action,
            thread: Some(thread),
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}/?token={}", self.address, self.token)
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn action(&self) -> Arc<AtomicU8> {
        Arc::clone(&self.action)
    }

    pub fn request_close(&self) {
        if let Ok(mut stream) = TcpStream::connect(self.address) {
            let _ = stream.write_all(b"GET /close-request HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    action: Arc<AtomicU8>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                // 监听套接字是非阻塞的，接受出来的连接在 Linux 上会继承这个标志。
                let _ = stream.set_nonblocking(false);
                let shared = Arc::clone(&shared);
                let action = Arc::clone(&action);
                thread::spawn(move || {
                    let _ = handle(stream, &shared, &action);
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle(mut stream: TcpStream, shared: &Mutex<Shared>, action: &AtomicU8) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end;
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > 16 * 1024 {
            return write_status(&mut stream, 413, "application/json", b"{\"error\":\"too_large\"}");
        }
        if let Some(end) = buffer.windows(4).position(|mark| mark == b"\r\n\r\n") {
            header_end = end;
            break;
        }
    }
    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let length = content_length(&head);
    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > 16 * 1024 {
            return write_status(&mut stream, 413, "application/json", b"{\"error\":\"too_large\"}");
        }
    }
    body.truncate(length);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let cookie_ok = cookie_token(&head).is_some_and(|value| {
        shared.lock().ok().is_some_and(|guard| guard.token == value)
    });
    if path == "/" && method == "GET" {
        let presented = query_token(query);
        let expected = shared.lock().map_err(|_| io::Error::other("preference lock"))?;
        if presented.as_deref() != Some(expected.token.as_str()) {
            return write_status(&mut stream, 403, "application/json", b"{\"error\":\"forbidden\"}");
        }
        let file = expected.assets.join("index.html");
        drop(expected);
        let page = std::fs::read(&file).unwrap_or_else(|_| b"missing official gui".to_vec());
        let token = shared.lock().map_err(|_| io::Error::other("preference lock"))?.token.clone();
        return write_page(&mut stream, &page, &token);
    }
    if !cookie_ok && path != "/close-request" {
        return write_status(&mut stream, 403, "application/json", b"{\"error\":\"forbidden\"}");
    }
    match (method, path) {
        ("GET", "/state") => state_response(&mut stream, shared),
        ("GET", "/close-request") => close_request(&mut stream, shared),
        ("POST", "/command") => command_response(&mut stream, shared, action, &body),
        ("GET", "/shell.js") => file_response(&mut stream, shared, "shell.js", "text/javascript; charset=utf-8"),
        ("GET", "/layout.css") => file_response(&mut stream, shared, "layout.css", "text/css; charset=utf-8"),
        ("GET", "/glass.css") => file_response(&mut stream, shared, "glass.css", "text/css; charset=utf-8"),
        ("GET", "/NOTICES.txt") => file_response(&mut stream, shared, "NOTICES.txt", "text/plain; charset=utf-8"),
        _ => write_status(&mut stream, 404, "application/json", b"{\"error\":\"not_found\"}"),
    }
}

fn state_response(stream: &mut TcpStream, shared: &Mutex<Shared>) -> io::Result<()> {
    let guard = shared.lock().map_err(|_| io::Error::other("preference lock"))?;
    let observed = observe_at(&guard.locator, guard.endpoint.as_ref());
    let body = json!({
        "theme": guard.preferences.theme,
        "language": guard.preferences.language,
        "close": guard.preferences.close,
        "closePending": guard.close_pending,
        "user": observed.user,
        "storage": observed.storage,
        "run": observed.run,
        "service": observed.service,
        "plugins": observed.plugins.iter().map(|plugin| json!({
            "id": plugin.id,
            "name": plugin.name,
            "availability": plugin.availability,
            "enabled": plugin.enabled,
        })).collect::<Vec<_>>(),
        "privilege": observed.privilege,
    });
    drop(guard);
    let bytes = serde_json::to_vec(&body).map_err(|error| io::Error::other(error.to_string()))?;
    write_status(stream, 200, "application/json", &bytes)
}

fn close_request(stream: &mut TcpStream, shared: &Mutex<Shared>) -> io::Result<()> {
    let mut guard = shared.lock().map_err(|_| io::Error::other("preference lock"))?;
    match guard.preferences.close {
        Some(ClosePreference::Tray) => {
            write_status(stream, 200, "application/json", b"{\"action\":\"hide\"}")
        }
        Some(ClosePreference::Exit) => {
            write_status(stream, 200, "application/json", b"{\"action\":\"exit\"}")
        }
        Some(ClosePreference::Ask) | None => {
            guard.close_pending = true;
            write_status(stream, 200, "application/json", b"{\"action\":\"ask\"}")
        }
    }
}

fn command_response(
    stream: &mut TcpStream,
    shared: &Mutex<Shared>,
    action: &AtomicU8,
    body: &[u8],
) -> io::Result<()> {
    let command: Command = match serde_json::from_slice(body) {
        Ok(command) => command,
        Err(_) => {
            return write_status(stream, 400, "application/json", b"{\"error\":\"invalid_record\"}");
        }
    };
    if matches!(
        command.command.as_str(),
        "subscribe" | "release" | "audit" | "uninstall" | "delete" | "rescue" | "grant"
            | "start-live" | "enable" | "disable"
    ) {
        return write_status(stream, 403, "application/json", b"{\"error\":\"forbidden\"}");
    }
    let mut guard = shared.lock().map_err(|_| io::Error::other("preference lock"))?;
    match command.command.as_str() {
        "set-theme" => {
            let Some(theme) = command.theme else {
                return write_status(stream, 400, "application/json", b"{\"error\":\"invalid_record\"}");
            };
            guard.preferences.theme = Some(theme);
        }
        "set-language" => {
            let Some(language) = command.language else {
                return write_status(stream, 400, "application/json", b"{\"error\":\"invalid_record\"}");
            };
            guard.preferences.language = Some(language);
        }
        "set-close" => {
            let Some(close) = command.close else {
                return write_status(stream, 400, "application/json", b"{\"error\":\"invalid_record\"}");
            };
            guard.preferences.close = Some(close);
            if guard.close_pending {
                action.store(close_action(close), Ordering::Release);
                guard.close_pending = false;
            }
        }
        _ => return write_status(stream, 404, "application/json", b"{\"error\":\"not_found\"}"),
    }
    if let Err(error) = guard.preferences.save(&guard.directory) {
        let body = format!("{{\"error\":\"{error}\"}}");
        return write_status(stream, 500, "application/json", &body.into_bytes());
    }
    write_status(stream, 200, "application/json", b"{\"ok\":true}")
}

fn close_action(close: ClosePreference) -> u8 {
    match close {
        ClosePreference::Tray => ACTION_HIDE,
        ClosePreference::Exit => ACTION_QUIT,
        ClosePreference::Ask => 0,
    }
}

fn file_response(stream: &mut TcpStream, shared: &Mutex<Shared>, name: &str, content_type: &str) -> io::Result<()> {
    let guard = shared.lock().map_err(|_| io::Error::other("preference lock"))?;
    let bytes = std::fs::read(guard.assets.join(name))?;
    drop(guard);
    write_status(stream, 200, content_type, &bytes)
}

fn write_page(stream: &mut TcpStream, page: &[u8], token: &str) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nSet-Cookie: winstrata_gui={token}; HttpOnly; SameSite=Strict; Path=/\r\nConnection: close\r\n\r\n",
        page.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(page)?;
    Ok(())
}

fn write_status(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn cookie_token(head: &str) -> Option<String> {
    let cookie = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("cookie").then(|| value.trim().to_owned())
    })?;
    for part in cookie.split(';') {
        let (name, value) = part.trim().split_once('=')?;
        if name == "winstrata_gui" {
            return Some(value.to_owned());
        }
    }
    None
}

fn query_token(query: &str) -> Option<String> {
    for part in query.split('&') {
        let (name, value) = part.split_once('=')?;
        if name == "token" && !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    command: String,
    theme: Option<Theme>,
    language: Option<Language>,
    close: Option<ClosePreference>,
}

fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    #[cfg(unix)]
    {
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
        };
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status != 0 {
            return Err(io::Error::other(format!("random generation failed ({status:#x})")));
        }
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub const HIDE: u8 = ACTION_HIDE;
#[cfg_attr(not(windows), allow(dead_code))]
pub const QUIT: u8 = ACTION_QUIT;
