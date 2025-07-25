//! BPSV document representation

use crate::error::{Error, Result};
use crate::schema::BpsvSchema;
use crate::value::BpsvValue;
use std::collections::HashMap;

/// A single row in a BPSV document with borrowed data
#[derive(Debug, Clone, PartialEq)]
pub struct BpsvRow<'a> {
    /// Raw string values as they appear in the BPSV (borrowed)
    raw_values: Vec<&'a str>,
    /// Typed values (lazy-loaded)
    typed_values: Option<Vec<BpsvValue>>,
}

impl<'a> BpsvRow<'a> {
    /// Create a new row from raw string slices
    pub fn new(values: Vec<&'a str>) -> Self {
        Self {
            raw_values: values,
            typed_values: None,
        }
    }

    /// Create a new row from typed values
    pub fn from_typed_values(values: Vec<BpsvValue>) -> BpsvRow<'static> {
        // For typed values, we need to allocate since we're creating new data
        BpsvRow {
            raw_values: vec![],
            typed_values: Some(values),
        }
    }

    /// Get the number of values in this row
    pub fn len(&self) -> usize {
        if let Some(typed) = &self.typed_values {
            typed.len()
        } else {
            self.raw_values.len()
        }
    }

    /// Check if the row is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a raw string value by index
    pub fn get_raw(&self, index: usize) -> Option<&str> {
        self.raw_values.get(index).copied()
    }

    /// Get a raw string value by field name using the schema
    pub fn get_raw_by_name(&self, field_name: &str, schema: &BpsvSchema) -> Option<&str> {
        schema
            .get_field(field_name)
            .and_then(|field| self.get_raw(field.index))
    }

    /// Get all raw values
    pub fn raw_values(&self) -> &[&'a str] {
        &self.raw_values
    }

    /// Parse and get typed values using the schema
    pub fn get_typed_values(&mut self, schema: &BpsvSchema) -> Result<&[BpsvValue]> {
        if self.typed_values.is_none() {
            if self.raw_values.len() != schema.field_count() {
                return Err(Error::SchemaMismatch {
                    expected: schema.field_count(),
                    actual: self.raw_values.len(),
                });
            }

            let mut typed = Vec::new();
            for (value, field) in self.raw_values.iter().zip(schema.fields()) {
                let typed_value = BpsvValue::parse(value, &field.field_type)?;
                typed.push(typed_value);
            }
            self.typed_values = Some(typed);
        }

        Ok(self.typed_values.as_ref().unwrap())
    }

    /// Get a typed value by index
    pub fn get_typed(&mut self, index: usize, schema: &BpsvSchema) -> Result<Option<&BpsvValue>> {
        let typed_values = self.get_typed_values(schema)?;
        Ok(typed_values.get(index))
    }

    /// Get a typed value by field name
    pub fn get_typed_by_name(
        &mut self,
        field_name: &str,
        schema: &BpsvSchema,
    ) -> Result<Option<&BpsvValue>> {
        if let Some(field) = schema.get_field(field_name) {
            self.get_typed(field.index, schema)
        } else {
            Err(Error::FieldNotFound {
                field: field_name.to_string(),
            })
        }
    }

    /// Convert row to a map of field names to raw values
    pub fn to_map(&self, schema: &BpsvSchema) -> Result<HashMap<String, String>> {
        if self.raw_values.len() != schema.field_count() {
            return Err(Error::SchemaMismatch {
                expected: schema.field_count(),
                actual: self.raw_values.len(),
            });
        }

        let mut map = HashMap::new();
        for (field, value) in schema.fields().iter().zip(self.raw_values.iter()) {
            map.insert(field.name.clone(), value.to_string());
        }
        Ok(map)
    }

    /// Convert row to a map of field names to typed values
    pub fn to_typed_map(&mut self, schema: &BpsvSchema) -> Result<HashMap<String, BpsvValue>> {
        let typed_values = self.get_typed_values(schema)?;
        let mut map = HashMap::new();

        for (field, value) in schema.fields().iter().zip(typed_values.iter()) {
            map.insert(field.name.clone(), value.clone());
        }
        Ok(map)
    }

    /// Convert to BPSV line format
    pub fn to_bpsv_line(&self) -> String {
        if let Some(typed) = &self.typed_values {
            typed
                .iter()
                .map(|v| v.to_bpsv_string())
                .collect::<Vec<_>>()
                .join("|")
        } else {
            self.raw_values.join("|")
        }
    }
}

