use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use winstrata_kernel::DriverHost;

use super::{Endpoint, Privilege};
use crate::protocol::{read_frame, write_frame};
use crate::session::{self, Session};

pub(crate) struct Connection(UnixStream);

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

pub(super) fn current_user() -> io::Result<String> {
    Ok(format!("uid:{}", effective_user()))
}

pub(super) fn current_privilege() -> io::Result<Privilege> {
    Ok(Privilege::NotWindows)
}

pub(super) fn unique_endpoint() -> io::Result<Endpoint> {
    let dir = std::env::temp_dir().join(super::next_name("ws008"));
    fs::create_dir(&dir)?;
    set_mode(&dir, 0o700)?;
    let file = dir.join("s");
    let address = path_text(&file)?;
    if address.len() >= 100 {
        let _ = fs::remove_dir(&dir);
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "endpoint path exceeds the socket limit",
        ));
    }
    Ok(Endpoint {
        address,
        owned_file: Some(file),
        owned_dir: Some(dir),
    })
}

pub(super) fn user_endpoint() -> io::Result<Endpoint> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "XDG_RUNTIME_DIR is not set"))?;
    let runtime = PathBuf::from(runtime);
    private_directory(&runtime)?;
    let dir = runtime.join("winstrata");
    fs::create_dir_all(&dir)?;
    set_mode(&dir, 0o700)?;
    private_directory(&dir)?;
    let file = dir.join("service.sock");
    if file.symlink_metadata().is_ok() {
        // 名称可能来自上一次进程。只移除套接字名，不终止仍持有旧 inode 的进程。
        fs::remove_file(&file)?;
    }
    Ok(Endpoint {
        address: path_text(&file)?,
        owned_file: None,
        owned_dir: None,
    })
}

pub(super) fn connect(address: &str) -> io::Result<Connection> {
    let stream = UnixStream::connect(address)?;
    stream.set_read_timeout(Some(super::IO_TIMEOUT))?;
    stream.set_write_timeout(Some(super::IO_TIMEOUT))?;
    Ok(Connection(stream))
}

pub(super) fn serve<H>(
    address: &str,
    session: Session<H>,
    stop: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>>
where
    H: DriverHost + Send + 'static,
{
    if Path::new(address).symlink_metadata().is_ok() {
        return Err(io::Error::new(
            ErrorKind::AlreadyExists,
            "service endpoint already exists",
        ));
    }
    let listener = UnixListener::bind(address)?;
    set_mode(Path::new(address), 0o600)?;
    listener.set_nonblocking(true)?;
    let shared = Arc::new(Mutex::new(session));
    Ok(thread::spawn(move || accept_loop(listener, shared, stop)))
}

fn accept_loop<H>(listener: UnixListener, shared: Arc<Mutex<Session<H>>>, stop: Arc<AtomicBool>)
where
    H: DriverHost,
{
    while !stop.load(Ordering::Acquire) {
        if !wait_readable(listener.as_raw_fd(), 200) {
            continue;
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        let Ok((stream, _)) = listener.accept() else {
            continue;
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(super::IO_TIMEOUT));
        let _ = stream.set_write_timeout(Some(super::IO_TIMEOUT));
        let peer = match peer_user(&stream) {
            Ok(peer) => peer,
            Err(_) => {
                lock(&shared).record_fault();
                continue;
            }
        };
        conversation(stream, &shared, &peer);
    }
}

fn conversation<H>(mut stream: UnixStream, shared: &Mutex<Session<H>>, peer: &str)
where
    H: DriverHost,
{
    loop {
        let input = match read_frame(&mut stream) {
            Ok(input) => input,
            Err(_) => return,
        };
        let output = {
            let mut session = lock(shared);
            session::dispatch(&mut session, peer, &input)
        };
        if write_frame(&mut stream, &output).is_err() {
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

fn peer_user(stream: &UnixStream) -> io::Result<String> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = size_of::<libc::ucred>() as libc::socklen_t;
    // SO_PEERCRED 由内核填写连接对端的有效用户，不读取请求里的身份字符串。
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    if len != size_of::<libc::ucred>() as libc::socklen_t || cred.uid != effective_user() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "peer user does not own the service",
        ));
    }
    Ok(format!("uid:{}", cred.uid))
}

fn wait_readable(fd: i32, timeout_ms: i32) -> bool {
    let mut poll = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let ready = unsafe { libc::poll(&mut poll, 1, timeout_ms) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                continue;
            }
            return false;
        }
        return ready > 0;
    }
}

fn effective_user() -> u32 {
    unsafe { libc::geteuid() }
}

fn private_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() || metadata.uid() != effective_user() || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "service directory must be private to the current user",
        ));
    }
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
}

fn path_text(path: &Path) -> io::Result<String> {
    path.to_str()
        .filter(|text| !text.is_empty() && !text.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "endpoint path is not Unicode"))
}
