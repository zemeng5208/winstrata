use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use crate::{Id, Version, object, read_versioned};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeError {
    pub domain: Id,
    pub code: Id,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "object::Object<WireError>")]
pub struct ProtocolError {
    #[serde(serialize_with = "write_version")]
    pub version: Version,
    pub code: Id,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native: Option<NativeError>,
}

#[derive(Deserialize)]
struct WireError {
    #[serde(deserialize_with = "object::required")]
    version: Version,
    code: Id,
    detail: String,
    #[serde(default, deserialize_with = "object::optional")]
    native: Option<NativeError>,
}

impl TryFrom<object::Object<WireError>> for ProtocolError {
    type Error = ProtocolError;

    fn try_from(object::Object(wire): object::Object<WireError>) -> Result<Self, Self::Error> {
        wire.version.check()?;
        Ok(Self {
            version: wire.version,
            code: wire.code,
            detail: wire.detail,
            native: wire.native,
        })
    }
}

fn write_version<S: Serializer>(version: &Version, serializer: S) -> Result<S::Ok, S::Error> {
    version
        .check_writable()
        .map_err(serde::ser::Error::custom)?;
    version.serialize(serializer)
}

impl ProtocolError {
    fn local(code: &'static str, detail: String) -> Self {
        Self {
            version: Version::CURRENT,
            code: Id::new(code).expect("built-in protocol error code must be a valid ID"),
            detail,
            native: None,
        }
    }

    pub(crate) fn invalid(reason: &str) -> Self {
        Self::local("invalid_record", reason.into())
    }

    pub(crate) fn unsupported_version(version: Version) -> Self {
        Self::local(
            "unsupported_version",
            format!(
                "Unsupported protocol version {}.{}",
                version.major, version.minor
            ),
        )
    }

    pub(crate) fn from_json_error(error: serde_json::Error) -> Self {
        use serde_json::error::Category;
        // 输入可能含用户数据，仅在诊断中返回位置和类别。
        match error.classify() {
            Category::Syntax | Category::Eof => Self::local(
                "invalid_json",
                format!(
                    "Invalid JSON at line {}, column {}",
                    error.line(),
                    error.column()
                ),
            ),
            Category::Data | Category::Io => Self::invalid("Record does not match the protocol"),
        }
    }

    pub fn from_json(input: &str) -> Result<Self, ProtocolError> {
        read_versioned(input)
    }

    pub fn to_json(&self) -> Result<String, ProtocolError> {
        self.version.check_writable()?;
        serde_json::to_string(self).map_err(Self::from_json_error)
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn errors_retain_native_evidence_and_future_codes() {
        let wire = json!({
            "version":{"major":1,"minor":4}, "code":"source.access_denied",
            "detail":"Cannot open this event channel", "extra":42,
            "native":{"domain":"win32","code":"5"}
        });
        let error = ProtocolError::from_json(&wire.to_string()).unwrap();
        assert_eq!(error.code.as_str(), "source.access_denied");
        assert_eq!(error.native.as_ref().unwrap().code.as_str(), "5");
        assert_eq!(
            error.to_json().unwrap_err().code.as_str(),
            "unsupported_version"
        );
        let mut local = error;
        local.version = Version::CURRENT;
        assert_eq!(
            ProtocolError::from_json(&local.to_json().unwrap()).unwrap(),
            local
        );
        let mut incompatible = local;
        incompatible.version.major = 2;
        assert_eq!(
            incompatible.to_json().unwrap_err().code.as_str(),
            "unsupported_version"
        );
        assert!(serde_json::to_value(&incompatible).is_err());
    }

    #[test]
    fn malformed_and_incompatible_messages_report_distinct_stable_codes() {
        for (wire, expected) in [
            ("{", "invalid_json"),
            (
                r#"{"version":{"major":2,"minor":0},"new_format":[]}"#,
                "unsupported_version",
            ),
            (
                r#"{"version":{"major":1,"minor":0},"version":{"major":2,"minor":0}}"#,
                "invalid_record",
            ),
            (
                r#"{"version":{"major":1,"minor":0},"code":false,"detail":"private"}"#,
                "invalid_record",
            ),
        ] {
            let result = ProtocolError::from_json(wire).unwrap_err();
            assert_eq!(result.code.as_str(), expected);
            assert!(!result.detail.contains("private"));
        }
    }
}
