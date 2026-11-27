use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use winstrata_kernel::{CatalogHost, Driver, DriverHost, Manager};
use winstrata_protocols::ProtocolError;

use crate::protocol::{
    AuditLog, AuditPage, Operation, Reply, Request, Role, Snapshot, audit_page, audit_record,
    decode_reply, driver_state, encode_reply, encode_request, host_error, invalid, next_id,
    parse_request, read_frame, service_error, snapshot, unreadable, write_frame,
};
use crate::transport::{self, Connection, Endpoint, Privilege};

pub(crate) struct Session<H> {
    privilege: Privilege,
    owner: String,
    manager: Manager<H>,
    audit: AuditLog,
    faults: u64,
}

impl<H: DriverHost> Session<H> {
    #[cfg(test)]
    pub(crate) fn new(privilege: Privilege, owner: impl Into<String>, host: H) -> Self {
        Self {
            privilege,
            owner: owner.into(),
            manager: Manager::new(host),
            audit: AuditLog::new(),
            faults: 0,
        }
    }

    pub(crate) fn record_fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn host(&self) -> &H {
        self.manager.host()
    }
}

enum Outcome {
    Drivers,
    Audit,
}

pub(crate) fn dispatch<H: DriverHost>(
    session: &mut Session<H>,
    peer: &str,
    bytes: &[u8],
) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return frame(error_reply(
            session,
            "rejected",
            service_error("invalid_record", "Frame is not UTF-8"),
        ));
    };
    let request = match parse_request(text) {
        Ok(request) => request,
        Err(error) => {
            let code = error.code.as_str().to_owned();
            session.audit.push(unreadable(&code));
            return frame(error_reply(session, "rejected", error));
        }
    };
    let request_id = request.id.as_str().to_owned();
    let panic_audit = audit_record(&request, "driver_fault");
    let decided = panic::catch_unwind(AssertUnwindSafe(|| apply(session, peer, request)));
    match decided {
        Ok(reply) => frame(reply),
        Err(_) => {
            session.faults += 1;
            session.audit.push(panic_audit);
            frame(error_reply(
                session,
                &request_id,
                service_error("driver_fault", "Driver host panicked"),
            ))
        }
    }
}

fn apply<H: DriverHost>(session: &mut Session<H>, peer: &str, request: Request) -> Reply {
    if peer.is_empty() || peer != session.owner || request.user != peer {
        return complete(
            session,
            &request,
            Err(service_error(
                "identity_mismatch",
                "Peer identity does not match the service owner",
            )),
        );
    }
    if !request.allowed() {
        return complete(
            session,
            &request,
            Err(service_error(
                "permission_denied",
                "This role cannot perform the operation",
            )),
        );
    }
    if request.operation.changes_driver() && !session.privilege.can_manage_drivers() {
        return complete(
            session,
            &request,
            Err(service_error(
                "privilege_unavailable",
                "The service process cannot manage drivers",
            )),
        );
    }
    let result = match request.operation {
        Operation::Status => Ok(Outcome::Drivers),
        Operation::Audit => Ok(Outcome::Audit),
        Operation::Subscribe => match request.driver {
            Some(driver) => session
                .manager
                .subscribe(driver)
                .map(|_| Outcome::Drivers)
                .map_err(|error| host_error(&error)),
            None => Err(invalid("This operation requires a driver")),
        },
        Operation::Release => match request.driver {
            Some(driver) => session
                .manager
                .release(driver)
                .map(|_| Outcome::Drivers)
                .map_err(|error| host_error(&error)),
            None => Err(invalid("This operation requires a driver")),
        },
    };
    complete(session, &request, result)
}

fn complete<H: DriverHost>(
    session: &mut Session<H>,
    request: &Request,
    result: Result<Outcome, ProtocolError>,
) -> Reply {
    let code = match &result {
        Ok(_) => "allowed".to_owned(),
        Err(error) => error.code.as_str().to_owned(),
    };
    session.audit.push(audit_record(request, &code));
    let mut reply = Reply {
        id: request.id.as_str().to_owned(),
        error: None,
        drivers: None,
        audit: None,
        audit_dropped: session.audit.dropped(),
        faults: session.faults,
    };
    match result {
        Ok(Outcome::Drivers) => {
            reply.drivers = Some(session.manager.view().map(|view| driver_state(&view)));
        }
        Ok(Outcome::Audit) => reply.audit = Some(session.audit.records()),
        Err(error) => reply.error = Some(error),
    }
    reply
}

fn error_reply<H>(session: &Session<H>, id: &str, error: ProtocolError) -> Reply {
    Reply {
        id: id.to_owned(),
        error: Some(error),
        drivers: None,
        audit: None,
        audit_dropped: session.audit.dropped(),
        faults: session.faults,
    }
}

