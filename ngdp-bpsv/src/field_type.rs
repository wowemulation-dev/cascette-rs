//! BPSV field type definitions and parsing

use crate::error::{Error, Result};
use std::fmt;

/// Represents a BPSV field type with its length specification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BpsvFieldType {
    /// String field with maximum length (0 = unlimited)
    String(u32),
    /// Hexadecimal field with byte count (N bytes = N*2 hex characters)
    Hex(u32),
    /// Decimal number field with storage size in bytes (e.g., 4 = uint32)
    Decimal(u32),
}

impl BpsvFieldType {
    /// Parse a field type from a string like "STRING:0", "HEX:16", "DEC:4"
    ///
    /// Parsing is case-insensitive for the type name.
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvFieldType;
    ///
    /// assert_eq!(BpsvFieldType::parse("STRING:0")?, BpsvFieldType::String(0));
    /// assert_eq!(BpsvFieldType::parse("string:0")?, BpsvFieldType::String(0));
    /// assert_eq!(BpsvFieldType::parse("String:0")?, BpsvFieldType::String(0));
    /// assert_eq!(BpsvFieldType::parse("HEX:16")?, BpsvFieldType::Hex(16));
    /// assert_eq!(BpsvFieldType::parse("hex:16")?, BpsvFieldType::Hex(16));
    /// assert_eq!(BpsvFieldType::parse("DEC:4")?, BpsvFieldType::Decimal(4));
    /// assert_eq!(BpsvFieldType::parse("dec:4")?, BpsvFieldType::Decimal(4));
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse(type_spec: &str) -> Result<Self> {
        let parts: Vec<&str> = type_spec.split(':').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidFieldType {
                field_type: type_spec.to_string(),
            });
        }

        let type_name = parts[0].to_uppercase();
        let length_str = parts[1];

        let length: u32 = length_str.parse().map_err(|_| Error::InvalidFieldType {
            field_type: type_spec.to_string(),
        })?;

        match type_name.as_str() {
            "STRING" => Ok(BpsvFieldType::String(length)),
            "HEX" => Ok(BpsvFieldType::Hex(length)),
            "DEC" | "DECIMAL" => Ok(BpsvFieldType::Decimal(length)),
            _ => Err(Error::InvalidFieldType {
                field_type: type_spec.to_string(),
            }),
        }
    }

    /// Get the type name (uppercase)
    pub fn type_name(&self) -> &'static str {
        match self {
            BpsvFieldType::String(_) => "STRING",
            BpsvFieldType::Hex(_) => "HEX",
            BpsvFieldType::Decimal(_) => "DEC",
        }
    }

    /// Get the length specification
    pub fn length(&self) -> u32 {
        match self {
            BpsvFieldType::String(len) => *len,
            BpsvFieldType::Hex(len) => *len,
            BpsvFieldType::Decimal(len) => *len,
        }
    }

    /// Check if a string value is valid for this field type
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvFieldType;
    ///
    /// let string_type = BpsvFieldType::String(5);
    /// assert!(string_type.is_valid_value("hello"));
    /// assert!(!string_type.is_valid_value("too_long")); // > 5 chars
    ///
    /// let hex_type = BpsvFieldType::Hex(4);  // 4 bytes = 8 hex chars
    /// assert!(hex_type.is_valid_value("abcd1234"));
    /// assert!(hex_type.is_valid_value("12345678"));
    /// assert!(!hex_type.is_valid_value("xyz")); // invalid hex
    /// assert!(!hex_type.is_valid_value("1234")); // wrong length (need 8 chars)
    ///
    /// let dec_type = BpsvFieldType::Decimal(4);
    /// assert!(dec_type.is_valid_value("1234"));
    /// assert!(dec_type.is_valid_value("0"));
    /// assert!(!dec_type.is_valid_value("abc")); // not a number
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn is_valid_value(&self, value: &str) -> bool {
        match self {
            BpsvFieldType::String(max_len) => *max_len == 0 || value.len() <= *max_len as usize,
            BpsvFieldType::Hex(byte_count) => {
                if value.is_empty() {
                    return true; // Empty values are always valid
                }
                // HEX:N means N bytes, which is N*2 hex characters
                if *byte_count > 0 && value.len() != (*byte_count as usize * 2) {
                    return false;
                }
                value.chars().all(|c| c.is_ascii_hexdigit())
            }
            BpsvFieldType::Decimal(_) => value.is_empty() || value.parse::<i64>().is_ok(),
        }
    }

    /// Validate and potentially normalize a value for this field type
    ///
    /// Returns the normalized value or an error if invalid.
    pub fn validate_value(&self, value: &str) -> Result<String> {
        if !self.is_valid_value(value) {
            return Err(Error::InvalidValue {
                field: "unknown".to_string(),
                field_type: self.to_string(),
                value: value.to_string(),
            });
        }

        match self {
            BpsvFieldType::String(_) => Ok(value.to_string()),
            BpsvFieldType::Hex(_) => {
                if value.is_empty() {
                    return Ok(value.to_string()); // Keep empty values as-is
                }
                Ok(value.to_lowercase()) // Normalize to lowercase
            }
            BpsvFieldType::Decimal(_) => {
                if value.is_empty() {
                    return Ok(value.to_string()); // Keep empty values as-is
                }
                // Normalize number (remove leading zeros, etc.)
                let num: i64 = value.parse().map_err(|_| Error::InvalidNumber {
                    value: value.to_string(),
                })?;
                Ok(num.to_string())
            }
        }
    }
}

