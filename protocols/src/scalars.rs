use std::fmt::Display;
use std::num::NonZeroU64;
use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer};

use crate::ProtocolError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Id(String);

impl Id {
    pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 256
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(ProtocolError::invalid(
                "ID must occupy 1 to 256 UTF-8 bytes without surrounding whitespace or control characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Id {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

// 先检查十进制前缀，再由目标整数类型验证剩余数字与溢出。
fn parse_decimal<T: FromStr>(value: &str, message: &str) -> Result<T, ProtocolError> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if value != "0" && !matches!(digits.as_bytes().first(), Some(b'1'..=b'9')) {
        return Err(ProtocolError::invalid(message));
    }
    value.parse().map_err(|_| ProtocolError::invalid(message))
}

fn serialize_decimal<T: Display, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Counter(#[serde(serialize_with = "serialize_decimal")] NonZeroU64);

impl Counter {
    pub fn new(value: u64) -> Result<Self, ProtocolError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or_else(|| ProtocolError::invalid("counter must be positive"))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<String> for Counter {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_decimal(&value, "expected a canonical positive u64 decimal string").map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct SignedNanos(#[serde(serialize_with = "serialize_decimal")] i64);

impl SignedNanos {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<String> for SignedNanos {
    type Error = ProtocolError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_decimal(&value, "expected a canonical signed i64 decimal string").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, from_value, json, to_value};

    #[test]
    fn ids_preserve_utf8_and_measure_the_limit_in_bytes() {
        let boundary = "é".repeat(128);
        for text in [boundary.as_str(), "资料 源", "e\u{301}", "é", "path:α/7"] {
            let id = Id::new(text).unwrap();
            assert_eq!(id.as_str(), text);
            assert_eq!(to_value(&id).unwrap(), json!(text));
            assert_eq!(from_value::<Id>(json!(text)).unwrap(), id);
        }
        assert_ne!(Id::new("e\u{301}").unwrap(), Id::new("é").unwrap());
        let too_long = boundary + "a";
        for text in ["", " ", "\u{2002}a", "a ", "a\n b", "a\u{85}b", &too_long] {
            assert!(Id::new(text).is_err());
            assert!(from_value::<Id>(json!(text)).is_err());
        }
        assert!(from_value::<Id>(json!(7)).is_err());
    }

    #[test]
    fn integers_round_trip_at_their_limits_and_beyond_javascript_precision() {
        assert!(Counter::new(0).is_err());
        for value in [1, (1 << 53) + 1, u64::MAX] {
            let wire = json!(value.to_string());
            assert_eq!(to_value(Counter::new(value).unwrap()).unwrap(), wire);
            assert_eq!(from_value::<Counter>(wire).unwrap().get(), value);
        }
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            let wire = json!(value.to_string());
            assert_eq!(to_value(SignedNanos::new(value)).unwrap(), wire);
            assert_eq!(from_value::<SignedNanos>(wire).unwrap().get(), value);
        }
    }

    #[test]
    fn integers_reject_noncanonical_or_out_of_range_wire_values() {
        for text in ["", "+1", "01", "-0", "-01", " 1", "1 ", "1.0", "1e1", "３"] {
            assert!(from_value::<Counter>(json!(text)).is_err(), "{text}");
            assert!(from_value::<SignedNanos>(json!(text)).is_err(), "{text}");
        }
        for literal in ["1", "1.0", "1e2", "null", "false", "[]"] {
            assert!(from_str::<Counter>(literal).is_err(), "{literal}");
            assert!(from_str::<SignedNanos>(literal).is_err(), "{literal}");
        }
        for text in ["0", "-1", "18446744073709551616"] {
            assert!(from_value::<Counter>(json!(text)).is_err());
        }
        for text in ["-9223372036854775809", "9223372036854775808"] {
            assert!(from_value::<SignedNanos>(json!(text)).is_err());
        }
    }
}
