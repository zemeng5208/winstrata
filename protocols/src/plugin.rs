//! 插件清单与宿主进程之间的版本化契约。
//!
//! 字段使用命名 JSON 对象。清单描述身份、版本、能力和权限；帧只在宿主与该插件进程之间传递。

use std::io::{self, Read, Write};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Id, ProtocolError, Version, object, read_versioned};

pub const MAX_PLUGIN_FRAME: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluginVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PluginVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for PluginVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginKind {
    Builtin,
    ThirdParty,
}

impl PluginKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::ThirdParty => "third_party",
        }
    }

    fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "builtin" => Ok(Self::Builtin),
            "third_party" => Ok(Self::ThirdParty),
            _ => Err(ProtocolError::invalid("plugin kind is not recognized")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    EventPublish,
    EventSubscribe,
    HistoryRead,
    PluginCall,
    StoragePrivate,
}

impl Permission {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EventPublish => "event.publish",
            Self::EventSubscribe => "event.subscribe",
            Self::HistoryRead => "history.read",
            Self::PluginCall => "plugin.call",
            Self::StoragePrivate => "storage.private",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "event.publish" => Ok(Self::EventPublish),
            "event.subscribe" => Ok(Self::EventSubscribe),
            "history.read" => Ok(Self::HistoryRead),
            "plugin.call" => Ok(Self::PluginCall),
            "storage.private" => Ok(Self::StoragePrivate),
            _ => Err(ProtocolError::invalid("permission is not recognized")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionRange {
    Exact,
    Compatible,
}

impl VersionRange {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Compatible => "compatible",
        }
    }

    fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "exact" => Ok(Self::Exact),
            "compatible" => Ok(Self::Compatible),
            _ => Err(ProtocolError::invalid("dependency range is not recognized")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub id: Id,
    pub range: VersionRange,
    pub version: PluginVersion,
}

impl Dependency {
    pub fn matches(&self, installed: PluginVersion) -> bool {
        match self.range {
            VersionRange::Exact => installed == self.version,
            VersionRange::Compatible => compatible(installed, self.version),
        }
    }
}

/// 主版本 0 只接受同一次版本及不低于要求的修订；其余主版本接受同一主版本内不低于要求的版本。
pub fn compatible(installed: PluginVersion, required: PluginVersion) -> bool {
    if required.major == 0 {
        installed.major == 0
            && installed.minor == required.minor
            && installed.patch >= required.patch
    } else {
        installed.major == required.major
            && (installed.minor, installed.patch) >= (required.minor, required.patch)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub id: Id,
    pub name: Id,
    pub plugin_version: PluginVersion,
    pub kind: PluginKind,
    pub runtime: Id,
    pub capabilities: Vec<Id>,
    pub permissions: Vec<Permission>,
    pub dependencies: Vec<Dependency>,
    pub events: Vec<Id>,
    pub rescue: bool,
}

#[derive(Deserialize)]
struct WireManifest {
    #[serde(deserialize_with = "object::required")]
    version: Version,
    id: Id,
    name: Id,
    #[serde(deserialize_with = "object::required")]
    plugin_version: PluginVersion,
    kind: String,
    runtime: Id,
    capabilities: Vec<Id>,
    permissions: Vec<String>,
    #[serde(deserialize_with = "dependency_list")]
    dependencies: Vec<WireDependency>,
    events: Vec<Id>,
    rescue: bool,
}

#[derive(Deserialize)]
struct WireDependency {
    id: Id,
    range: String,
    #[serde(deserialize_with = "object::required")]
    version: PluginVersion,
}

fn dependency_list<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<WireDependency>, D::Error> {
    Vec::<object::Object<WireDependency>>::deserialize(deserializer)
        .map(|items| items.into_iter().map(|object::Object(item)| item).collect())
}

impl Manifest {
    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        read_versioned(input)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        self.version.check_writable()?;
        serde_json::to_string(self).map_err(ProtocolError::from_json_error)
    }

    fn from_wire(wire: WireManifest) -> Result<Self, ProtocolError> {
        wire.version.check()?;
        let kind = PluginKind::parse(&wire.kind)?;
        if !plugin_token(wire.id.as_str()) {
            return Err(ProtocolError::invalid(
                "plugin id must be a lowercase token of 1 to 64 bytes",
            ));
        }
        if !plugin_token(wire.runtime.as_str()) {
            return Err(ProtocolError::invalid(
                "runtime name must be a lowercase token of 1 to 64 bytes",
            ));
        }
        if wire.name.as_str().is_empty() {
            return Err(ProtocolError::invalid("plugin name is empty"));
        }
        let mut permissions = Vec::with_capacity(wire.permissions.len());
        for permission in &wire.permissions {
            let parsed = Permission::parse(permission)?;
            if permissions.contains(&parsed) {
                return Err(ProtocolError::invalid("permission is repeated"));
            }
            permissions.push(parsed);
        }
        if duplicate_ids(wire.capabilities.iter().map(Id::as_str))
            || duplicate_ids(wire.events.iter().map(Id::as_str))
            || duplicate_ids(wire.dependencies.iter().map(|item| item.id.as_str()))
        {
            return Err(ProtocolError::invalid(
                "capability, event, or dependency is repeated",
            ));
        }
        for id in wire
            .capabilities
            .iter()
            .chain(wire.events.iter())
            .chain(wire.dependencies.iter().map(|item| &item.id))
        {
            if !plugin_token(id.as_str()) {
                return Err(ProtocolError::invalid(
                    "capability, event, and dependency ids use the plugin token form",
                ));
            }
        }
        if wire.rescue && wire.id.as_str() != "strata.rescue" {
            return Err(ProtocolError::invalid(
                "rescue authority is isolated to strata.rescue",
            ));
        }
        let mut dependencies = Vec::with_capacity(wire.dependencies.len());
        for dependency in wire.dependencies {
            dependencies.push(Dependency {
                id: dependency.id,
                range: VersionRange::parse(&dependency.range)?,
                version: dependency.version,
            });
        }
        Ok(Self {
            version: wire.version,
            id: wire.id,
            name: wire.name,
            plugin_version: wire.plugin_version,
            kind,
            runtime: wire.runtime,
            capabilities: wire.capabilities,
            permissions,
            dependencies,
            events: wire.events,
            rescue: wire.rescue,
        })
    }
}

fn plugin_token(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'.' || *byte == b'-'
        })
        && !value.contains("..")
        && !value.ends_with('.')
}

