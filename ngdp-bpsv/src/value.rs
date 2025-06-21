//! BPSV value types with type-safe conversions

use crate::error::{Error, Result};
use crate::field_type::BpsvFieldType;
use std::fmt;

/// Represents a typed value in a BPSV document
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BpsvValue {
    /// String value
    String(String),
    /// Hexadecimal value (stored as lowercase string)
    Hex(String),
    /// Decimal integer value
    Decimal(i64),
    /// Empty/null value
    Empty,
}

impl BpsvValue {
    /// Parse a string value according to the specified field type
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::{BpsvValue, BpsvFieldType};
    ///
    /// let string_val = BpsvValue::parse("hello", &BpsvFieldType::String(0))?;
    /// assert_eq!(string_val, BpsvValue::String("hello".to_string()));
    ///
    /// let hex_val = BpsvValue::parse("ABCD1234", &BpsvFieldType::Hex(4))?;
    /// assert_eq!(hex_val, BpsvValue::Hex("abcd1234".to_string()));
    ///
    /// let dec_val = BpsvValue::parse("1234", &BpsvFieldType::Decimal(4))?;
    /// assert_eq!(dec_val, BpsvValue::Decimal(1234));
    ///
    /// let empty_val = BpsvValue::parse("", &BpsvFieldType::String(0))?;
    /// assert_eq!(empty_val, BpsvValue::Empty);
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse(value: &str, field_type: &BpsvFieldType) -> Result<Self> {
        if value.is_empty() {
            return Ok(Self::Empty);
        }

        match field_type {
            BpsvFieldType::String(_) => {
                if !field_type.is_valid_value(value) {
                    return Err(Error::InvalidValue {
                        field: "unknown".to_string(),
                        field_type: field_type.to_string(),
                        value: value.to_string(),
                    });
                }
                Ok(Self::String(value.to_string()))
            }
            BpsvFieldType::Hex(_) => {
                // This should not happen due to early return, but be defensive
                if value.is_empty() {
                    return Ok(Self::Empty);
                }
                if !field_type.is_valid_value(value) {
                    return Err(Error::InvalidHex {
                        value: value.to_string(),
                    });
                }
                Ok(Self::Hex(value.to_lowercase()))
            }
            BpsvFieldType::Decimal(_) => {
                // This should not happen due to early return, but be defensive
                if value.is_empty() {
                    return Ok(Self::Empty);
                }
                let num = value.parse::<i64>().map_err(|_| Error::InvalidNumber {
                    value: value.to_string(),
                })?;
                Ok(Self::Decimal(num))
            }
        }
    }

    /// Convert the value to its string representation for BPSV output
    pub fn to_bpsv_string(&self) -> String {
        match self {
            Self::String(s) => s.clone(),
            Self::Hex(h) => h.clone(),
            Self::Decimal(d) => d.to_string(),
            Self::Empty => String::new(),
        }
    }

    /// Check if this value is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Get the value as a string, if it is a string type
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get the value as a hex string, if it is a hex type
    pub fn as_hex(&self) -> Option<&str> {
        match self {
            Self::Hex(h) => Some(h),
            _ => None,
        }
    }

    /// Get the value as a decimal number, if it is a decimal type
    pub fn as_decimal(&self) -> Option<i64> {
        match self {
            Self::Decimal(d) => Some(*d),
            _ => None,
        }
    }

    /// Convert to string value, consuming self
    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to hex value, consuming self
    pub fn into_hex(self) -> Option<String> {
        match self {
            Self::Hex(h) => Some(h),
            _ => None,
        }
    }

    /// Convert to decimal value, consuming self
    pub fn into_decimal(self) -> Option<i64> {
        match self {
            Self::Decimal(d) => Some(d),
            _ => None,
        }
    }

    /// Get the type of this value
    pub fn value_type(&self) -> &'static str {
        match self {
            Self::String(_) => "String",
            Self::Hex(_) => "Hex",
            Self::Decimal(_) => "Decimal",
            Self::Empty => "Empty",
        }
    }

    /// Check if this value is compatible with the given field type
    pub fn is_compatible_with(&self, field_type: &BpsvFieldType) -> bool {
        match (self, field_type) {
            (Self::String(_), BpsvFieldType::String(_)) => true,
            (Self::Hex(_), BpsvFieldType::Hex(_)) => true,
            (Self::Decimal(_), BpsvFieldType::Decimal(_)) => true,
            (Self::Empty, _) => true, // Empty is compatible with any type
            _ => false,
        }
    }
}

impl fmt::Display for BpsvValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bpsv_string())
    }
}

impl From<String> for BpsvValue {
    fn from(s: String) -> Self {
        if s.is_empty() {
            Self::Empty
        } else {
            Self::String(s)
        }
    }
}