impl fmt::Display for BpsvFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.type_name(), self.length())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_field_types() {
        assert_eq!(
            BpsvFieldType::parse("STRING:0").unwrap(),
            BpsvFieldType::String(0)
        );
        assert_eq!(
            BpsvFieldType::parse("string:0").unwrap(),
            BpsvFieldType::String(0)
        );
        assert_eq!(
            BpsvFieldType::parse("String:0").unwrap(),
            BpsvFieldType::String(0)
        );
        assert_eq!(
            BpsvFieldType::parse("HEX:16").unwrap(),
            BpsvFieldType::Hex(16)
        );
        assert_eq!(
            BpsvFieldType::parse("hex:16").unwrap(),
            BpsvFieldType::Hex(16)
        );
        assert_eq!(
            BpsvFieldType::parse("DEC:4").unwrap(),
            BpsvFieldType::Decimal(4)
        );
        assert_eq!(
            BpsvFieldType::parse("dec:4").unwrap(),
            BpsvFieldType::Decimal(4)
        );
        assert_eq!(
            BpsvFieldType::parse("DECIMAL:4").unwrap(),
            BpsvFieldType::Decimal(4)
        );
    }

    #[test]
    fn test_invalid_field_types() {
        assert!(BpsvFieldType::parse("INVALID:0").is_err());
        assert!(BpsvFieldType::parse("STRING").is_err());
        assert!(BpsvFieldType::parse("STRING:abc").is_err());
        assert!(BpsvFieldType::parse("").is_err());
    }

    #[test]
    fn test_value_validation() {
        let string_type = BpsvFieldType::String(5);
        assert!(string_type.is_valid_value("hello"));
        assert!(string_type.is_valid_value(""));
        assert!(!string_type.is_valid_value("toolong"));

        let string_unlimited = BpsvFieldType::String(0);
        assert!(string_unlimited.is_valid_value("any length string here"));

        let hex_type = BpsvFieldType::Hex(4); // 4 bytes = 8 hex chars
        assert!(hex_type.is_valid_value("abcd1234"));
        assert!(hex_type.is_valid_value("12345678"));
        assert!(hex_type.is_valid_value("ABCD1234"));
        assert!(!hex_type.is_valid_value("xyz12345")); // invalid hex
        assert!(!hex_type.is_valid_value("1234")); // too short (4 chars, need 8)
        assert!(!hex_type.is_valid_value("123456789")); // too long (9 chars)

        let hex_unlimited = BpsvFieldType::Hex(0);
        assert!(hex_unlimited.is_valid_value("abc123"));
        assert!(hex_unlimited.is_valid_value(""));
        assert!(!hex_unlimited.is_valid_value("xyz"));

        let dec_type = BpsvFieldType::Decimal(4);
        assert!(dec_type.is_valid_value("1234"));
        assert!(dec_type.is_valid_value("0"));
        assert!(dec_type.is_valid_value("-123"));
        assert!(!dec_type.is_valid_value("abc"));
        assert!(!dec_type.is_valid_value("12.34"));
    }

    #[test]
    fn test_normalize_values() {
        let hex_type = BpsvFieldType::Hex(4); // 4 bytes = 8 hex chars
        assert_eq!(hex_type.validate_value("ABCD1234").unwrap(), "abcd1234");

        let dec_type = BpsvFieldType::Decimal(4);
        assert_eq!(dec_type.validate_value("0123").unwrap(), "123");
        assert_eq!(dec_type.validate_value("-0042").unwrap(), "-42");
    }

    #[test]
    fn test_display() {
        assert_eq!(BpsvFieldType::String(0).to_string(), "STRING:0");
        assert_eq!(BpsvFieldType::Hex(16).to_string(), "HEX:16");
        assert_eq!(BpsvFieldType::Decimal(4).to_string(), "DEC:4");
    }
}
