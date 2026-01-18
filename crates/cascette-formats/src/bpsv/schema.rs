use crate::bpsv::types::{BpsvError, BpsvField};
use std::collections::HashMap;

/// BPSV document schema defining field structure
#[derive(Debug, Clone)]
pub struct BpsvSchema {
    /// Ordered list of fields
    fields: Vec<BpsvField>,
    /// Field name to index mapping for fast lookup
    field_map: HashMap<String, usize>,
}

impl BpsvSchema {
    /// Create a new schema from fields
    #[must_use]
    pub fn new(fields: Vec<BpsvField>) -> Self {
        let mut field_map = HashMap::new();
        for (index, field) in fields.iter().enumerate() {
            field_map.insert(field.name.clone(), index);
        }

        Self { fields, field_map }
    }

    /// Parse schema from header line
    pub fn parse(header: &str) -> Result<Self, BpsvError> {
        if header.is_empty() {
            return Err(BpsvError::EmptyDocument);
        }

        let field_specs: Vec<&str> = header.split('|').collect();
        if field_specs.is_empty() {
            return Err(BpsvError::InvalidHeader(
                "No fields found in header".to_string(),
            ));
        }

        let mut fields = Vec::new();
        for spec in field_specs {
            let field = BpsvField::parse(spec)?;
            fields.push(field);
        }

        Ok(Self::new(fields))
    }

    /// Get the number of fields
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Get field by index
    #[must_use]
    pub fn get_field(&self, index: usize) -> Option<&BpsvField> {
        self.fields.get(index)
    }

    /// Get field by name
    #[must_use]
    pub fn get_field_by_name(&self, name: &str) -> Option<&BpsvField> {
        self.field_map
            .get(name)
            .and_then(|&index| self.fields.get(index))
    }

    /// Get field index by name
    #[must_use]
    pub fn get_field_index(&self, name: &str) -> Option<usize> {
        self.field_map.get(name).copied()
    }

    /// Check if schema has a field with given name
    #[must_use]
    pub fn has_field(&self, name: &str) -> bool {
        self.field_map.contains_key(name)
    }

    /// Get all fields
    #[must_use]
    pub fn fields(&self) -> &[BpsvField] {
        &self.fields
    }

    /// Get field names in order
    #[must_use]
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    /// Format schema as header line
    #[must_use]
    pub fn to_header(&self) -> String {
        self.fields
            .iter()
            .map(super::types::BpsvField::to_spec)
            .collect::<Vec<_>>()
            .join("|")
    }

    /// Validate that a row has the correct number of values
    pub fn validate_row(&self, values: &[&str]) -> Result<(), BpsvError> {
        if values.len() != self.fields.len() {
            return Err(BpsvError::FieldCountMismatch {
                expected: self.fields.len(),
                actual: values.len(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bpsv::types::BpsvType;

    #[test]
    fn test_schema_parse() {
        let header = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4";
        let schema = BpsvSchema::parse(header).expect("Test operation should succeed");

        assert_eq!(schema.field_count(), 3);
        assert!(schema.has_field("Region"));
        assert!(schema.has_field("BuildConfig"));
        assert!(schema.has_field("BuildId"));

        assert_eq!(schema.get_field_index("Region"), Some(0));
        assert_eq!(schema.get_field_index("BuildConfig"), Some(1));
        assert_eq!(schema.get_field_index("BuildId"), Some(2));
    }

    #[test]
    fn test_schema_to_header() {
        let fields = vec![
            BpsvField::new("Region", BpsvType::String(0)),
            BpsvField::new("BuildId", BpsvType::Dec(4)),
        ];
        let schema = BpsvSchema::new(fields);

        assert_eq!(schema.to_header(), "Region!STRING:0|BuildId!DEC:4");
    }

    #[test]
    fn test_schema_field_access() {
        let fields = vec![
            BpsvField::new("Field1", BpsvType::String(0)),
            BpsvField::new("Field2", BpsvType::Hex(8)),
        ];
        let schema = BpsvSchema::new(fields.clone());

        assert_eq!(schema.get_field(0), Some(&fields[0]));
        assert_eq!(schema.get_field(1), Some(&fields[1]));
        assert_eq!(schema.get_field(2), None);

        assert_eq!(schema.get_field_by_name("Field1"), Some(&fields[0]));
        assert_eq!(schema.get_field_by_name("Field2"), Some(&fields[1]));
        assert_eq!(schema.get_field_by_name("Field3"), None);
    }

    #[test]
    fn test_schema_validate_row() {
        let schema = BpsvSchema::parse("A!STRING:0|B!STRING:0|C!STRING:0")
            .expect("Test operation should succeed");

        // Valid row
        assert!(schema.validate_row(&["a", "b", "c"]).is_ok());

        // Too few values
        assert!(matches!(
            schema.validate_row(&["a", "b"]),
            Err(BpsvError::FieldCountMismatch {
                expected: 3,
                actual: 2
            })
        ));

        // Too many values
        assert!(matches!(
            schema.validate_row(&["a", "b", "c", "d"]),
            Err(BpsvError::FieldCountMismatch {
                expected: 3,
                actual: 4
            })
        ));
    }

    #[test]
    fn test_field_names() {
        let schema = BpsvSchema::parse("Region!STRING:0|BuildId!DEC:4")
            .expect("Test operation should succeed");
        assert_eq!(schema.field_names(), vec!["Region", "BuildId"]);
    }
}
