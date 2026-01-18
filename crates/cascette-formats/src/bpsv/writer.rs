use crate::bpsv::document::BpsvDocument;
use crate::bpsv::row::BpsvRow;
use crate::bpsv::schema::BpsvSchema;
use crate::bpsv::types::{BpsvError, BpsvField, BpsvValue};
use std::io::{BufWriter, Write};

/// BPSV document writer
pub struct BpsvWriter<W: Write> {
    writer: BufWriter<W>,
}

impl<W: Write> BpsvWriter<W> {
    /// Create a new writer
    pub fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::new(writer),
        }
    }

    /// Write a complete BPSV document
    pub fn write_document(&mut self, document: &BpsvDocument) -> Result<(), std::io::Error> {
        // Write header
        writeln!(self.writer, "{}", document.schema().to_header())?;

        // Write sequence number if present
        if let Some(seqn) = document.sequence_number() {
            writeln!(self.writer, "## seqn = {seqn}")?;
        }

        // Write data rows
        for row in document.rows() {
            writeln!(self.writer, "{}", row.to_line())?;
        }

        self.writer.flush()?;
        Ok(())
    }

    /// Write only the schema header
    pub fn write_schema(&mut self, schema: &BpsvSchema) -> Result<(), std::io::Error> {
        writeln!(self.writer, "{}", schema.to_header())?;
        self.writer.flush()?;
        Ok(())
    }

    /// Write a single row
    pub fn write_row(&mut self, row: &BpsvRow) -> Result<(), std::io::Error> {
        writeln!(self.writer, "{}", row.to_line())?;
        self.writer.flush()?;
        Ok(())
    }

    /// Write a sequence number line
    pub fn write_sequence(&mut self, seqn: u32) -> Result<(), std::io::Error> {
        writeln!(self.writer, "## seqn = {seqn}")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Write a comment line
    pub fn write_comment(&mut self, comment: &str) -> Result<(), std::io::Error> {
        writeln!(self.writer, "# {comment}")?;
        self.writer.flush()?;
        Ok(())
    }

    /// Get the inner writer
    pub fn into_inner(self) -> Result<W, std::io::Error> {
        self.writer
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)
    }
}

/// Builder for creating BPSV documents
pub struct BpsvBuilder {
    fields: Vec<BpsvField>,
    rows: Vec<Vec<BpsvValue>>,
    sequence_number: Option<u32>,
}

impl BpsvBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            rows: Vec::new(),
            sequence_number: None,
        }
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, field: BpsvField) -> &mut Self {
        self.fields.push(field);
        self
    }

    /// Add multiple fields
    pub fn add_fields(&mut self, fields: Vec<BpsvField>) -> &mut Self {
        self.fields.extend(fields);
        self
    }

    /// Set the sequence number
    pub fn set_sequence(&mut self, seqn: u32) -> &mut Self {
        self.sequence_number = Some(seqn);
        self
    }

    /// Add a row of values
    pub fn add_row(&mut self, values: Vec<BpsvValue>) -> Result<&mut Self, BpsvError> {
        if values.len() != self.fields.len() {
            return Err(BpsvError::FieldCountMismatch {
                expected: self.fields.len(),
                actual: values.len(),
            });
        }
        self.rows.push(values);
        Ok(self)
    }

    /// Build the document
    #[must_use]
    #[allow(clippy::expect_used)] // Field count validated in add_row, cannot fail
    pub fn build(self) -> BpsvDocument {
        let schema = BpsvSchema::new(self.fields);
        let mut document = BpsvDocument::new(schema);

        if let Some(seqn) = self.sequence_number {
            document.set_sequence_number(seqn);
        }

        for row_values in self.rows {
            let row = BpsvRow::from_values(row_values);
            // Field count validated in add_row
            document
                .add_row(row)
                .expect("Field count was validated in add_row");
        }

        document
    }
}

impl Default for BpsvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a BPSV document to a string
#[must_use]
pub fn format(document: &BpsvDocument) -> String {
    document.to_string()
}

