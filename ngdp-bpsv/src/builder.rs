//! BPSV document builder for creating BPSV content programmatically

use crate::document::BpsvDocument;
use crate::error::{Error, Result};
use crate::field_type::BpsvFieldType;
use crate::schema::BpsvSchema;
use crate::value::BpsvValue;

/// Builder for creating BPSV documents
///
/// # Examples
///
/// ```
/// use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
///
/// let mut builder = BpsvBuilder::new();
///
/// // Define schema
/// builder.add_field("Region", BpsvFieldType::String(0))?;
/// builder.add_field("BuildConfig", BpsvFieldType::Hex(16))?;
/// builder.add_field("BuildId", BpsvFieldType::Decimal(4))?;
///
/// // Set sequence number
/// builder.set_sequence_number(12345);
///
/// // Add data rows
/// builder.add_row(vec![
///     BpsvValue::String("us".to_string()),
///     BpsvValue::Hex("abcd1234abcd1234".to_string()),
///     BpsvValue::Decimal(1234),
/// ])?;
///
/// builder.add_row(vec![
///     BpsvValue::String("eu".to_string()),
///     BpsvValue::Hex("1234abcd1234abcd".to_string()),
///     BpsvValue::Decimal(5678),
/// ])?;
///
/// let document = builder.build()?;
/// let bpsv_string = document.to_bpsv_string();
/// # Ok::<(), ngdp_bpsv::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct BpsvBuilder {
    /// Schema being built
    schema: BpsvSchema,
    /// Sequence number (optional)
    sequence_number: Option<u32>,
    /// Rows to add to the document
    rows: Vec<Vec<BpsvValue>>,
}

impl BpsvBuilder {
    /// Create a new BPSV builder
    pub fn new() -> Self {
        Self {
            schema: BpsvSchema::new(),
            sequence_number: None,
            rows: Vec::new(),
        }
    }

    /// Create a builder from an existing schema
    pub fn from_schema(schema: BpsvSchema) -> Self {
        Self {
            schema,
            sequence_number: None,
            rows: Vec::new(),
        }
    }

    /// Add a field to the schema
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::{BpsvBuilder, BpsvFieldType};
    ///
    /// let mut builder = BpsvBuilder::new();
    /// builder.add_field("Region", BpsvFieldType::String(0))?;
    /// builder.add_field("BuildId", BpsvFieldType::Decimal(4))?;
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn add_field(&mut self, name: &str, field_type: BpsvFieldType) -> Result<&mut Self> {
        self.schema.add_field(name.to_string(), field_type)?;
        Ok(self)
    }

    /// Set the sequence number
    pub fn set_sequence_number(&mut self, seqn: u32) -> &mut Self {
        self.sequence_number = Some(seqn);
        self
    }

    /// Clear the sequence number
    pub fn clear_sequence_number(&mut self) -> &mut Self {
        self.sequence_number = None;
        self
    }

    /// Add a row of typed values
    ///
    /// The number of values must match the number of fields in the schema.
    pub fn add_row(&mut self, values: Vec<BpsvValue>) -> Result<&mut Self> {
        if values.len() != self.schema.field_count() {
            return Err(Error::SchemaMismatch {
                expected: self.schema.field_count(),
                actual: values.len(),
            });
        }

        // Validate that values are compatible with field types
        for (value, field) in values.iter().zip(self.schema.fields()) {
            if !value.is_compatible_with(&field.field_type) {
                return Err(Error::InvalidValue {
                    field: field.name.clone(),
                    field_type: field.field_type.to_string(),
                    value: value.to_bpsv_string(),
                });
            }

            // Also validate the actual value content
            let value_str = value.to_bpsv_string();
            field
                .field_type
                .validate_value(&value_str)
                .map_err(|mut err| {
                    if let Error::InvalidValue {
                        field: err_field, ..
                    } = &mut err
                    {
                        *err_field = field.name.clone();
                    }
                    err
                })?;
        }

        self.rows.push(values);
        Ok(self)
    }

    /// Add a row from raw string values
    ///
    /// Values will be parsed according to the field types in the schema.
    pub fn add_raw_row(&mut self, values: Vec<String>) -> Result<&mut Self> {
        if values.len() != self.schema.field_count() {
            return Err(Error::SchemaMismatch {
                expected: self.schema.field_count(),
                actual: values.len(),
            });
        }

        let mut typed_values = Vec::new();
        for (value, field) in values.iter().zip(self.schema.fields()) {
            let typed_value = BpsvValue::parse(value, &field.field_type)?;
            typed_values.push(typed_value);
        }

        self.rows.push(typed_values);
        Ok(self)
    }