fn duplicate_ids<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = Vec::new();
    for id in ids {
        if seen.contains(&id) {
            return true;
        }
        seen.push(id);
    }
    false
}

impl<'de> Deserialize<'de> for Manifest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = object::required::<D, WireManifest>(deserializer)?;
        Self::from_wire(wire).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Manifest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.version
            .check_writable()
            .map_err(serde::ser::Error::custom)?;
        WireOut {
            version: self.version,
            id: &self.id,
            name: &self.name,
            plugin_version: self.plugin_version,
            kind: self.kind.as_str(),
            runtime: &self.runtime,
            capabilities: &self.capabilities,
            permissions: self.permissions.iter().map(|item| item.as_str()).collect(),
            dependencies: self
                .dependencies
                .iter()
                .map(|item| DependencyOut {
                    id: &item.id,
                    range: item.range.as_str(),
                    version: item.version,
                })
                .collect(),
            events: &self.events,
            rescue: self.rescue,
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct WireOut<'a> {
    version: Version,
    id: &'a Id,
    name: &'a Id,
    plugin_version: PluginVersion,
    kind: &'a str,
    runtime: &'a Id,
    capabilities: &'a [Id],
    permissions: Vec<&'a str>,
    dependencies: Vec<DependencyOut<'a>>,
    events: &'a [Id],
    rescue: bool,
}

#[derive(Serialize)]
struct DependencyOut<'a> {
    id: &'a Id,
    range: &'a str,
    version: PluginVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Startup {
    pub version: Version,
    pub ready: bool,
    pub reason: Option<Id>,
}

#[derive(Deserialize)]
struct WireStartup {
    #[serde(deserialize_with = "object::required")]
    version: Version,
    ready: bool,
    #[serde(default)]
    reason: Option<Id>,
}

impl Startup {
    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        read_versioned(input)
    }
}

impl<'de> Deserialize<'de> for Startup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = object::required::<D, WireStartup>(deserializer)?;
        wire.version.check().map_err(serde::de::Error::custom)?;
        if !wire.ready && wire.reason.is_none() {
            return Err(serde::de::Error::custom(
                "a plugin that declines startup must give a reason",
            ));
        }
        Ok(Self {
            version: wire.version,
            ready: wire.ready,
            reason: wire.reason,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginFrame {
    pub version: Version,
    pub op: String,
    pub ok: Option<bool>,
    pub code: Option<String>,
    pub detail: Option<String>,
    pub phase: Option<String>,
    pub capability: Option<String>,
    pub event_type: Option<String>,
    pub name: Option<String>,
    pub payload: Option<String>,
    pub pid: Option<u32>,
    pub plugin_version: Option<PluginVersion>,
}

#[derive(Deserialize)]
struct WireFrame {
    #[serde(deserialize_with = "object::required")]
    version: Version,
    op: String,
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    capability: Option<String>,
    #[serde(default)]
    event_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    payload: Option<String>,
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default, deserialize_with = "object::optional")]
    plugin_version: Option<PluginVersion>,
}

impl PluginFrame {
    pub fn request(op: &str) -> Self {
        Self {
            version: Version::CURRENT,
            op: op.to_owned(),
            ok: None,
            code: None,
            detail: None,
            phase: None,
            capability: None,
            event_type: None,
            name: None,
            payload: None,
            pid: None,
            plugin_version: None,
        }
    }

    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        read_versioned(input)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        self.version.check_writable()?;
        self.check_text()?;
        serde_json::to_string(self).map_err(ProtocolError::from_json_error)
    }

    fn check_text(&self) -> Result<(), ProtocolError> {
        if !frame_op(&self.op) {
            return Err(ProtocolError::invalid("plugin operation is not recognized"));
        }
        for value in [
            self.code.as_deref(),
            self.detail.as_deref(),
            self.phase.as_deref(),
            self.capability.as_deref(),
            self.event_type.as_deref(),
            self.name.as_deref(),
            self.payload.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 1024 || value.chars().any(char::is_control) {
                return Err(ProtocolError::invalid(
                    "plugin frame text is empty, too long, or contains a control character",
                ));
            }
        }
        Ok(())
    }
}

