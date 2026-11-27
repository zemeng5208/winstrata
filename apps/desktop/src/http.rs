use std::fs;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Deserialize;

use crate::session::Session;

pub struct Server {
    pub address: SocketAddr,
    pub token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub struct UiPaths {
    pub root: PathBuf,
    pub bundle: PathBuf,
}

impl Server {
    pub fn start(session: Session, paths: UiPaths, tray_event: Arc<AtomicU8>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let token = fresh_token()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_token = token.clone();
        let thread = std::thread::Builder::new()
            .name("ws-desktop".to_owned())
            .spawn(move || {
                serve(
                    listener,
                    session,
                    paths,
                    thread_token,
                    thread_stop,
                    tray_event,
                )
            })
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(Self {
            address,
            token,
            stop,
            thread: Some(thread),
        })
    }

    pub fn url(&self, probe: bool) -> String {
        let query = if probe { "?probe=1" } else { "" };
        format!("http://{}/{}/{query}", self.address, self.token)
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Deserialize)]
struct Command {
    action: String,
    value: Option<String>,
    #[serde(default)]
    remember: bool,
}

fn serve(
    listener: TcpListener,
    mut session: Session,
    paths: UiPaths,
    token: String,
    stop: Arc<AtomicBool>,
    tray_event: Arc<AtomicU8>,
) {
    let _ = listener.set_nonblocking(false);
    while !stop.load(Ordering::Acquire) {
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(_) => break,
        };
        if stop.load(Ordering::Acquire) {
            break;
        }
        let event = tray_event.swap(0, Ordering::AcqRel);
        session.observe_tray(event);
        if handle(&mut stream, &mut session, &paths, &token).unwrap_or(false) {
            stop.store(true, Ordering::Release);
            break;
        }
    }
}

fn handle(
    stream: &mut TcpStream,
    session: &mut Session,
    paths: &UiPaths,
    token: &str,
) -> io::Result<bool> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let (method, path, _query, body) = match read_request(stream) {
        Ok(request) => request,
        Err(_) => {
            respond(stream, 400, "Bad Request", "text/plain", b"bad request")?;
            return Ok(false);
        }
    };
    let Some(route) = route(token, &path) else {
        respond(stream, 404, "Not Found", "text/plain", b"not found")?;
        return Ok(false);
    };
    match (method.as_str(), route) {
        ("GET", "") => {
            let page = fs::read(paths.root.join("static/page.html"))?;
            respond(stream, 200, "OK", "text/html; charset=utf-8", &page)?;
        }
        ("GET", "api/state") => {
            let body = serde_json::to_vec(&session.view()).unwrap_or_else(|_| b"{}".to_vec());
            respond(stream, 200, "OK", "application/json", &body)?;
        }
        ("POST", "api/command") => {
            let command: Command = match serde_json::from_slice(&body) {
                Ok(command) => command,
                Err(_) => {
                    respond(stream, 400, "Bad Request", "text/plain", b"invalid command")?;
                    return Ok(false);
                }
            };
            let exit = session.command(&command.action, command.value.as_deref(), command.remember);
            let body = serde_json::to_vec(&session.view()).unwrap_or_else(|_| b"{}".to_vec());
            respond(stream, 200, "OK", "application/json", &body)?;
            return Ok(exit);
        }
        ("GET", name) => {
            if let Some((file, kind)) = static_file(paths, name) {
                match fs::read(&file) {
                    Ok(bytes) => respond(stream, 200, "OK", kind, &bytes)?,
                    Err(_) => respond(stream, 404, "Not Found", "text/plain", b"not found")?,
                }
            } else {
                respond(stream, 404, "Not Found", "text/plain", b"not found")?;
            }
        }
        _ => respond(stream, 405, "Method Not Allowed", "text/plain", b"method")?,
    }
    Ok(false)
}

fn route<'a>(token: &str, path: &'a str) -> Option<&'a str> {
    let path = path.strip_prefix('/')?;
    let (prefix, rest) = path.split_once('/').unwrap_or((path, ""));
    if prefix != token {
        return None;
    }
    Some(rest.trim_end_matches('/'))
}

fn static_file(paths: &UiPaths, name: &str) -> Option<(PathBuf, &'static str)> {
    let (path, kind) = match name {
        "boot.js" => (
            paths.root.join("static/boot.js"),
            "text/javascript; charset=utf-8",
        ),
        "shell.js" => (
            paths.root.join("static/shell.js"),
            "text/javascript; charset=utf-8",
        ),
        "model.mjs" => (
            paths.root.join("model.mjs"),
            "text/javascript; charset=utf-8",
        ),
        "messages.json" => (paths.root.join("static/messages.json"), "application/json"),
        "themes.css" => (
            paths.root.join("static/themes.css"),
            "text/css; charset=utf-8",
        ),
        "glass.css" => (
            paths.root.join("vendor/glasswork/glass.css"),
            "text/css; charset=utf-8",
        ),
        "layout.css" => (paths.bundle.join("layout.css"), "text/css; charset=utf-8"),
        "amicro.js" => (
            paths.bundle.join("amicro.js"),
            "text/javascript; charset=utf-8",
        ),
        "shadcn.js" => (
            paths.bundle.join("shadcn.js"),
            "text/javascript; charset=utf-8",
        ),
        "glass.js" => (
            paths.bundle.join("glass.js"),
            "text/javascript; charset=utf-8",
        ),
        "NOTICES.txt" => (
            paths.bundle.join("NOTICES.txt"),
            "text/plain; charset=utf-8",
        ),
        _ => return None,
    };
    Some((path, kind))
}

fn read_request(stream: &mut TcpStream) -> io::Result<(String, String, String, Vec<u8>)> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];
    let split = loop {
        if buffer.len() > 16 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "headers are too large",
            ));
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed"));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
    };
    let header = std::str::from_utf8(&buffer[..split])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "headers are not utf-8"))?;
    let mut lines = header.lines();
    let request = lines.next().unwrap_or("");
    let mut parts = request.split(' ');
    let method = parts.next().unwrap_or("").to_owned();
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut length = 0usize;
    for line in lines {
        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            length = value.trim().parse().unwrap_or(usize::MAX);
        }
    }
    if length > 8192 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "body is too large",
        ));
    }
    let mut body = buffer[split + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "body closed"));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    Ok((method, path.to_owned(), query.to_owned(), body))
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    if body.len() > 8 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response is too large",
        ));
    }
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}

fn fresh_token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(unix)]
fn fill_random(bytes: &mut [u8]) -> io::Result<()> {
    let mut file = fs::File::open("/dev/urandom")?;
    file.read_exact(bytes)
}

#[cfg(windows)]
fn fill_random(bytes: &mut [u8]) -> io::Result<()> {
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
        return Err(io::Error::other(format!(
            "random generation failed ({status:#x})"
        )));
    }
    Ok(())
}

pub fn bundle_ready(bundle: &Path) -> bool {
    [
        "amicro.js",
        "shadcn.js",
        "glass.js",
        "layout.css",
        "NOTICES.txt",
    ]
    .iter()
    .all(|name| bundle.join(name).is_file())
}