    /// Add a row from a vector of values that can be converted to BpsvValue
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue};
    ///
    /// let mut builder = BpsvBuilder::new();
    /// builder.add_field("Region", BpsvFieldType::String(0))?;
    /// builder.add_field("BuildId", BpsvFieldType::Decimal(4))?;
    ///
    /// // Use homogeneous types or convert manually
    /// builder.add_row(vec![
    ///     BpsvValue::String("us".to_string()),
    ///     BpsvValue::Decimal(1234),
    /// ])?;
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn add_values_row<T>(&mut self, values: Vec<T>) -> Result<&mut Self>
    where
        T: Into<BpsvValue>,
    {
        let typed_values: Vec<BpsvValue> = values.into_iter().map(|v| v.into()).collect();
        self.add_row(typed_values)
    }

    /// Get the current number of fields
    pub fn field_count(&self) -> usize {
        self.schema.field_count()
    }

    /// Get the current number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Check if any fields have been defined
    pub fn has_fields(&self) -> bool {
        self.schema.field_count() > 0
    }

    /// Check if any rows have been added
    pub fn has_rows(&self) -> bool {
        !self.rows.is_empty()
    }

    /// Get the current schema
    pub fn schema(&self) -> &BpsvSchema {
        &self.schema
    }

    /// Clear all rows but keep the schema
    pub fn clear_rows(&mut self) -> &mut Self {
        self.rows.clear();
        self
    }

    /// Reset the builder to empty state
    pub fn reset(&mut self) -> &mut Self {
        self.schema = BpsvSchema::new();
        self.sequence_number = None;
        self.rows.clear();
        self
    }

    /// Build the final BPSV document
    ///
    /// This consumes the builder and returns a BpsvDocument.
    pub fn build(self) -> Result<BpsvDocument> {
        if self.schema.field_count() == 0 {
            return Err(Error::InvalidHeader {
                reason: "No fields defined in schema".to_string(),
            });
        }

        let mut document = BpsvDocument::new(self.schema);
        document.set_sequence_number(self.sequence_number);

        for row in self.rows {
            document.add_typed_row(row)?;
        }

        Ok(document)
    }

    /// Build and return the BPSV string representation
    ///
    /// This is a convenience method that builds the document and converts it to a string.
    pub fn build_string(self) -> Result<String> {
        let document = self.build()?;
        Ok(document.to_bpsv_string())
    }

    /// Validate the current builder state
    ///
    /// Returns Ok(()) if the builder is in a valid state, Err otherwise.
    pub fn validate(&self) -> Result<()> {
        if self.schema.field_count() == 0 {
            return Err(Error::InvalidHeader {
                reason: "No fields defined".to_string(),
            });
        }

        // Validate all rows
        for (row_index, row) in self.rows.iter().enumerate() {
            if row.len() != self.schema.field_count() {
                return Err(Error::RowValidation {
                    row_index,
                    reason: format!(
                        "Expected {} fields, got {}",
                        self.schema.field_count(),
                        row.len()
                    ),
                });
            }

            for (value, field) in row.iter().zip(self.schema.fields()) {
                if !value.is_compatible_with(&field.field_type) {
                    return Err(Error::RowValidation {
                        row_index,
                        reason: format!(
                            "Value '{}' is not compatible with field '{}' of type {}",
                            value.to_bpsv_string(),
                            field.name,
                            field.field_type
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Create a builder from existing BPSV content
    ///
    /// This parses the BPSV content and creates a builder with the same schema and data.
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvBuilder;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
    ///
    /// let builder = BpsvBuilder::from_bpsv(content)?;
    /// assert_eq!(builder.field_count(), 2);
    /// assert_eq!(builder.row_count(), 2);
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn from_bpsv(content: &str) -> Result<Self> {
        let document = BpsvDocument::parse(content)?;

        let mut builder = Self::from_schema(document.schema().clone());
        builder.sequence_number = document.sequence_number();

        // Convert all rows to typed values
        for row in document.rows() {
            let typed_values: Vec<BpsvValue> = row
                .raw_values()
                .iter()
                .zip(document.schema().fields())
                .map(|(value, field)| BpsvValue::parse(value, &field.field_type))
                .collect::<Result<Vec<_>>>()?;

            builder.rows.push(typed_values);
        }

        Ok(builder)
    }
}

impl Default for BpsvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_building() {
        let mut builder = BpsvBuilder::new();

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();
        builder
            .add_field("BuildId", BpsvFieldType::Decimal(4))
            .unwrap();
        builder.set_sequence_number(12345);

        builder
            .add_row(vec![
                BpsvValue::String("us".to_string()),
                BpsvValue::Decimal(1234),
            ])
            .unwrap();

        builder
            .add_row(vec![
                BpsvValue::String("eu".to_string()),
                BpsvValue::Decimal(5678),
            ])
            .unwrap();

        let document = builder.build().unwrap();

        assert_eq!(document.sequence_number(), Some(12345));
        assert_eq!(document.row_count(), 2);
        assert_eq!(document.schema().field_count(), 2);
    }

    #[test]
    fn test_raw_row_addition() {
        let mut builder = BpsvBuilder::new();

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();
        builder
            .add_field("BuildId", BpsvFieldType::Decimal(4))
            .unwrap();

        builder
            .add_raw_row(vec!["us".to_string(), "1234".to_string()])
            .unwrap();
        builder
            .add_raw_row(vec!["eu".to_string(), "5678".to_string()])
            .unwrap();

        let document = builder.build().unwrap();
        assert_eq!(document.row_count(), 2);
    }

    #[test]
    fn test_values_row_addition() {
        let mut builder = BpsvBuilder::new();

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();
        builder
            .add_field("BuildId", BpsvFieldType::Decimal(4))
            .unwrap();

        builder
            .add_raw_row(vec!["us".to_string(), "1234".to_string()])
            .unwrap();
        builder
            .add_raw_row(vec!["eu".to_string(), "5678".to_string()])
            .unwrap();

        let document = builder.build().unwrap();
        assert_eq!(document.row_count(), 2);
    }

    #[test]
    fn test_build_string() {
        let mut builder = BpsvBuilder::new();

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();
        builder
            .add_field("BuildId", BpsvFieldType::Decimal(4))
            .unwrap();
        builder.set_sequence_number(12345);

        builder
            .add_raw_row(vec!["us".to_string(), "1234".to_string()])
            .unwrap();

        let bpsv_string = builder.build_string().unwrap();
        let lines: Vec<&str> = bpsv_string.lines().collect();

        assert_eq!(lines[0], "Region!STRING:0|BuildId!DEC:4");
        assert_eq!(lines[1], "## seqn = 12345");
        assert_eq!(lines[2], "us|1234");
    }

    #[test]
    fn test_from_bpsv() {
        let content = r#"Region!STRING:0|BuildId!DEC:4
## seqn = 12345
us|1234
eu|5678"#;

        let builder = BpsvBuilder::from_bpsv(content).unwrap();

        assert_eq!(builder.field_count(), 2);
        assert_eq!(builder.row_count(), 2);

        let rebuilt = builder.build_string().unwrap();

        // Parse both and compare structure (order might differ)
        let original_doc = BpsvDocument::parse(content).unwrap();
        let rebuilt_doc = BpsvDocument::parse(&rebuilt).unwrap();

        assert_eq!(
            original_doc.sequence_number(),
            rebuilt_doc.sequence_number()
        );
        assert_eq!(original_doc.row_count(), rebuilt_doc.row_count());
        assert_eq!(
            original_doc.schema().field_count(),
            rebuilt_doc.schema().field_count()
        );
    }

    #[test]
    fn test_validation() {
        let mut builder = BpsvBuilder::new();

        // No fields defined
        assert!(builder.validate().is_err());

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();
        builder
            .add_field("BuildId", BpsvFieldType::Decimal(4))
            .unwrap();

        // Valid now
        assert!(builder.validate().is_ok());

        // Add compatible row
        builder
            .add_row(vec![
                BpsvValue::String("us".to_string()),
                BpsvValue::Decimal(1234),
            ])
            .unwrap();

        assert!(builder.validate().is_ok());
    }

    #[test]
    fn test_schema_mismatch_errors() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();

        // Too many values
        let result = builder.add_row(vec![
            BpsvValue::String("us".to_string()),
            BpsvValue::Decimal(1234),
        ]);
        assert!(matches!(result, Err(Error::SchemaMismatch { .. })));

        // Too few values
        let result = builder.add_row(vec![]);
        assert!(matches!(result, Err(Error::SchemaMismatch { .. })));
    }

    #[test]
    fn test_incompatible_value_types() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();

        // Try to add decimal value to string field
        let result = builder.add_row(vec![BpsvValue::Decimal(1234)]);
        assert!(matches!(result, Err(Error::InvalidValue { .. })));
    }

    #[test]
    fn test_builder_state_methods() {
        let mut builder = BpsvBuilder::new();

        assert_eq!(builder.field_count(), 0);
        assert_eq!(builder.row_count(), 0);
        assert!(!builder.has_fields());
        assert!(!builder.has_rows());

        builder
            .add_field("Region", BpsvFieldType::String(0))
            .unwrap();

        assert_eq!(builder.field_count(), 1);
        assert!(builder.has_fields());
        assert!(!builder.has_rows());

        builder.add_values_row(vec!["us"]).unwrap();

        assert_eq!(builder.row_count(), 1);
        assert!(builder.has_rows());

        builder.clear_rows();

        assert_eq!(builder.row_count(), 0);
        assert!(!builder.has_rows());
        assert!(builder.has_fields());

        builder.reset();

        assert_eq!(builder.field_count(), 0);
        assert_eq!(builder.row_count(), 0);
        assert!(!builder.has_fields());
        assert!(!builder.has_rows());
    }
}