fn frame(reply: Reply) -> Vec<u8> {
    encode_reply(&reply).unwrap_or_else(|_| {
        encode_reply(&Reply {
            id: reply.id,
            error: Some(service_error(
                "invalid_record",
                "Service reply exceeds the frame limit",
            )),
            drivers: None,
            audit: None,
            audit_dropped: reply.audit_dropped,
            faults: reply.faults,
        })
        .unwrap_or_else(|_| br#"{"version":{"major":1,"minor":0},"id":"rejected","audit_dropped":"0","faults":"0","error":{"version":{"major":1,"minor":0},"code":"invalid_record","detail":"Service reply exceeds the frame limit"}}"#.to_vec())
    })
}

pub struct Service {
    endpoint: Endpoint,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Service {
    pub fn listen_catalog(endpoint: Endpoint) -> std::io::Result<Self> {
        Self::listen(endpoint, CatalogHost)
    }

    pub(crate) fn listen<H>(endpoint: Endpoint, host: H) -> std::io::Result<Self>
    where
        H: DriverHost + Send + 'static,
    {
        let session = Session {
            privilege: transport::current_privilege()?,
            owner: transport::current_user()?,
            manager: Manager::new(host),
            audit: AuditLog::new(),
            faults: 0,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread = transport::serve(&endpoint, session, Arc::clone(&stop))?;
        Ok(Self {
            endpoint,
            stop,
            thread: Some(thread),
        })
    }

    pub fn address(&self) -> &str {
        self.endpoint.address()
    }

    pub fn client_endpoint(&self) -> Endpoint {
        self.endpoint.anonymous()
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.endpoint.cleanup();
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Client {
    stream: Connection,
    user: String,
    role: Role,
    next_id: u64,
}

impl Client {
    fn connect(endpoint: &Endpoint, role: Role) -> std::io::Result<Self> {
        Ok(Self {
            stream: transport::connect(endpoint)?,
            user: transport::current_user()?,
            role,
            next_id: 1,
        })
    }

    fn call(
        &mut self,
        operation: Operation,
        driver: Option<Driver>,
    ) -> Result<Reply, ProtocolError> {
        let id = next_id(self.next_id)?;
        self.next_id += 1;
        let request = Request {
            id,
            role: self.role,
            user: self.user.clone(),
            operation,
            driver,
        };
        let payload = encode_request(&request)?;
        write_frame(&mut self.stream, &payload).map_err(transport_error)?;
        let response = read_frame(&mut self.stream).map_err(transport_error)?;
        let text = std::str::from_utf8(&response).map_err(|_| invalid("Frame is not UTF-8"))?;
        decode_reply(text, request.id.as_str())
    }
}

fn transport_error(error: std::io::Error) -> ProtocolError {
    use winstrata_protocols::{Id, NativeError, Version};
    let mut protocol = ProtocolError {
        version: Version::CURRENT,
        code: Id::new("transport").expect("stable service code"),
        detail: "Service transport failed".to_owned(),
        native: None,
    };
    if let Some(code) = error.raw_os_error() {
        let domain = if cfg!(windows) { "win32" } else { "errno" };
        if let (Ok(domain), Ok(native)) = (Id::new(domain), Id::new(code.to_string())) {
            protocol.native = Some(NativeError {
                domain,
                code: native,
            });
        }
    }
    protocol
}

pub struct CoreClient {
    inner: Client,
}

impl CoreClient {
    pub fn connect(endpoint: &Endpoint) -> std::io::Result<Self> {
        Ok(Self {
            inner: Client::connect(endpoint, Role::Core)?,
        })
    }

    pub fn status(&mut self) -> Result<Snapshot, ProtocolError> {
        snapshot(self.inner.call(Operation::Status, None)?)
    }

    pub fn subscribe(&mut self, driver: Driver) -> Result<Snapshot, ProtocolError> {
        snapshot(self.inner.call(Operation::Subscribe, Some(driver))?)
    }

    pub fn release(&mut self, driver: Driver) -> Result<Snapshot, ProtocolError> {
        snapshot(self.inner.call(Operation::Release, Some(driver))?)
    }

    pub fn audit(&mut self) -> Result<AuditPage, ProtocolError> {
        audit_page(self.inner.call(Operation::Audit, None)?)
    }
}

pub struct GuiClient {
    inner: Client,
}

impl GuiClient {
    pub fn connect(endpoint: &Endpoint) -> std::io::Result<Self> {
        Ok(Self {
            inner: Client::connect(endpoint, Role::Gui)?,
        })
    }

    pub fn status(&mut self) -> Result<Snapshot, ProtocolError> {
        snapshot(self.inner.call(Operation::Status, None)?)
    }
}
