#![forbid(unsafe_code)]

mod error;
mod event;
mod object;
mod plugin;
mod scalars;

pub use error::{NativeError, ProtocolError};
pub use event::{
    Attribution, Completeness, Event, EventTime, GapReason, ObjectRef, ProcessRef, Record,
    SequenceRange, Source,
};
pub use plugin::{
    Dependency, MAX_PLUGIN_FRAME, Manifest, Permission, PluginFrame, PluginKind, PluginVersion,
    Startup, VersionRange, compatible, read_plugin_frame, write_plugin_frame,
};
pub use scalars::{Counter, Id, SignedNanos};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
}

impl Version {
    pub const CURRENT: Self = Self { major: 1, minor: 0 };

    // 旧读取端会跳过未来可选字段；禁止用旧写入端冒充未来版本重写。
    fn check_writable(self) -> Result<(), ProtocolError> {
        if self == Self::CURRENT {
            Ok(())
        } else {
            Err(ProtocolError::unsupported_version(self))
        }
    }

    pub fn check(self) -> Result<(), ProtocolError> {
        if self.major == Self::CURRENT.major {
            Ok(())
        } else {
            Err(ProtocolError::unsupported_version(self))
        }
    }
}

// 先检验版本，再解释记录。未知主版本即使改变了记录形状，也会得到明确的版本错误。
fn read_versioned<T: DeserializeOwned>(input: &str) -> Result<T, ProtocolError> {
    #[derive(Deserialize)]
    struct Header {
        #[serde(deserialize_with = "object::required")]
        version: Version,
    }

    let object::Object(header): object::Object<Header> =
        serde_json::from_str(input).map_err(ProtocolError::from_json_error)?;
    header.version.check()?;
    serde_json::from_str(input).map_err(ProtocolError::from_json_error)
}

impl Event {
    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        read_versioned(input)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        self.version.check_writable()?;
        self.validate()?;
        serde_json::to_string(self).map_err(ProtocolError::from_json_error)
    }
}

// 原生构造、Serde 与公开 JSON 入口使用同一套边界校验。
impl Serialize for Event {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.version
            .check_writable()
            .map_err(serde::ser::Error::custom)?;
        self.validate().map_err(serde::ser::Error::custom)?;
        event::EventWire::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let event = event::read(deserializer)?;
        event.validate().map_err(serde::de::Error::custom)?;
        Ok(event)
    }
}
