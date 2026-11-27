use std::collections::{HashSet, VecDeque};
use std::io::{self, Read, Write};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use winstrata_kernel::{Driver, DriverView, HostError, Phase};
use winstrata_protocols::{Id, NativeError, ProtocolError, Version};

pub const MAX_FRAME: usize = 16 * 1024;
pub const AUDIT_CAP: usize = 64;
const MAX_JSON_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Core,
    Gui,
    Plugin,
    Script,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Gui => "gui",
            Self::Plugin => "plugin",
            Self::Script => "script",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Status,
    Subscribe,
    Release,
    Audit,
}

impl Operation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Subscribe => "subscribe",
            Self::Release => "release",
            Self::Audit => "audit",
        }
    }

    pub(crate) fn changes_driver(self) -> bool {
        matches!(self, Self::Subscribe | Self::Release)
    }
}

pub(crate) struct Request {
    pub id: Id,
    pub role: Role,
    pub user: String,
    pub operation: Operation,
    pub driver: Option<Driver>,
}

#[derive(Debug, Deserialize)]
struct Header {
    version: Version,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire {
    version: Version,
    id: Id,
    role: Role,
    user: String,
    operation: Operation,
    driver: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverState {
    pub driver: String,
    pub subscriptions: u32,
    pub phase: String,
    #[serde(with = "decimal")]
    pub generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRecord {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    pub code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub drivers: [DriverState; 4],
    pub audit_dropped: u64,
    pub faults: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditPage {
    pub records: Vec<AuditRecord>,
    pub audit_dropped: u64,
    pub faults: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyWire {
    version: Version,
    id: String,
    #[serde(with = "decimal")]
    audit_dropped: u64,
    #[serde(with = "decimal")]
    faults: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drivers: Option<Vec<DriverState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audit: Option<Vec<AuditRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ProtocolError>,
}

pub(crate) struct Reply {
    pub id: String,
    pub error: Option<ProtocolError>,
    pub drivers: Option<[DriverState; 4]>,
    pub audit: Option<Vec<AuditRecord>>,
    pub audit_dropped: u64,
    pub faults: u64,
}

pub(crate) fn read_frame(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds the service limit",
        ));
    }
    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

pub(crate) fn write_frame(writer: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.is_empty() || payload.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds the service limit",
        ));
    }
    writer.write_all(&(payload.len() as u32).to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()
}

pub(crate) fn parse_request(input: &str) -> Result<Request, ProtocolError> {
    check_json(input)?;
    let header: Header = serde_json::from_str(input).map_err(json_error)?;
    header.version.check()?;
    let wire: RequestWire = serde_json::from_str(input).map_err(json_error)?;
    let _ = wire.version;
    validate_user(&wire.user)?;
    let driver = match wire.driver {
        Some(name) => Some(Driver::parse(&name).ok_or_else(|| invalid("Unknown driver"))?),
        None => None,
    };
    match (wire.operation.changes_driver(), driver) {
        (true, Some(_)) | (false, None) => {}
        (true, None) => return Err(invalid("This operation requires a driver")),
        (false, Some(_)) => return Err(invalid("This operation does not accept a driver")),
    }
    Ok(Request {
        id: wire.id,
        role: wire.role,
        user: wire.user,
        operation: wire.operation,
        driver,
    })
}

fn validate_user(user: &str) -> Result<(), ProtocolError> {
    if user.is_empty()
        || user.len() > 256
        || user.trim() != user
        || user.chars().any(char::is_control)
    {
        return Err(invalid(
            "User identity must occupy 1 to 256 bytes without surrounding whitespace or control characters",
        ));
    }
    Ok(())
}

pub(crate) fn encode_request(request: &Request) -> Result<Vec<u8>, ProtocolError> {
    #[derive(Serialize)]
    #[serde(deny_unknown_fields)]
    struct Wire<'a> {
        version: Version,
        id: &'a Id,
        role: Role,
        user: &'a str,
        operation: Operation,
        driver: Option<&'a str>,
    }
    let json = serde_json::to_string(&Wire {
        version: Version::CURRENT,
        id: &request.id,
        role: request.role,
        user: &request.user,
        operation: request.operation,
        driver: request.driver.map(Driver::as_str),
    })
    .map_err(json_error)?;
    Ok(json.into_bytes())
}

pub(crate) fn encode_reply(reply: &Reply) -> Result<Vec<u8>, ProtocolError> {
    let wire = ReplyWire {
        version: Version::CURRENT,
        id: reply.id.clone(),
        audit_dropped: reply.audit_dropped,
        faults: reply.faults,
        drivers: reply.drivers.as_ref().map(|items| items.to_vec()),
        audit: reply.audit.clone(),
        error: reply.error.clone(),
    };
    let json = serde_json::to_string(&wire).map_err(json_error)?;
    if json.len() > MAX_FRAME {
        return Err(invalid("Service reply exceeds the frame limit"));
    }
    Ok(json.into_bytes())
}

