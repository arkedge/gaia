//! Telemetry domain types

use serde::{Deserialize, Serialize};

/// Telemetry value type with backward-compatible parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Integer,
    Double,
    Enum,
    String,
    Bytes,
    Unknown,
}

impl ValueType {
    /// Convert to database string format (canonical form)
    pub fn to_db_string(&self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Double => "double",
            Self::Enum => "enum",
            Self::String => "string",
            Self::Bytes => "bytes",
            Self::Unknown => "unknown",
        }
    }

    /// Parse from database string format (supports legacy formats)
    ///
    /// Accepts both new canonical forms and legacy forms for backward compatibility:
    /// - "integer" or "int" → Integer
    /// - "double" or "num" → Double
    /// - "string" or "text" → String
    /// - "enum" → Enum
    /// - "bytes" → Bytes
    /// - anything else → Unknown
    pub fn from_db_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "integer" | "int" => Self::Integer,
            "double" | "num" => Self::Double,
            "string" | "text" => Self::String,
            "enum" => Self::Enum,
            "bytes" => Self::Bytes,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_db_string() {
        assert_eq!(ValueType::Integer.to_db_string(), "integer");
        assert_eq!(ValueType::Double.to_db_string(), "double");
        assert_eq!(ValueType::String.to_db_string(), "string");
        assert_eq!(ValueType::Enum.to_db_string(), "enum");
        assert_eq!(ValueType::Bytes.to_db_string(), "bytes");
        assert_eq!(ValueType::Unknown.to_db_string(), "unknown");
    }

    #[test]
    fn test_from_db_string_canonical() {
        assert_eq!(ValueType::from_db_string("integer"), ValueType::Integer);
        assert_eq!(ValueType::from_db_string("double"), ValueType::Double);
        assert_eq!(ValueType::from_db_string("string"), ValueType::String);
        assert_eq!(ValueType::from_db_string("enum"), ValueType::Enum);
        assert_eq!(ValueType::from_db_string("bytes"), ValueType::Bytes);
        assert_eq!(ValueType::from_db_string("unknown"), ValueType::Unknown);
    }

    #[test]
    fn test_from_db_string_legacy() {
        // Test legacy CSV import formats
        assert_eq!(ValueType::from_db_string("int"), ValueType::Integer);
        assert_eq!(ValueType::from_db_string("num"), ValueType::Double);
        assert_eq!(ValueType::from_db_string("text"), ValueType::String);
    }

    #[test]
    fn test_from_db_string_case_insensitive() {
        assert_eq!(ValueType::from_db_string("INTEGER"), ValueType::Integer);
        assert_eq!(ValueType::from_db_string("Double"), ValueType::Double);
        assert_eq!(ValueType::from_db_string("INT"), ValueType::Integer);
    }

    #[test]
    fn test_from_db_string_unknown() {
        assert_eq!(ValueType::from_db_string("invalid"), ValueType::Unknown);
        assert_eq!(ValueType::from_db_string(""), ValueType::Unknown);
    }

    #[test]
    fn test_roundtrip() {
        let types = vec![
            ValueType::Integer,
            ValueType::Double,
            ValueType::String,
            ValueType::Enum,
            ValueType::Bytes,
            ValueType::Unknown,
        ];

        for typ in types {
            let s = typ.to_db_string();
            assert_eq!(ValueType::from_db_string(s), typ);
        }
    }
}
