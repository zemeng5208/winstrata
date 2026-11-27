//! 桌面壳：导航、语言、主题、托盘，以及到受控服务的只读 IPC。
//!
//! 页面只通过本机回环地址访问宿主。宿主以 `gui` 角色查询服务状态，不订阅驱动。

mod http;
mod prefs;
mod session;
mod tray;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use http::{Server, UiPaths};
use session::Session;

pub struct Desktop {
    pub prefs: PathBuf,
    pub locator: Option<PathBuf>,
    pub service: Option<String>,
    pub runtime: Option<PathBuf>,
    pub ui_root: PathBuf,
    pub bundle: PathBuf,
    pub tray: bool,
}

pub struct Running {
    server: Server,
    tray: Option<tray::Tray>,
    _events: Arc<AtomicU8>,
}

impl Desktop {
    pub fn serve(self) -> io::Result<Running> {
        let events = Arc::new(AtomicU8::new(0));
        let (tray_state, tray) = if self.tray {
            match tray::install(Arc::clone(&events)) {
                Ok(tray) => ("ready", Some(tray)),
                Err(_) => ("unsupported", None),
            }
        } else {
            ("unsupported", None)
        };
        let session = Session::open(
            self.prefs,
            self.locator.as_deref(),
            self.service,
            self.runtime,
            http::bundle_ready(&self.bundle),
            tray_state,
        );
        let server = Server::start(
            session,
            UiPaths {
                root: self.ui_root,
                bundle: self.bundle,
            },
            events.clone(),
        )?;
        Ok(Running {
            server,
            tray,
            _events: events,
        })
    }
}

impl Running {
    pub fn url(&self, probe: bool) -> String {
        self.server.url(probe)
    }

    pub fn shutdown(&mut self) {
        self.server.shutdown();
        self.tray.take();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn tray_probe() -> io::Result<()> {
    tray::probe()
}

pub fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/ui")
}
