use crate::bpsv::schema::BpsvSchema;
use crate::bpsv::types::{BpsvError, BpsvValue};
use std::collections::HashMap;

/// A single row of BPSV data
#[derive(Debug, Clone)]
pub struct BpsvRow {
    /// Parsed values in order
    values: Vec<BpsvValue>,
    /// Raw string values for serialization
    raw_values: Vec<String>,
}

impl BpsvRow {
    /// Create a new row from raw values and schema
    pub fn parse(raw_values: Vec<String>, schema: &BpsvSchema) -> Result<Self, BpsvError> {
        // Validate field count
        if raw_values.len() != schema.field_count() {
            return Err(BpsvError::FieldCountMismatch {
                expected: schema.field_count(),
                actual: raw_values.len(),
            });
        }

        // Parse values according to schema
        let mut values = Vec::new();
        for (i, raw) in raw_values.iter().enumerate() {
            let field = schema
                .get_field(i)
                .ok_or(BpsvError::ColumnIndexOutOfBounds(i))?;
            let value = BpsvValue::parse(raw, field.field_type)?;
            values.push(value);
        }

        Ok(Self { values, raw_values })
    }

    /// Create a row from pre-parsed values
    pub fn from_values(values: Vec<BpsvValue>) -> Self {
        let raw_values = values
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        Self { values, raw_values }
    }

    /// Get value by index
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&BpsvValue> {
        self.values.get(index)
    }

    /// Get raw string value by index
    #[must_use]
    pub fn get_raw(&self, index: usize) -> Option<&str> {
        self.raw_values.get(index).map(std::string::String::as_str)
    }

    /// Get value by field name (requires schema)
    #[must_use]
    pub fn get_by_name<'a>(&'a self, name: &str, schema: &BpsvSchema) -> Option<&'a BpsvValue> {
        schema
            .get_field_index(name)
            .and_then(|index| self.values.get(index))
    }

    /// Get raw string value by field name (requires schema)
    ///
    /// This is useful for HEX fields where you need the raw hex string
    /// rather than the parsed bytes.
    #[must_use]
    pub fn get_raw_by_name<'a>(&'a self, name: &str, schema: &BpsvSchema) -> Option<&'a str> {
        schema
            .get_field_index(name)
            .and_then(|index| self.raw_values.get(index).map(String::as_str))
    }

    /// Get all values
    #[must_use]
    pub fn values(&self) -> &[BpsvValue] {
        &self.values
    }

    /// Get all raw values
    #[must_use]
    pub fn raw_values(&self) -> &[String] {
        &self.raw_values
    }

    /// Convert row to map using schema field names
    #[must_use]
    pub fn to_map(&self, schema: &BpsvSchema) -> HashMap<String, BpsvValue> {
        let mut map = HashMap::new();
        for (i, field) in schema.fields().iter().enumerate() {
            if let Some(value) = self.values.get(i) {
                map.insert(field.name.clone(), value.clone());
            }
        }
        map
    }

    /// Format row as pipe-separated string
    #[must_use]
    pub fn to_line(&self) -> String {
        self.raw_values.join("|")
    }

    /// Get the number of values in this row
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if row is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bpsv::types::{BpsvField, BpsvType};

    fn create_test_schema() -> BpsvSchema {
        BpsvSchema::new(vec![
            BpsvField::new("Region", BpsvType::String(0)),
            BpsvField::new("BuildConfig", BpsvType::Hex(16)),
            BpsvField::new("BuildId", BpsvType::Dec(4)),
        ])
    }

    #[test]
    fn test_row_parse() {
        let schema = create_test_schema();
        let raw = vec!["us".to_string(), "abcd1234".to_string(), "5678".to_string()];

        let parsed_row = BpsvRow::parse(raw, &schema).expect("Test operation should succeed");
        assert_eq!(parsed_row.len(), 3);
        assert_eq!(parsed_row.get_raw(0), Some("us"));
        assert_eq!(parsed_row.get_raw(1), Some("abcd1234"));
        assert_eq!(parsed_row.get_raw(2), Some("5678"));

        // Check parsed values
        assert_eq!(
            parsed_row
                .get(0)
                .expect("Operation should succeed")
                .as_string(),
            Some("us")
        );
        assert_eq!(
            parsed_row
                .get(2)
                .expect("Operation should succeed")
                .as_dec(),
            Some(5678)
        );
    }

    #[test]
    fn test_row_from_values() {
        let values = vec![
            BpsvValue::String("test".to_string()),
            BpsvValue::Dec(42),
            BpsvValue::Empty,
        ];

        let row = BpsvRow::from_values(values.clone());
        assert_eq!(row.len(), 3);
        assert_eq!(row.get(0), Some(&values[0]));
        assert_eq!(row.get(1), Some(&values[1]));
        assert_eq!(row.get(2), Some(&values[2]));
    }

    #[test]
    fn test_row_get_by_name() {
        let schema = create_test_schema();
        let raw_data = vec!["eu".to_string(), "1234abcd".to_string(), "9999".to_string()];

        let parsed_row = BpsvRow::parse(raw_data, &schema).expect("Test operation should succeed");

        assert_eq!(
            parsed_row
                .get_by_name("Region", &schema)
                .expect("Operation should succeed")
                .as_string(),
            Some("eu")
        );
        assert_eq!(
            parsed_row
                .get_by_name("BuildId", &schema)
                .expect("Operation should succeed")
                .as_dec(),
            Some(9999)
        );
        assert!(parsed_row.get_by_name("NonExistent", &schema).is_none());
    }

    #[test]
    fn test_row_to_map() {
        let schema = create_test_schema();
        let raw_values = vec!["cn".to_string(), "deadbeef".to_string(), "1111".to_string()];

        let parsed_row =
            BpsvRow::parse(raw_values, &schema).expect("Test operation should succeed");
        let map = parsed_row.to_map(&schema);

        assert_eq!(
            map.get("Region")
                .expect("Operation should succeed")
                .as_string(),
            Some("cn")
        );
        assert_eq!(
            map.get("BuildId")
                .expect("Operation should succeed")
                .as_dec(),
            Some(1111)
        );
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn test_row_to_line() {
        let values = vec![
            BpsvValue::String("a".to_string()),
            BpsvValue::Empty,
            BpsvValue::String("c".to_string()),
        ];

        let row = BpsvRow::from_values(values);
        assert_eq!(row.to_line(), "a||c");
    }

    #[test]
    fn test_row_field_count_mismatch() {
        let schema = create_test_schema();
        let raw = vec!["us".to_string(), "abcd".to_string()]; // Missing one field

        let result = BpsvRow::parse(raw, &schema);
        assert!(matches!(
            result,
            Err(BpsvError::FieldCountMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }
}
