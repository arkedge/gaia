//! Field name transformation utilities
//!
//! This module provides centralized field name conversions between different formats:
//! - gRPC format: `SH_TI` or `SH_TI@RAW` (underscores, optional @RAW suffix)
//! - Database format: `SH.TI:conv` or `SH.TI:raw` (dots, :conv/:raw suffix)
//!
//! # Format Conversion Rules
//! 1. Replace underscores with dots: `SH_TI` → `SH.TI`
//! 2. Strip `@RAW` suffix if present
//! 3. Add `:raw` or `:conv` suffix based on presence of `@RAW`
//!
//! # Examples
//! ```
//! # use gaia_recorder::transform::FieldName;
//! // From gRPC format
//! let field = FieldName::from_grpc("SH_TI@RAW");
//! assert_eq!(field.to_db_format(), "SH.TI:raw");
//! assert!(field.is_raw());
//!
//! let field = FieldName::from_grpc("AB_CD");
//! assert_eq!(field.to_db_format(), "AB.CD:conv");
//! assert!(!field.is_raw());
//!
//! // From database format
//! let field = FieldName::from_db_format("SH.TI:raw");
//! assert_eq!(field.base_name(), "SH.TI");
//! assert!(field.is_raw());
//! ```

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldName {
    base_name: String,
    is_raw: bool,
}

impl FieldName {
    /// Parse field name from gRPC format
    ///
    /// # Format
    /// - Input: `FIELD_NAME` or `FIELD_NAME@RAW`
    /// - Underscores are converted to dots
    /// - `@RAW` suffix indicates raw telemetry (case-insensitive)
    ///
    /// # Examples
    /// ```
    /// # use gaia_recorder::transform::FieldName;
    /// let field = FieldName::from_grpc("SH_TI@RAW");
    /// assert_eq!(field.base_name(), "SH.TI");
    /// assert!(field.is_raw());
    ///
    /// let field = FieldName::from_grpc("OBC_MM_OPSMODE");
    /// assert_eq!(field.base_name(), "OBC.MM.OPSMODE");
    /// assert!(!field.is_raw());
    /// ```
    pub fn from_grpc(name: &str) -> Self {
        let is_raw = name.ends_with("@RAW") || name.ends_with("@raw");
        let base_name = name
            .trim_end_matches("@RAW")
            .trim_end_matches("@raw")
            .replace('_', ".");
        Self { base_name, is_raw }
    }

    /// Parse field name from database format
    ///
    /// # Format
    /// - Input: `FIELD.NAME:conv` or `FIELD.NAME:raw`
    /// - Dots separate field components
    /// - `:conv` or `:raw` suffix indicates telemetry type
    ///
    /// # Examples
    /// ```
    /// # use gaia_recorder::transform::FieldName;
    /// let field = FieldName::from_db_format("SH.TI:raw");
    /// assert_eq!(field.base_name(), "SH.TI");
    /// assert!(field.is_raw());
    ///
    /// let field = FieldName::from_db_format("ABC.DEF:conv");
    /// assert_eq!(field.base_name(), "ABC.DEF");
    /// assert!(!field.is_raw());
    /// ```
    pub fn from_db_format(name: &str) -> Self {
        let is_raw = name.ends_with(":raw");
        let base_name = name
            .trim_end_matches(":raw")
            .trim_end_matches(":conv")
            .to_string();
        Self { base_name, is_raw }
    }

    /// Convert to database storage format
    ///
    /// Returns field name with `:raw` or `:conv` suffix
    ///
    /// # Examples
    /// ```
    /// # use gaia_recorder::transform::FieldName;
    /// let field = FieldName::from_grpc("SH_TI@RAW");
    /// assert_eq!(field.to_db_format(), "SH.TI:raw");
    ///
    /// let field = FieldName::from_grpc("SH_TI");
    /// assert_eq!(field.to_db_format(), "SH.TI:conv");
    /// ```
    pub fn to_db_format(&self) -> String {
        format!(
            "{}:{}",
            self.base_name,
            if self.is_raw { "raw" } else { "conv" }
        )
    }

    /// Get base field name (without suffix)
    pub fn base_name(&self) -> &str {
        &self.base_name
    }

    /// Check if this is raw telemetry
    pub fn is_raw(&self) -> bool {
        self.is_raw
    }

    /// Get is_raw flag as database integer (0 or 1)
    pub fn is_raw_int(&self) -> i32 {
        if self.is_raw {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_grpc_conv() {
        let field = FieldName::from_grpc("SH_TI");
        assert_eq!(field.base_name(), "SH.TI");
        assert!(!field.is_raw());
        assert_eq!(field.to_db_format(), "SH.TI:conv");
        assert_eq!(field.is_raw_int(), 0);
    }

    #[test]
    fn test_from_grpc_raw() {
        let field = FieldName::from_grpc("SH_TI@RAW");
        assert_eq!(field.base_name(), "SH.TI");
        assert!(field.is_raw());
        assert_eq!(field.to_db_format(), "SH.TI:raw");
        assert_eq!(field.is_raw_int(), 1);
    }

    #[test]
    fn test_from_grpc_raw_lowercase() {
        let field = FieldName::from_grpc("SH_TI@raw");
        assert_eq!(field.base_name(), "SH.TI");
        assert!(field.is_raw());
        assert_eq!(field.to_db_format(), "SH.TI:raw");
    }

    #[test]
    fn test_from_grpc_multiple_underscores() {
        let field = FieldName::from_grpc("OBC_MM_OPSMODE@RAW");
        assert_eq!(field.base_name(), "OBC.MM.OPSMODE");
        assert!(field.is_raw());
        assert_eq!(field.to_db_format(), "OBC.MM.OPSMODE:raw");
    }

    #[test]
    fn test_from_db_format_conv() {
        let field = FieldName::from_db_format("SH.TI:conv");
        assert_eq!(field.base_name(), "SH.TI");
        assert!(!field.is_raw());
    }

    #[test]
    fn test_from_db_format_raw() {
        let field = FieldName::from_db_format("SH.TI:raw");
        assert_eq!(field.base_name(), "SH.TI");
        assert!(field.is_raw());
    }

    #[test]
    fn test_roundtrip_grpc_to_db() {
        let test_cases = vec![
            "SH_TI",
            "SH_TI@RAW",
            "OBC_MM_OPSMODE",
            "OBC_MM_OPSMODE@RAW",
            "SIMPLE",
            "SIMPLE@RAW",
        ];

        for input in test_cases {
            let field = FieldName::from_grpc(input);
            let db_format = field.to_db_format();
            let parsed = FieldName::from_db_format(&db_format);
            assert_eq!(field, parsed, "Roundtrip failed for input: {}", input);
        }
    }

    #[test]
    fn test_no_underscores() {
        let field = FieldName::from_grpc("SIMPLE");
        assert_eq!(field.base_name(), "SIMPLE");
        assert!(!field.is_raw());
        assert_eq!(field.to_db_format(), "SIMPLE:conv");
    }

    #[test]
    fn test_trailing_underscore() {
        let field = FieldName::from_grpc("FIELD_");
        assert_eq!(field.base_name(), "FIELD.");
        assert!(!field.is_raw());
    }
}