/// An owned version of BpsvRow for when we need to store data
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OwnedBpsvRow {
    /// Raw string values as they appear in the BPSV
    raw_values: Vec<String>,
    /// Typed values (lazy-loaded)
    typed_values: Option<Vec<BpsvValue>>,
}

impl OwnedBpsvRow {
    /// Create a new row from owned string values
    pub fn new(values: Vec<String>) -> Self {
        Self {
            raw_values: values,
            typed_values: None,
        }
    }

    /// Create from a borrowed row
    pub fn from_borrowed(row: &BpsvRow<'_>) -> Self {
        Self {
            raw_values: row.raw_values.iter().map(|&s| s.to_string()).collect(),
            typed_values: row.typed_values.clone(),
        }
    }

    /// Convert to borrowed row
    pub fn as_borrowed(&self) -> BpsvRow<'_> {
        BpsvRow {
            raw_values: self.raw_values.iter().map(|s| s.as_str()).collect(),
            typed_values: self.typed_values.clone(),
        }
    }
}

/// Represents a complete BPSV document with borrowed data
#[derive(Debug, Clone, PartialEq)]
pub struct BpsvDocument<'a> {
    /// The original content (for zero-copy)
    content: &'a str,
    /// The schema defining field structure
    schema: BpsvSchema,
    /// Sequence number (optional)
    sequence_number: Option<u32>,
    /// All data rows
    rows: Vec<BpsvRow<'a>>,
}

impl<'a> BpsvDocument<'a> {
    /// Create a new BPSV document
    pub fn new(content: &'a str, schema: BpsvSchema) -> Self {
        Self {
            content,
            schema,
            sequence_number: None,
            rows: Vec::new(),
        }
    }

    /// Parse a BPSV document from string content
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvDocument;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
    ///
    /// let doc = BpsvDocument::parse(content)?;
    /// assert_eq!(doc.sequence_number(), Some(12345));
    /// assert_eq!(doc.rows().len(), 2);
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse(content: &'a str) -> Result<Self> {
        crate::parser::BpsvParser::parse(content)
    }

    /// Get the schema
    pub fn schema(&self) -> &BpsvSchema {
        &self.schema
    }

    /// Get the sequence number
    pub fn sequence_number(&self) -> Option<u32> {
        self.sequence_number
    }

    /// Set the sequence number
    pub fn set_sequence_number(&mut self, seqn: Option<u32>) {
        self.sequence_number = seqn;
    }

    /// Get all rows
    pub fn rows(&self) -> &[BpsvRow<'a>] {
        &self.rows
    }

    /// Get a mutable reference to all rows
    pub fn rows_mut(&mut self) -> &mut [BpsvRow<'a>] {
        &mut self.rows
    }

    /// Get the number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Check if the document has no data rows
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Add a row from raw string slices
    pub fn add_row(&mut self, values: Vec<&'a str>) -> Result<()> {
        // Validate against schema
        let validated = self.schema.validate_row_refs(&values)?;
        self.rows.push(BpsvRow::new(validated));
        Ok(())
    }

    /// Add a row from typed values
    pub fn add_typed_row(&mut self, values: Vec<BpsvValue>) -> Result<()> {
        if values.len() != self.schema.field_count() {
            return Err(Error::SchemaMismatch {
                expected: self.schema.field_count(),
                actual: values.len(),
            });
        }

        // Validate compatibility
        for (value, field) in values.iter().zip(self.schema.fields()) {
            if !value.is_compatible_with(&field.field_type) {
                return Err(Error::InvalidValue {
                    field: field.name.clone(),
                    field_type: field.field_type.to_string(),
                    value: value.to_bpsv_string(),
                });
            }
        }

        self.rows.push(BpsvRow::from_typed_values(values));
        Ok(())
    }

