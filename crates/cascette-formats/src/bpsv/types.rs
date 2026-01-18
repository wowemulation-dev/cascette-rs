use std::fmt;
use thiserror::Error;

/// BPSV field type definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpsvType {
    /// String field with size hint
    String(usize),
    /// Hexadecimal field with byte count
    Hex(usize),
    /// Decimal number field with digit count hint
    Dec(usize),
}

impl BpsvType {
    /// Parse a type specification like "STRING:0" or "HEX:16"
    pub fn parse(spec: &str) -> Result<Self, BpsvError> {
        let parts: Vec<&str> = spec.split(':').collect();
        if parts.len() != 2 {
            return Err(BpsvError::InvalidTypeSpec(spec.to_string()));
        }

        let type_name = parts[0].to_uppercase();
        let size = parts[1]
            .parse::<usize>()
            .map_err(|_| BpsvError::InvalidTypeSpec(spec.to_string()))?;

        match type_name.as_str() {
            "STRING" => Ok(Self::String(size)),
            "HEX" => Ok(Self::Hex(size)),
            "DEC" => Ok(Self::Dec(size)),
            _ => Err(BpsvError::UnknownType(type_name)),
        }
    }

    /// Format type specification for output
    #[must_use]
    pub fn to_spec(&self) -> String {
        match self {
            Self::String(size) => format!("STRING:{size}"),
            Self::Hex(size) => format!("HEX:{size}"),
            Self::Dec(size) => format!("DEC:{size}"),
        }
    }

    /// Get the size hint for this type
    #[must_use]
    pub fn size_hint(&self) -> usize {
        match self {
            Self::String(size) | Self::Hex(size) | Self::Dec(size) => *size,
        }
    }
}

/// BPSV field definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BpsvField {
    /// Field name
    pub name: String,
    /// Field type with size hint
    pub field_type: BpsvType,
}

impl BpsvField {
    /// Create a new field
    pub fn new(name: impl Into<String>, field_type: BpsvType) -> Self {
        Self {
            name: name.into(),
            field_type,
        }
    }

    /// Parse a field specification like "BuildConfig!HEX:16"
    pub fn parse(spec: &str) -> Result<Self, BpsvError> {
        let parts: Vec<&str> = spec.split('!').collect();
        if parts.len() != 2 {
            return Err(BpsvError::InvalidFieldSpec(spec.to_string()));
        }

        let name = parts[0].to_string();
        let field_type = BpsvType::parse(parts[1])?;

        Ok(Self { name, field_type })
    }

    /// Format field specification for output
    #[must_use]
    pub fn to_spec(&self) -> String {
        format!("{}!{}", self.name, self.field_type.to_spec())
    }
}

/// BPSV value that can hold different types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BpsvValue {
    /// String value
    String(String),
    /// Hexadecimal bytes
    Hex(Vec<u8>),
    /// Decimal number
    Dec(i64),
    /// Empty field
    Empty,
}

impl BpsvValue {
    /// Parse a value according to its type
    pub fn parse(raw: &str, field_type: BpsvType) -> Result<Self, BpsvError> {
        if raw.is_empty() {
            return Ok(Self::Empty);
        }

        match field_type {
            BpsvType::String(_) => Ok(Self::String(raw.to_string())),
            BpsvType::Hex(_) => {
                // Validate hex string
                if raw.len() % 2 != 0 {
                    return Err(BpsvError::InvalidHexLength(raw.to_string()));
                }

                let bytes =
                    hex::decode(raw).map_err(|_| BpsvError::InvalidHexValue(raw.to_string()))?;
                Ok(Self::Hex(bytes))
            }
            BpsvType::Dec(_) => {
                let value = raw
                    .parse::<i64>()
                    .map_err(|_| BpsvError::InvalidDecValue(raw.to_string()))?;
                Ok(Self::Dec(value))
            }
        }
    }

    /// Get the raw string value if this is a string
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get the hex bytes if this is hex data
    #[must_use]
    pub fn as_hex(&self) -> Option<&[u8]> {
        match self {
            Self::Hex(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Get the decimal value if this is a number
    #[must_use]
    pub fn as_dec(&self) -> Option<i64> {
        match self {
            Self::Dec(n) => Some(*n),
            _ => None,
        }
    }

    /// Check if this value is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl fmt::Display for BpsvValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s}"),
            Self::Hex(bytes) => write!(f, "{}", hex::encode(bytes)),
            Self::Dec(n) => write!(f, "{n}"),
            Self::Empty => Ok(()),
        }
    }
}

/// BPSV error types
#[derive(Debug, Error)]
pub enum BpsvError {
    /// Invalid type specification format
    #[error("Invalid type specification: {0}")]
    InvalidTypeSpec(String),

