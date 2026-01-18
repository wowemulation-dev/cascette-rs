use crate::bpsv::row::BpsvRow;
use crate::bpsv::schema::BpsvSchema;
use crate::bpsv::types::BpsvError;
use std::fmt;

/// A complete BPSV document with schema and data rows
#[derive(Debug, Clone)]
pub struct BpsvDocument {
    /// Document schema
    schema: BpsvSchema,
    /// Data rows
    rows: Vec<BpsvRow>,
    /// Optional sequence number from "## seqn = N" line
    sequence_number: Option<u32>,
}

impl BpsvDocument {
    /// Create a new empty document with schema
    #[must_use]
    pub fn new(schema: BpsvSchema) -> Self {
        Self {
            schema,
            rows: Vec::new(),
            sequence_number: None,
        }
    }

    /// Create a document with schema and rows
    #[must_use]
    pub fn with_rows(schema: BpsvSchema, rows: Vec<BpsvRow>) -> Self {
        Self {
            schema,
            rows,
            sequence_number: None,
        }
    }

    /// Add a row to the document
    pub fn add_row(&mut self, row: BpsvRow) -> Result<(), BpsvError> {
        if row.len() != self.schema.field_count() {
            return Err(BpsvError::FieldCountMismatch {
                expected: self.schema.field_count(),
                actual: row.len(),
            });
        }
        self.rows.push(row);
        Ok(())
    }

    /// Add a row from raw values
    pub fn add_raw_row(&mut self, values: Vec<String>) -> Result<(), BpsvError> {
        let row = BpsvRow::parse(values, &self.schema)?;
        self.rows.push(row);
        Ok(())
    }

    /// Get the document schema
    #[must_use]
    pub fn schema(&self) -> &BpsvSchema {
        &self.schema
    }

    /// Get all rows
    #[must_use]
    pub fn rows(&self) -> &[BpsvRow] {
        &self.rows
    }

    /// Get a specific row by index
    #[must_use]
    pub fn get_row(&self, index: usize) -> Option<&BpsvRow> {
        self.rows.get(index)
    }

    /// Get the number of rows
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Check if document has any rows
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get the sequence number if present
    #[must_use]
    pub fn sequence_number(&self) -> Option<u32> {
        self.sequence_number
    }

    /// Set the sequence number
    pub fn set_sequence_number(&mut self, seqn: u32) {
        self.sequence_number = Some(seqn);
    }

    /// Clear the sequence number
    pub fn clear_sequence_number(&mut self) {
        self.sequence_number = None;
    }

    /// Check if document has a field with given name
    #[must_use]
    pub fn has_field(&self, name: &str) -> bool {
        self.schema.has_field(name)
    }

    /// Create an iterator over rows
    pub fn iter(&self) -> impl Iterator<Item = &BpsvRow> {
        self.rows.iter()
    }

    /// Clear all rows from the document
    pub fn clear(&mut self) {
        self.rows.clear();
    }

    /// Get field names from schema
    #[must_use]
    pub fn field_names(&self) -> Vec<&str> {
        self.schema.field_names()
    }
}

impl fmt::Display for BpsvDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write header
        writeln!(f, "{}", self.schema.to_header())?;

        // Write sequence number if present
        if let Some(seqn) = self.sequence_number {
            writeln!(f, "## seqn = {seqn}")?;
        }

        // Write data rows
        for row in &self.rows {
            writeln!(f, "{}", row.to_line())?;
        }

        Ok(())
    }
}

impl crate::CascFormat for BpsvDocument {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let input = std::str::from_utf8(data)?;
        crate::bpsv::parse(input).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let formatted = crate::bpsv::format(self);
        Ok(formatted.into_bytes())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bpsv::types::{BpsvField, BpsvType, BpsvValue};

    fn create_test_schema() -> BpsvSchema {
        BpsvSchema::new(vec![
            BpsvField::new("Region", BpsvType::String(0)),
            BpsvField::new("BuildId", BpsvType::Dec(4)),
        ])
    }