    /// Get a row by index
    pub fn get_row(&self, index: usize) -> Option<&BpsvRow<'a>> {
        self.rows.get(index)
    }

    /// Get a mutable row by index
    pub fn get_row_mut(&mut self, index: usize) -> Option<&mut BpsvRow<'a>> {
        self.rows.get_mut(index)
    }

    /// Find rows where a field matches a specific value
    pub fn find_rows_by_field(&self, field_name: &str, value: &str) -> Result<Vec<usize>> {
        let field = self
            .schema
            .get_field(field_name)
            .ok_or_else(|| Error::FieldNotFound {
                field: field_name.to_string(),
            })?;

        let mut matching_indices = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            if let Some(row_value) = row.get_raw(field.index) {
                if row_value == value {
                    matching_indices.push(index);
                }
            }
        }

        Ok(matching_indices)
    }

    /// Convert the entire document back to BPSV format
    pub fn to_bpsv_string(&self) -> String {
        let mut lines = Vec::new();

        // Header line
        lines.push(self.schema.to_header_line());

        // Sequence number line
        if let Some(seqn) = self.sequence_number {
            lines.push(format!("## seqn = {seqn}"));
        }

        // Data rows
        for row in &self.rows {
            lines.push(row.to_bpsv_line());
        }

        lines.join("\n")
    }

    /// Get all values for a specific field
    pub fn get_column(&self, field_name: &str) -> Result<Vec<&str>> {
        let field = self
            .schema
            .get_field(field_name)
            .ok_or_else(|| Error::FieldNotFound {
                field: field_name.to_string(),
            })?;

        let mut values = Vec::new();
        for row in &self.rows {
            if let Some(value) = row.get_raw(field.index) {
                values.push(value);
            }
        }

        Ok(values)
    }

    /// Convert all rows to maps for easier access
    pub fn to_maps(&self) -> Result<Vec<HashMap<String, String>>> {
        let mut maps = Vec::new();
        for row in &self.rows {
            maps.push(row.to_map(&self.schema)?);
        }
        Ok(maps)
    }
}

/// An owned version of BpsvDocument for serialization
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OwnedBpsvDocument {
    /// The schema defining field structure
    schema: BpsvSchema,
    /// Sequence number (optional)
    sequence_number: Option<u32>,
    /// All data rows
    rows: Vec<OwnedBpsvRow>,
}

impl OwnedBpsvDocument {
    /// Create a new owned document
    pub fn new(schema: BpsvSchema) -> Self {
        Self {
            schema,
            sequence_number: None,
            rows: Vec::new(),
        }
    }

    /// Set the sequence number
    pub fn set_sequence_number(&mut self, seqn: Option<u32>) {
        self.sequence_number = seqn;
    }

    /// Add a row to the document
    pub fn add_row(&mut self, row: OwnedBpsvRow) {
        self.rows.push(row);
    }

    /// Get the schema
    pub fn schema(&self) -> &BpsvSchema {
        &self.schema
    }

    /// Get the sequence number
    pub fn sequence_number(&self) -> Option<u32> {
        self.sequence_number
    }

    /// Get the number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get all rows
    pub fn rows(&self) -> &[OwnedBpsvRow] {
        &self.rows
    }

    /// Create from a borrowed document
    pub fn from_borrowed(doc: &BpsvDocument<'_>) -> Self {
        Self {
            schema: doc.schema.clone(),
            sequence_number: doc.sequence_number,
            rows: doc.rows.iter().map(OwnedBpsvRow::from_borrowed).collect(),
        }
    }