impl From<&str> for BpsvValue {
    fn from(s: &str) -> Self {
        if s.is_empty() {
            Self::Empty
        } else {
            Self::String(s.to_string())
        }
    }
}

impl From<i64> for BpsvValue {
    fn from(i: i64) -> Self {
        Self::Decimal(i)
    }
}

impl From<i32> for BpsvValue {
    fn from(i: i32) -> Self {
        Self::Decimal(i64::from(i))
    }
}

impl From<u32> for BpsvValue {
    fn from(i: u32) -> Self {
        Self::Decimal(i64::from(i))
    }
}

impl From<u64> for BpsvValue {
    fn from(i: u64) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Self::Decimal(i as i64)
    }
}

impl TryFrom<BpsvValue> for String {
    type Error = Error;

    fn try_from(value: BpsvValue) -> Result<Self> {
        match value {
            BpsvValue::String(s) => Ok(s),
            BpsvValue::Empty => Ok(String::new()),
            _ => Err(Error::InvalidValue {
                field: "unknown".to_string(),
                field_type: "String".to_string(),
                value: value.to_bpsv_string(),
            }),
        }
    }
}

impl TryFrom<BpsvValue> for i64 {
    type Error = Error;

    fn try_from(value: BpsvValue) -> Result<Self> {
        match value {
            BpsvValue::Decimal(d) => Ok(d),
            _ => Err(Error::InvalidValue {
                field: "unknown".to_string(),
                field_type: "Decimal".to_string(),
                value: value.to_bpsv_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_values() {
        let string_type = BpsvFieldType::String(0);
        assert_eq!(
            BpsvValue::parse("hello", &string_type).unwrap(),
            BpsvValue::String("hello".to_string())
        );

        let hex_type = BpsvFieldType::Hex(4); // 4 bytes = 8 hex chars
        assert_eq!(
            BpsvValue::parse("ABCD1234", &hex_type).unwrap(),
            BpsvValue::Hex("abcd1234".to_string())
        );

        let dec_type = BpsvFieldType::Decimal(4);
        assert_eq!(
            BpsvValue::parse("1234", &dec_type).unwrap(),
            BpsvValue::Decimal(1234)
        );

        assert_eq!(
            BpsvValue::parse("", &string_type).unwrap(),
            BpsvValue::Empty
        );
    }

    #[test]
    fn test_invalid_values() {
        let hex_type = BpsvFieldType::Hex(4);
        assert!(BpsvValue::parse("xyz", &hex_type).is_err());

        let dec_type = BpsvFieldType::Decimal(4);
        assert!(BpsvValue::parse("abc", &dec_type).is_err());
    }

    #[test]
    fn test_conversions() {
        let string_val: BpsvValue = "hello".into();
        assert_eq!(string_val, BpsvValue::String("hello".to_string()));

        let num_val: BpsvValue = 1234i64.into();
        assert_eq!(num_val, BpsvValue::Decimal(1234));

        let empty_val: BpsvValue = "".into();
        assert_eq!(empty_val, BpsvValue::Empty);
    }

    #[test]
    fn test_accessors() {
        let string_val = BpsvValue::String("hello".to_string());
        assert_eq!(string_val.as_string(), Some("hello"));
        assert_eq!(string_val.as_hex(), None);
        assert_eq!(string_val.as_decimal(), None);

        let hex_val = BpsvValue::Hex("abcd".to_string());
        assert_eq!(hex_val.as_hex(), Some("abcd"));
        assert_eq!(hex_val.as_string(), None);

        let dec_val = BpsvValue::Decimal(1234);
        assert_eq!(dec_val.as_decimal(), Some(1234));
        assert_eq!(dec_val.as_string(), None);
    }

    #[test]
    fn test_compatibility() {
        let string_val = BpsvValue::String("hello".to_string());
        let string_type = BpsvFieldType::String(0);
        let hex_type = BpsvFieldType::Hex(4);

        assert!(string_val.is_compatible_with(&string_type));
        assert!(!string_val.is_compatible_with(&hex_type));

        let empty_val = BpsvValue::Empty;
        assert!(empty_val.is_compatible_with(&string_type));
        assert!(empty_val.is_compatible_with(&hex_type));
    }

    #[test]
    fn test_to_bpsv_string() {
        assert_eq!(
            BpsvValue::String("hello".to_string()).to_bpsv_string(),
            "hello"
        );
        assert_eq!(BpsvValue::Hex("abcd".to_string()).to_bpsv_string(), "abcd");
        assert_eq!(BpsvValue::Decimal(1234).to_bpsv_string(), "1234");
        assert_eq!(BpsvValue::Empty.to_bpsv_string(), "");
    }
}