pub(crate) fn decode_reply(input: &str, expected_id: &str) -> Result<Reply, ProtocolError> {
    check_json(input)?;
    let header: Header = serde_json::from_str(input).map_err(json_error)?;
    header.version.check()?;
    let wire: ReplyWire = serde_json::from_str(input).map_err(json_error)?;
    if wire.id != expected_id {
        return Err(invalid("Response does not match the request"));
    }
    if let Some(error) = &wire.error {
        error.version.check()?;
    }
    let drivers = match wire.drivers {
        Some(items) => Some(driver_states(items)?),
        None => None,
    };
    if let Some(records) = &wire.audit {
        if records.len() > AUDIT_CAP {
            return Err(invalid("Audit page exceeds its limit"));
        }
    }
    Ok(Reply {
        id: wire.id,
        error: wire.error,
        drivers,
        audit: wire.audit,
        audit_dropped: wire.audit_dropped,
        faults: wire.faults,
    })
}

fn driver_states(items: Vec<DriverState>) -> Result<[DriverState; 4], ProtocolError> {
    if items.len() != 4 {
        return Err(invalid("Driver status must list the four catalog modules"));
    }
    let mut ordered = Vec::new();
    for driver in Driver::ALL {
        let item = items
            .iter()
            .find(|item| item.driver == driver.as_str())
            .ok_or_else(|| invalid("Driver status is missing a catalog module"))?;
        parse_phase(&item.phase)?;
        if let Some(code) = &item.failure {
            if !known_failure(code) {
                return Err(invalid("Unknown driver failure code"));
            }
        }
        ordered.push(item.clone());
    }
    ordered
        .try_into()
        .map_err(|_| invalid("Driver status must list the four catalog modules"))
}

fn known_failure(code: &str) -> bool {
    matches!(
        code,
        "image_absent" | "subscription_limit" | "no_subscription" | "driver_fault" | "driver_os"
    )
}

pub(crate) fn parse_phase(phase: &str) -> Result<Phase, ProtocolError> {
    match phase {
        "idle" => Ok(Phase::Idle),
        "ready" => Ok(Phase::Ready),
        "failed" => Ok(Phase::Failed),
        "unload_failed" => Ok(Phase::UnloadFailed),
        _ => Err(invalid("Unknown driver phase")),
    }
}

pub(crate) fn driver_state(view: &DriverView) -> DriverState {
    DriverState {
        driver: view.driver.as_str().to_owned(),
        subscriptions: view.subscriptions,
        phase: match view.phase {
            Phase::Idle => "idle",
            Phase::Ready => "ready",
            Phase::Failed => "failed",
            Phase::UnloadFailed => "unload_failed",
        }
        .to_owned(),
        generation: view.generation,
        failure: view
            .failure
            .as_ref()
            .map(HostError::code)
            .map(str::to_owned),
    }
}

pub(crate) fn host_error(error: &HostError) -> ProtocolError {
    let mut protocol = service_error(error.code(), &error.detail());
    if let HostError::Os(code) = error {
        if let (Ok(domain), Ok(native)) = (Id::new("os"), Id::new(code.to_string())) {
            protocol.native = Some(NativeError {
                domain,
                code: native,
            });
        }
    }
    protocol
}

pub(crate) fn service_error(code: &str, detail: &str) -> ProtocolError {
    ProtocolError {
        version: Version::CURRENT,
        code: Id::new(code).expect("stable service code"),
        detail: detail.to_owned(),
        native: None,
    }
}

pub(crate) fn invalid(detail: &str) -> ProtocolError {
    service_error("invalid_record", detail)
}

fn json_error(error: serde_json::Error) -> ProtocolError {
    use serde_json::error::Category;
    match error.classify() {
        Category::Syntax | Category::Eof => service_error(
            "invalid_json",
            &format!(
                "Invalid JSON at line {}, column {}",
                error.line(),
                error.column()
            ),
        ),
        Category::Data | Category::Io => invalid("Record does not match the service protocol"),
    }
}

pub(crate) fn snapshot(reply: Reply) -> Result<Snapshot, ProtocolError> {
    if let Some(error) = reply.error {
        return Err(error);
    }
    let drivers = reply
        .drivers
        .ok_or_else(|| invalid("Driver status is missing"))?;
    Ok(Snapshot {
        drivers,
        audit_dropped: reply.audit_dropped,
        faults: reply.faults,
    })
}