fn frame_op(value: &str) -> bool {
    matches!(
        value,
        "ping"
            | "pause"
            | "resume"
            | "stop"
            | "disable"
            | "emit"
            | "invoke"
            | "read_state"
            | "fault"
            | "result"
    )
}

impl<'de> Deserialize<'de> for PluginFrame {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = object::required::<D, WireFrame>(deserializer)?;
        let frame = Self {
            version: wire.version,
            op: wire.op,
            ok: wire.ok,
            code: wire.code,
            detail: wire.detail,
            phase: wire.phase,
            capability: wire.capability,
            event_type: wire.event_type,
            name: wire.name,
            payload: wire.payload,
            pid: wire.pid,
            plugin_version: wire.plugin_version,
        };
        frame.version.check().map_err(serde::de::Error::custom)?;
        frame.check_text().map_err(serde::de::Error::custom)?;
        Ok(frame)
    }
}

impl Serialize for PluginFrame {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.check_text().map_err(serde::ser::Error::custom)?;
        WireFrameOut {
            version: self.version,
            op: &self.op,
            ok: self.ok,
            code: self.code.as_deref(),
            detail: self.detail.as_deref(),
            phase: self.phase.as_deref(),
            capability: self.capability.as_deref(),
            event_type: self.event_type.as_deref(),
            name: self.name.as_deref(),
            payload: self.payload.as_deref(),
            pid: self.pid,
            plugin_version: self.plugin_version,
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct WireFrameOut<'a> {
    version: Version,
    op: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin_version: Option<PluginVersion>,
}

pub fn write_plugin_frame(mut writer: impl Write, body: &str) -> io::Result<()> {
    let bytes = body.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_PLUGIN_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin frame length is outside the supported range",
        ));
    }
    let len = u32::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin frame length is outside the supported range",
        )
    })?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()?;
    Ok(())
}

pub fn read_plugin_frame(mut reader: impl Read) -> io::Result<String> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_PLUGIN_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "plugin frame length is outside the supported range",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    String::from_utf8(body)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "plugin frame is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> serde_json::Value {
        json!({
            "version": {"major": 1, "minor": 0},
            "id": "fixture.alpha",
            "name": "Fixture Alpha",
            "plugin_version": {"major": 1, "minor": 2, "patch": 3},
            "kind": "third_party",
            "runtime": "native",
            "capabilities": ["fixture.echo"],
            "permissions": ["event.publish", "plugin.call"],
            "dependencies": [{
                "id": "fixture.base",
                "range": "compatible",
                "version": {"major": 1, "minor": 0, "patch": 0}
            }],
            "events": ["fixture.tick"],
            "rescue": false
        })
    }

    #[test]
    fn manifest_round_trip_preserves_dependencies_and_permissions() {
        let manifest = Manifest::from_json(&sample().to_string()).unwrap();
        assert_eq!(manifest.kind, PluginKind::ThirdParty);
        assert_eq!(
            manifest.permissions,
            vec![Permission::EventPublish, Permission::PluginCall]
        );
        assert!(manifest.dependencies[0].matches(PluginVersion::new(1, 4, 0)));
        assert!(!manifest.dependencies[0].matches(PluginVersion::new(2, 0, 0)));
        let again = Manifest::from_json(&manifest.to_json().unwrap()).unwrap();
        assert_eq!(again, manifest);
    }

    #[test]
    fn manifest_rejects_unknown_privilege_rescue_alias_and_positional_json() {
        let mut unknown = sample();
        unknown["permissions"] = json!(["event.publish", "rescue"]);
        assert_eq!(
            Manifest::from_json(&unknown.to_string())
                .unwrap_err()
                .code
                .as_str(),
            "invalid_record"
        );
        let mut rescue = sample();
        rescue["rescue"] = json!(true);
        assert!(Manifest::from_json(&rescue.to_string()).is_err());
        assert!(Manifest::from_json(r#"["not","an","object"]"#).is_err());
        let mut future = sample();
        future["version"] = json!({"major": 2, "minor": 0});
        assert_eq!(
            Manifest::from_json(&future.to_string())
                .unwrap_err()
                .code
                .as_str(),
            "unsupported_version"
        );
    }

    #[test]
    fn zero_major_compatibility_stays_on_the_same_minor() {
        let required = PluginVersion::new(0, 2, 1);
        assert!(compatible(PluginVersion::new(0, 2, 1), required));
        assert!(compatible(PluginVersion::new(0, 2, 4), required));
        assert!(!compatible(PluginVersion::new(0, 3, 0), required));
    }
}