    #[test]
    fn test_document_new() {
        let schema = create_test_schema();
        let doc = BpsvDocument::new(schema);

        assert_eq!(doc.row_count(), 0);
        assert!(doc.is_empty());
        assert_eq!(doc.sequence_number(), None);
        assert_eq!(doc.schema().field_count(), 2);
    }

    #[test]
    fn test_document_add_row() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema.clone());

        let row = BpsvRow::parse(vec!["us".to_string(), "1234".to_string()], &schema)
            .expect("Test operation should succeed");

        doc.add_row(row).expect("Test operation should succeed");
        assert_eq!(doc.row_count(), 1);
        assert!(!doc.is_empty());
    }

    #[test]
    fn test_document_add_raw_row() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        doc.add_raw_row(vec!["eu".to_string(), "5678".to_string()])
            .expect("Test operation should succeed");
        doc.add_raw_row(vec!["cn".to_string(), "9999".to_string()])
            .expect("Test operation should succeed");

        assert_eq!(doc.row_count(), 2);
        assert_eq!(
            doc.get_row(0).expect("Operation should succeed").get_raw(0),
            Some("eu")
        );
        assert_eq!(
            doc.get_row(1).expect("Operation should succeed").get_raw(0),
            Some("cn")
        );
    }

    #[test]
    fn test_document_sequence_number() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        assert_eq!(doc.sequence_number(), None);

        doc.set_sequence_number(12345);
        assert_eq!(doc.sequence_number(), Some(12345));

        doc.clear_sequence_number();
        assert_eq!(doc.sequence_number(), None);
    }

    #[test]
    fn test_document_to_string() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        doc.set_sequence_number(99999);
        doc.add_raw_row(vec!["us".to_string(), "1111".to_string()])
            .expect("Test operation should succeed");
        doc.add_raw_row(vec!["eu".to_string(), "2222".to_string()])
            .expect("Test operation should succeed");

        let output = doc.to_string();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines[0], "Region!STRING:0|BuildId!DEC:4");
        assert_eq!(lines[1], "## seqn = 99999");
        assert_eq!(lines[2], "us|1111");
        assert_eq!(lines[3], "eu|2222");
    }

    #[test]
    fn test_document_field_count_mismatch() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        // Try to add row with wrong number of fields
        let wrong_row = BpsvRow::from_values(vec![BpsvValue::String("us".to_string())]);

        let result = doc.add_row(wrong_row);
        assert!(matches!(
            result,
            Err(BpsvError::FieldCountMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn test_document_iteration() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        doc.add_raw_row(vec!["us".to_string(), "1".to_string()])
            .expect("Test operation should succeed");
        doc.add_raw_row(vec!["eu".to_string(), "2".to_string()])
            .expect("Test operation should succeed");
        doc.add_raw_row(vec!["cn".to_string(), "3".to_string()])
            .expect("Test operation should succeed");

        let regions: Vec<&str> = doc.iter().filter_map(|row| row.get_raw(0)).collect();

        assert_eq!(regions, vec!["us", "eu", "cn"]);
    }

    #[test]
    fn test_document_clear() {
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(schema);

        doc.add_raw_row(vec!["us".to_string(), "1234".to_string()])
            .expect("Test operation should succeed");
        assert_eq!(doc.row_count(), 1);

        doc.clear();
        assert_eq!(doc.row_count(), 0);
        assert!(doc.is_empty());
    }

    #[test]
    fn test_document_field_access() {
        let schema = create_test_schema();
        let doc = BpsvDocument::new(schema);

        assert!(doc.has_field("Region"));
        assert!(doc.has_field("BuildId"));
        assert!(!doc.has_field("NonExistent"));

        assert_eq!(doc.field_names(), vec!["Region", "BuildId"]);
    }
}