pub(crate) fn audit_page(reply: Reply) -> Result<AuditPage, ProtocolError> {
    if let Some(error) = reply.error {
        return Err(error);
    }
    Ok(AuditPage {
        records: reply
            .audit
            .ok_or_else(|| invalid("Audit page is missing"))?,
        audit_dropped: reply.audit_dropped,
        faults: reply.faults,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct AuditLog {
    records: VecDeque<AuditRecord>,
    dropped: u64,
}

impl AuditLog {
    pub(crate) fn new() -> Self {
        Self {
            records: VecDeque::new(),
            dropped: 0,
        }
    }

    pub(crate) fn push(&mut self, record: AuditRecord) {
        if self.records.len() == AUDIT_CAP {
            self.records.pop_front();
            self.dropped += 1;
        }
        self.records.push_back(record);
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn records(&self) -> Vec<AuditRecord> {
        self.records.iter().cloned().collect()
    }
}

pub(crate) fn audit_record(request: &Request, code: &str) -> AuditRecord {
    AuditRecord {
        id: request.id.as_str().to_owned(),
        role: Some(request.role.as_str().to_owned()),
        operation: Some(request.operation.as_str().to_owned()),
        driver: request.driver.map(|driver| driver.as_str().to_owned()),
        code: code.to_owned(),
    }
}

mod decimal {
    use super::*;

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if !canonical_u64(&text) {
            return Err(serde::de::Error::custom(
                "expected a canonical u64 decimal string",
            ));
        }
        text.parse()
            .map_err(|_| serde::de::Error::custom("expected a canonical u64 decimal string"))
    }
}

fn canonical_u64(text: &str) -> bool {
    if text == "0" {
        return true;
    }
    matches!(text.as_bytes().first(), Some(b'1'..=b'9'))
        && text.bytes().all(|byte| byte.is_ascii_digit())
}

struct Parser<'a> {
    input: &'a str,
    index: usize,
    depth: usize,
}

pub(crate) fn check_json(input: &str) -> Result<(), ProtocolError> {
    let mut parser = Parser {
        input,
        index: 0,
        depth: 0,
    };
    parser.value()?;
    parser.skip_ws();
    if parser.index != parser.input.len() {
        return Err(syntax(&parser, "Trailing data after JSON value"));
    }
    Ok(())
}

impl<'a> Parser<'a> {
    fn value(&mut self) -> Result<(), ProtocolError> {
        if self.depth == MAX_JSON_DEPTH {
            return Err(invalid("JSON nesting exceeds the service limit"));
        }
        self.skip_ws();
        match self.peek() {
            Some('{') => {
                self.depth += 1;
                self.object()?;
                self.depth -= 1;
                Ok(())
            }
            Some('[') => {
                self.depth += 1;
                self.array()?;
                self.depth -= 1;
                Ok(())
            }
            Some('"') => {
                self.string()?;
                Ok(())
            }
            Some('t') => self.literal("true"),
            Some('f') => self.literal("false"),
            Some('n') => self.literal("null"),
            Some('-') | Some('0'..='9') => self.number(),
            _ => Err(syntax(self, "Invalid JSON")),
        }
    }

    fn object(&mut self) -> Result<(), ProtocolError> {
        self.consume('{')?;
        self.skip_ws();
        if self.consume_if('}') {
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err(syntax(self, "JSON object key must be a string"));
            }
            let key = self.string()?;
            if !keys.insert(key) {
                return Err(invalid("Duplicate JSON key"));
            }
            self.skip_ws();
            self.consume(':')?;
            self.value()?;
            self.skip_ws();
            if self.consume_if(',') {
                continue;
            }
            if self.consume_if('}') {
                return Ok(());
            }
            return Err(syntax(self, "JSON object is not closed"));
        }
    }

    fn array(&mut self) -> Result<(), ProtocolError> {
        self.consume('[')?;
        self.skip_ws();
        if self.consume_if(']') {
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            if self.consume_if(',') {
                continue;
            }
            if self.consume_if(']') {
                return Ok(());
            }
            return Err(syntax(self, "JSON array is not closed"));
        }
    }

    fn string(&mut self) -> Result<String, ProtocolError> {
        self.consume('"')?;
        let mut value = String::new();
        loop {
            let Some(ch) = self.peek() else {
                return Err(syntax(self, "Unterminated JSON string"));
            };
            match ch {
                '"' => {
                    self.index += 1;
                    return Ok(value);
                }
                '\\' => {
                    self.index += 1;
                    value.push(self.escape()?);
                }
                ch if (ch as u32) < 0x20 => {
                    return Err(syntax(self, "Unescaped control character"));
                }
                ch => {
                    self.index += ch.len_utf8();
                    value.push(ch);
                }
            }
        }
    }

    fn escape(&mut self) -> Result<char, ProtocolError> {
        let Some(ch) = self.bump() else {
            return Err(syntax(self, "Unterminated JSON escape"));
        };
        match ch {
            '"' | '\\' | '/' => Ok(ch),
            'b' => Ok('\u{0008}'),
            'f' => Ok('\u{000c}'),
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            'u' => self.unicode(),
            _ => Err(syntax(self, "Invalid JSON escape")),
        }
    }

    fn unicode(&mut self) -> Result<char, ProtocolError> {
        let unit = self.hex4()?;
        if (0xDC00..=0xDFFF).contains(&unit) {
            return Err(syntax(self, "Lone low surrogate"));
        }
        if (0xD800..=0xDBFF).contains(&unit) {
            if self.bump() != Some('\\') || self.bump() != Some('u') {
                return Err(syntax(
                    self,
                    "High surrogate is not followed by a low surrogate",
                ));
            }
            let low = self.hex4()?;
            if !(0xDC00..=0xDFFF).contains(&low) {
                return Err(syntax(
                    self,
                    "High surrogate is not followed by a low surrogate",
                ));
            }
            let decoded =
                0x10000 + (((u32::from(unit) - 0xD800) << 10) | (u32::from(low) - 0xDC00));
            return char::from_u32(decoded).ok_or_else(|| syntax(self, "Invalid Unicode escape"));
        }
        char::from_u32(u32::from(unit)).ok_or_else(|| syntax(self, "Invalid Unicode escape"))
    }

    fn hex4(&mut self) -> Result<u16, ProtocolError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let Some(ch) = self.bump() else {
                return Err(syntax(self, "Short Unicode escape"));
            };
            let digit = match ch {
                '0'..='9' => ch as u16 - '0' as u16,
                'a'..='f' => ch as u16 - 'a' as u16 + 10,
                'A'..='F' => ch as u16 - 'A' as u16 + 10,
                _ => return Err(syntax(self, "Invalid Unicode escape")),
            };
            value = value << 4 | digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<(), ProtocolError> {
        if self.consume_if('-') && !matches!(self.peek(), Some('0'..='9')) {
            return Err(syntax(self, "Invalid JSON number"));
        }
        match self.bump() {
            Some('0') => {}
            Some('1'..='9') => {
                while matches!(self.peek(), Some('0'..='9')) {
                    self.index += 1;
                }
            }
            _ => return Err(syntax(self, "Invalid JSON number")),
        }
        if self.consume_if('.') {
            if !matches!(self.peek(), Some('0'..='9')) {
                return Err(syntax(self, "Invalid JSON number"));
            }
            while matches!(self.peek(), Some('0'..='9')) {
                self.index += 1;
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.index += 1;
            if matches!(self.peek(), Some('+' | '-')) {
                self.index += 1;
            }
            if !matches!(self.peek(), Some('0'..='9')) {
                return Err(syntax(self, "Invalid JSON number"));
            }
            while matches!(self.peek(), Some('0'..='9')) {
                self.index += 1;
            }
        }
        Ok(())
    }

    fn literal(&mut self, expected: &str) -> Result<(), ProtocolError> {
        if self.input[self.index..].starts_with(expected) {
            self.index += expected.len();
            Ok(())
        } else {
            Err(syntax(self, "Invalid JSON literal"))
        }
    }

    fn consume(&mut self, expected: char) -> Result<(), ProtocolError> {
        if self.bump() == Some(expected) {
            Ok(())
        } else {
            Err(syntax(self, "Invalid JSON"))
        }
    }

    fn consume_if(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.index += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += ch.len_utf8();
        Some(ch)
    }
}

fn syntax(parser: &Parser<'_>, summary: &str) -> ProtocolError {
    let mut line = 1usize;
    let mut column = 1usize;
    for ch in parser.input[..parser.index].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    service_error(
        "invalid_json",
        &format!("{summary} at line {line}, column {column}"),
    )
}

pub(crate) fn next_id(value: u64) -> Result<Id, ProtocolError> {
    Id::new(value.to_string())
}

impl Request {
    pub(crate) fn allowed(&self) -> bool {
        match self.operation {
            Operation::Status => true,
            Operation::Audit | Operation::Subscribe | Operation::Release => self.role == Role::Core,
        }
    }
}

pub(crate) fn unreadable(code: &str) -> AuditRecord {
    AuditRecord {
        id: "rejected".to_owned(),
        role: None,
        operation: None,
        driver: None,
        code: code.to_owned(),
    }
}