    /// Convert to BPSV string
    pub fn to_bpsv_string(&self) -> String {
        let mut lines = Vec::new();

        // Header line
        lines.push(self.schema.to_header_line());

        // Sequence number line
        if let Some(seqn) = self.sequence_number {
            lines.push(format!("## seqn = {seqn}"));
        }

        // Data rows
        for row in &self.rows {
            lines.push(row.as_borrowed().to_bpsv_line());
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BpsvFieldType, BpsvSchema};

    fn create_test_schema() -> BpsvSchema {
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
        schema
    }

    #[test]
    fn test_row_operations() {
        let schema = create_test_schema();
        let mut row = BpsvRow::new(vec!["us", "abcd1234abcd1234abcd1234abcd1234", "1234"]);

        assert_eq!(row.len(), 3);
        assert_eq!(row.get_raw(0), Some("us"));
        assert_eq!(row.get_raw_by_name("Region", &schema), Some("us"));

        let typed_values = row.get_typed_values(&schema).unwrap();
        assert_eq!(typed_values.len(), 3);
        assert_eq!(typed_values[0], BpsvValue::String("us".to_string()));
        assert_eq!(
            typed_values[1],
            BpsvValue::Hex("abcd1234abcd1234abcd1234abcd1234".to_string())
        );
        assert_eq!(typed_values[2], BpsvValue::Decimal(1234));
    }

    #[test]
    fn test_document_creation() {
        let content = "";
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(content, schema);

        doc.set_sequence_number(Some(12345));
        assert_eq!(doc.sequence_number(), Some(12345));

        doc.add_row(vec!["us", "abcd1234abcd1234abcd1234abcd1234", "1234"])
            .unwrap();
        doc.add_row(vec!["eu", "1234abcd1234abcd1234abcd1234abcd", "5678"])
            .unwrap();

        assert_eq!(doc.row_count(), 2);
        assert!(!doc.is_empty());
    }

    #[test]
    fn test_find_rows() {
        let content = "";
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(content, schema);

        doc.add_row(vec!["us", "abcd1234abcd1234abcd1234abcd1234", "1234"])
            .unwrap();
        doc.add_row(vec!["eu", "1234abcd1234abcd1234abcd1234abcd", "5678"])
            .unwrap();
        doc.add_row(vec!["us", "deadbeefdeadbeefdeadbeefdeadbeef", "9999"])
            .unwrap();

        let us_rows = doc.find_rows_by_field("Region", "us").unwrap();
        assert_eq!(us_rows, vec![0, 2]);

        let eu_rows = doc.find_rows_by_field("Region", "eu").unwrap();
        assert_eq!(eu_rows, vec![1]);
    }

    #[test]
    fn test_column_access() {
        let content = "";
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(content, schema);

        doc.add_row(vec!["us", "abcd1234abcd1234abcd1234abcd1234", "1234"])
            .unwrap();
        doc.add_row(vec!["eu", "1234abcd1234abcd1234abcd1234abcd", "5678"])
            .unwrap();

        let regions = doc.get_column("Region").unwrap();
        assert_eq!(regions, vec!["us", "eu"]);

        let build_ids = doc.get_column("BuildId").unwrap();
        assert_eq!(build_ids, vec!["1234", "5678"]);
    }

    #[test]
    fn test_to_bpsv_string() {
        let content = "";
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(content, schema);
        doc.set_sequence_number(Some(12345));
        doc.add_row(vec!["us", "abcd1234abcd1234abcd1234abcd1234", "1234"])
            .unwrap();

        let bpsv_string = doc.to_bpsv_string();
        let lines: Vec<&str> = bpsv_string.lines().collect();

        assert_eq!(lines[0], "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4");
        assert_eq!(lines[1], "## seqn = 12345");
        assert_eq!(lines[2], "us|abcd1234abcd1234abcd1234abcd1234|1234");
    }

    #[test]
    fn test_schema_mismatch() {
        let content = "";
        let schema = create_test_schema();
        let mut doc = BpsvDocument::new(content, schema);

        // Too few values
        let result = doc.add_row(vec!["us"]);
        assert!(matches!(result, Err(Error::SchemaMismatch { .. })));

        // Too many values
        let result = doc.add_row(vec!["us", "hex", "123", "extra"]);
        assert!(matches!(result, Err(Error::SchemaMismatch { .. })));
    }
}