    /// Unknown field type name
    #[error("Unknown type: {0}")]
    UnknownType(String),

    /// Invalid field specification format
    #[error("Invalid field specification: {0}")]
    InvalidFieldSpec(String),

    /// Invalid hexadecimal value
    #[error("Invalid hex value: {0}")]
    InvalidHexValue(String),

    /// Hexadecimal string has odd length
    #[error("Invalid hex length (must be even): {0}")]
    InvalidHexLength(String),

    /// Invalid decimal number value
    #[error("Invalid decimal value: {0}")]
    InvalidDecValue(String),

    /// Document is empty
    #[error("Empty document")]
    EmptyDocument,

    /// Document is missing header line
    #[error("Missing header")]
    MissingHeader,

    /// Header line is invalid
    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    /// Row has wrong number of fields
    #[error("Field count mismatch: expected {expected}, got {actual}")]
    FieldCountMismatch {
        /// Expected number of fields
        expected: usize,
        /// Actual number of fields
        actual: usize,
    },

    /// Invalid sequence number format
    #[error("Invalid sequence number: {0}")]
    InvalidSequenceNumber(String),

    /// Field name not found in schema
    #[error("Field not found: {0}")]
    FieldNotFound(String),

    /// Row index is out of bounds
    #[error("Row index out of bounds: {0}")]
    RowIndexOutOfBounds(usize),

    /// Column index is out of bounds
    #[error("Column index out of bounds: {0}")]
    ColumnIndexOutOfBounds(usize),
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_bpsv_type_parse() {
        assert_eq!(
            BpsvType::parse("STRING:0").expect("Operation should succeed"),
            BpsvType::String(0)
        );
        assert_eq!(
            BpsvType::parse("HEX:16").expect("Operation should succeed"),
            BpsvType::Hex(16)
        );
        assert_eq!(
            BpsvType::parse("DEC:4").expect("Operation should succeed"),
            BpsvType::Dec(4)
        );

        // Case insensitive
        assert_eq!(
            BpsvType::parse("string:0").expect("Operation should succeed"),
            BpsvType::String(0)
        );
        assert_eq!(
            BpsvType::parse("hex:8").expect("Operation should succeed"),
            BpsvType::Hex(8)
        );
        assert_eq!(
            BpsvType::parse("dec:2").expect("Operation should succeed"),
            BpsvType::Dec(2)
        );
    }

    #[test]
    fn test_bpsv_field_parse() {
        let field = BpsvField::parse("Region!STRING:0").expect("Test operation should succeed");
        assert_eq!(field.name, "Region");
        assert_eq!(field.field_type, BpsvType::String(0));

        let field = BpsvField::parse("BuildConfig!HEX:16").expect("Test operation should succeed");
        assert_eq!(field.name, "BuildConfig");
        assert_eq!(field.field_type, BpsvType::Hex(16));
    }

    #[test]
    fn test_bpsv_value_parse() {
        // String value
        let val =
            BpsvValue::parse("us", BpsvType::String(0)).expect("Test operation should succeed");
        assert_eq!(val.as_string(), Some("us"));

        // Hex value
        let val =
            BpsvValue::parse("abcd1234", BpsvType::Hex(4)).expect("Test operation should succeed");
        assert_eq!(val.as_hex(), Some(&[0xab, 0xcd, 0x12, 0x34][..]));

        // Decimal value
        let val =
            BpsvValue::parse("12345", BpsvType::Dec(5)).expect("Test operation should succeed");
        assert_eq!(val.as_dec(), Some(12345));

        // Empty value
        let val = BpsvValue::parse("", BpsvType::String(0)).expect("Test operation should succeed");
        assert!(val.is_empty());
    }

    #[test]
    fn test_value_to_string() {
        let val = BpsvValue::String("test".to_string());
        assert_eq!(val.to_string(), "test");

        let val = BpsvValue::Hex(vec![0xab, 0xcd]);
        assert_eq!(val.to_string(), "abcd");

        let val = BpsvValue::Dec(42);
        assert_eq!(val.to_string(), "42");

        let val = BpsvValue::Empty;
        assert_eq!(val.to_string(), "");
    }
}