/// Write a BPSV document to a file
pub fn write_to_file<P: AsRef<std::path::Path>>(
    path: P,
    document: &BpsvDocument,
) -> Result<(), std::io::Error> {
    let file = std::fs::File::create(path)?;
    let mut writer = BpsvWriter::new(file);
    writer.write_document(document)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bpsv::types::BpsvType;

    #[test]
    fn test_write_document() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field(BpsvField::new("Region", BpsvType::String(0)))
            .add_field(BpsvField::new("BuildId", BpsvType::Dec(4)))
            .set_sequence(12345);

        builder
            .add_row(vec![
                BpsvValue::String("us".to_string()),
                BpsvValue::Dec(1234),
            ])
            .expect("Test operation should succeed");
        builder
            .add_row(vec![
                BpsvValue::String("eu".to_string()),
                BpsvValue::Dec(5678),
            ])
            .expect("Test operation should succeed");

        let document = builder.build();
        let output = format(&document);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "Region!STRING:0|BuildId!DEC:4");
        assert_eq!(lines[1], "## seqn = 12345");
        assert_eq!(lines[2], "us|1234");
        assert_eq!(lines[3], "eu|5678");
    }

    #[test]
    fn test_writer_to_vec() {
        let schema = BpsvSchema::new(vec![
            BpsvField::new("Field1", BpsvType::String(0)),
            BpsvField::new("Field2", BpsvType::Hex(4)),
        ]);
        let mut document = BpsvDocument::new(schema);
        document
            .add_raw_row(vec!["test".to_string(), "abcd".to_string()])
            .expect("Test operation should succeed");

        let mut buffer = Vec::new();
        {
            let mut writer = BpsvWriter::new(&mut buffer);
            writer
                .write_document(&document)
                .expect("Test operation should succeed");
        }

        let output = String::from_utf8(buffer).expect("Test operation should succeed");
        assert!(output.contains("Field1!STRING:0|Field2!HEX:4"));
        assert!(output.contains("test|abcd"));
    }

    #[test]
    fn test_writer_comments() {
        let mut buffer = Vec::new();
        {
            let mut writer = BpsvWriter::new(&mut buffer);
            writer
                .write_comment("This is a test comment")
                .expect("Test operation should succeed");
            writer
                .write_comment("Another comment")
                .expect("Test operation should succeed");
        }

        let output = String::from_utf8(buffer).expect("Test operation should succeed");
        assert!(output.contains("# This is a test comment"));
        assert!(output.contains("# Another comment"));
    }

    #[test]
    fn test_builder_field_count_validation() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field(BpsvField::new("Field1", BpsvType::String(0)))
            .add_field(BpsvField::new("Field2", BpsvType::Dec(4)));

        // Correct number of values
        let result = builder.add_row(vec![
            BpsvValue::String("test".to_string()),
            BpsvValue::Dec(42),
        ]);
        assert!(result.is_ok());

        // Wrong number of values
        let result = builder.add_row(vec![BpsvValue::String("test".to_string())]);
        assert!(matches!(
            result,
            Err(BpsvError::FieldCountMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn test_builder_empty_values() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field(BpsvField::new("A", BpsvType::String(0)))
            .add_field(BpsvField::new("B", BpsvType::String(0)))
            .add_field(BpsvField::new("C", BpsvType::String(0)));

        builder
            .add_row(vec![
                BpsvValue::String("a".to_string()),
                BpsvValue::Empty,
                BpsvValue::String("c".to_string()),
            ])
            .expect("Test operation should succeed");

        let document = builder.build();
        let output = format(&document);

        assert!(output.contains("a||c"));
    }

    #[test]
    fn test_builder_hex_values() {
        let mut builder = BpsvBuilder::new();
        builder
            .add_field(BpsvField::new("Hash", BpsvType::Hex(16)))
            .add_field(BpsvField::new("Size", BpsvType::Dec(4)));

        builder
            .add_row(vec![
                BpsvValue::Hex(vec![0xde, 0xad, 0xbe, 0xef]),
                BpsvValue::Dec(1024),
            ])
            .expect("Test operation should succeed");

        let document = builder.build();
        let output = format(&document);

        assert!(output.contains("deadbeef|1024"));
    }
}
