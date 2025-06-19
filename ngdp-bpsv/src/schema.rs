//! BPSV schema definitions for field structure

use crate::error::{Error, Result};
use crate::field_type::BpsvFieldType;
use std::collections::HashMap;

/// Represents a single field in a BPSV schema
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BpsvField {
    /// Field name (case-sensitive as specified in header)
    pub name: String,
    /// Field type and length specification
    pub field_type: BpsvFieldType,
    /// Zero-based index in the schema
    pub index: usize,
}

impl BpsvField {
    /// Create a new field
    pub fn new(name: String, field_type: BpsvFieldType, index: usize) -> Self {
        Self {
            name,
            field_type,
            index,
        }
    }

    /// Validate a value for this field
    pub fn validate_value(&self, value: &str) -> Result<String> {
        self.field_type.validate_value(value).map_err(|mut err| {
            if let Error::InvalidValue { field, .. } = &mut err {
                *field = self.name.clone();
            }
            err
        })
    }
}

/// Represents the complete schema of a BPSV document
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BpsvSchema {
    /// All fields in order
    fields: Vec<BpsvField>,
    /// Map from field name to field index for fast lookup
    field_map: HashMap<String, usize>,
}

impl BpsvSchema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            field_map: HashMap::new(),
        }
    }

    /// Parse schema from a header line
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvSchema;
    ///
    /// let header = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4";
    /// let schema = BpsvSchema::parse_header(header)?;
    ///
    /// assert_eq!(schema.field_count(), 3);
    /// assert!(schema.has_field("Region"));
    /// assert!(schema.has_field("BuildConfig"));
    /// assert!(schema.has_field("BuildId"));
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse_header(header_line: &str) -> Result<Self> {
        let mut schema = Self::new();

        for field_spec in header_line.split('|') {
            let parts: Vec<&str> = field_spec.split('!').collect();
            if parts.len() != 2 {
                return Err(Error::InvalidHeader {
                    reason: format!("Invalid field specification: {}", field_spec),
                });
            }

            let field_name = parts[0].to_string();
            let type_spec = parts[1];

            // Check for duplicate field names
            if schema.field_map.contains_key(&field_name) {
                return Err(Error::DuplicateField { field: field_name });
            }

            let field_type = BpsvFieldType::parse(type_spec)?;
            schema.add_field(field_name, field_type)?;
        }

        if schema.fields.is_empty() {
            return Err(Error::InvalidHeader {
                reason: "No fields found in header".to_string(),
            });
        }

        Ok(schema)
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, name: String, field_type: BpsvFieldType) -> Result<()> {
        if self.field_map.contains_key(&name) {
            return Err(Error::DuplicateField { field: name });
        }

        let index = self.fields.len();
        let field = BpsvField::new(name.clone(), field_type, index);

        self.fields.push(field);
        self.field_map.insert(name, index);

        Ok(())
    }

    /// Get the number of fields
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Check if a field exists
    pub fn has_field(&self, name: &str) -> bool {
        self.field_map.contains_key(name)
    }

    /// Get a field by name
    pub fn get_field(&self, name: &str) -> Option<&BpsvField> {
        self.field_map.get(name).map(|&index| &self.fields[index])
    }

    /// Get a field by index
    pub fn get_field_by_index(&self, index: usize) -> Option<&BpsvField> {
        self.fields.get(index)
    }

    /// Get all fields
    pub fn fields(&self) -> &[BpsvField] {
        &self.fields
    }

    /// Get field names in order
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    /// Validate a row of values against this schema
    pub fn validate_row(&self, values: &[String]) -> Result<Vec<String>> {
        if values.len() != self.fields.len() {
            return Err(Error::SchemaMismatch {
                expected: self.fields.len(),
                actual: values.len(),
            });
        }

        let mut validated = Vec::new();
        for (field, value) in self.fields.iter().zip(values.iter()) {
            let normalized = field.validate_value(value)?;
            validated.push(normalized);
        }

        Ok(validated)
    }

    /// Generate the header line for this schema
    pub fn to_header_line(&self) -> String {
        self.fields
            .iter()
            .map(|field| format!("{}!{}", field.name, field.field_type))
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl Default for BpsvSchema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BpsvFieldType;

    #[test]
    fn test_parse_header() {
        let header = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4";
        let schema = BpsvSchema::parse_header(header).unwrap();

        assert_eq!(schema.field_count(), 3);
        assert!(schema.has_field("Region"));
        assert!(schema.has_field("BuildConfig"));
        assert!(schema.has_field("BuildId"));

        let region_field = schema.get_field("Region").unwrap();
        assert_eq!(region_field.field_type, BpsvFieldType::String(0));
        assert_eq!(region_field.index, 0);

        let build_field = schema.get_field("BuildConfig").unwrap();
        assert_eq!(build_field.field_type, BpsvFieldType::Hex(16));
        assert_eq!(build_field.index, 1);
    }

    #[test]
    fn test_parse_header_case_insensitive() {
        let header = "Region!string:0|BuildConfig!hex:16|BuildId!dec:4";
        let schema = BpsvSchema::parse_header(header).unwrap();

        assert_eq!(schema.field_count(), 3);

        let region_field = schema.get_field("Region").unwrap();
        assert_eq!(region_field.field_type, BpsvFieldType::String(0));
    }

    #[test]
    fn test_duplicate_field_error() {
        let header = "Region!STRING:0|Region!HEX:16";
        let result = BpsvSchema::parse_header(header);
        assert!(matches!(result, Err(Error::DuplicateField { .. })));
    }

    #[test]
    fn test_invalid_header_format() {
        let header = "Region|BuildConfig!HEX:16"; // Missing type for Region
        let result = BpsvSchema::parse_header(header);
        assert!(matches!(result, Err(Error::InvalidHeader { .. })));
    }

    #[test]
    fn test_validate_row() {
        let header = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4";
        let schema = BpsvSchema::parse_header(header).unwrap();

        let valid_row = vec![
            "us".to_string(),
            "abcd1234abcd1234".to_string(),
            "1234".to_string(),
        ];
        let result = schema.validate_row(&valid_row);
        assert!(result.is_ok());

        let invalid_row = vec![
            "us".to_string(),
            "invalid_hex".to_string(),
            "1234".to_string(),
        ];
        let result = schema.validate_row(&invalid_row);
        assert!(result.is_err());

        let wrong_length = vec!["us".to_string()]; // Too few fields
        let result = schema.validate_row(&wrong_length);
        assert!(matches!(result, Err(Error::SchemaMismatch { .. })));
    }

    #[test]
    fn test_to_header_line() {
        let mut schema = BpsvSchema::new();
        schema
            .add_field("Region".to_string(), BpsvFieldType::String(0))
            .unwrap();
        schema
            .add_field("BuildConfig".to_string(), BpsvFieldType::Hex(16))
            .unwrap();
        schema
            .add_field("BuildId".to_string(), BpsvFieldType::Decimal(4))
            .unwrap();

        let header_line = schema.to_header_line();
        assert_eq!(
            header_line,
            "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4"
        );
    }

    #[test]
    fn test_field_access() {
        let header = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4";
        let schema = BpsvSchema::parse_header(header).unwrap();

        assert_eq!(
            schema.field_names(),
            vec!["Region", "BuildConfig", "BuildId"]
        );

        assert_eq!(schema.get_field_by_index(0).unwrap().name, "Region");
        assert_eq!(schema.get_field_by_index(1).unwrap().name, "BuildConfig");
        assert_eq!(schema.get_field_by_index(2).unwrap().name, "BuildId");
        assert!(schema.get_field_by_index(3).is_none());
    }
}
