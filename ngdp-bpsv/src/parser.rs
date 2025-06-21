//! BPSV parser implementation

use crate::document::BpsvDocument;
use crate::error::{Error, Result};
use crate::schema::BpsvSchema;

/// Parser for BPSV format data
pub struct BpsvParser;

impl BpsvParser {
    /// Parse BPSV content into a document
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvParser;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
    ///
    /// let doc = BpsvParser::parse(content)?;
    /// assert_eq!(doc.sequence_number(), Some(12345));
    /// assert_eq!(doc.rows().len(), 2);
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse(content: &str) -> Result<BpsvDocument> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Err(Error::EmptyDocument);
        }

        let mut line_index = 0;

        // Find and parse the header line
        let header_line = lines.get(line_index).ok_or(Error::MissingHeader)?;

        if !header_line.contains('!') {
            return Err(Error::InvalidHeader {
                reason: "Header line must contain field type specifications with '!'".to_string(),
            });
        }

        let schema = BpsvSchema::parse_header(header_line)?;
        line_index += 1;

        let mut document = BpsvDocument::new(schema);
        let mut sequence_number = None;

        // Process remaining lines
        while line_index < lines.len() {
            let line = lines[line_index].trim();
            line_index += 1;

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Parse sequence number line
            if line.starts_with("## seqn") {
                sequence_number = Self::parse_sequence_line(line)?;
                continue;
            }

            // Parse data row
            let values = Self::parse_data_line(line)?;
            document.add_row(values).map_err(|e| match e {
                Error::SchemaMismatch { expected, actual } => Error::RowValidation {
                    row_index: document.row_count(),
                    reason: format!("Expected {} fields, got {}", expected, actual),
                },
                Error::InvalidValue {
                    field,
                    field_type,
                    value,
                } => Error::RowValidation {
                    row_index: document.row_count(),
                    reason: format!(
                        "Invalid value for field '{}' of type {}: {}",
                        field, field_type, value
                    ),
                },
                other => other,
            })?;
        }

        document.set_sequence_number(sequence_number);
        Ok(document)
    }

    /// Parse a sequence number line like "## seqn = 12345"
    fn parse_sequence_line(line: &str) -> Result<Option<u32>> {
        // Handle various formats:
        // "## seqn = 12345"
        // "##seqn=12345"
        // "## seqn= 12345"
        // etc.

        let line = line.trim();
        if !line.starts_with("##") {
            return Err(Error::InvalidSequenceNumber {
                line: line.to_string(),
            });
        }

        let after_hash = &line[2..].trim();

        if !after_hash.starts_with("seqn") {
            return Err(Error::InvalidSequenceNumber {
                line: line.to_string(),
            });
        }

        let after_seqn = &after_hash[4..].trim();

        if !after_seqn.starts_with('=') {
            return Err(Error::InvalidSequenceNumber {
                line: line.to_string(),
            });
        }

        let number_part = &after_seqn[1..].trim();

        let seqn = number_part
            .parse::<u32>()
            .map_err(|_| Error::InvalidSequenceNumber {
                line: line.to_string(),
            })?;

        Ok(Some(seqn))
    }

    /// Parse a data line into field values
    fn parse_data_line(line: &str) -> Result<Vec<String>> {
        // Split on pipe, but handle empty fields correctly
        let values: Vec<String> = line.split('|').map(|s| s.to_string()).collect();
        Ok(values)
    }

    /// Parse just the header to get schema information
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvParser;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234";
    ///
    /// let schema = BpsvParser::parse_schema(content)?;
    /// assert_eq!(schema.field_count(), 2);
    /// assert!(schema.has_field("Region"));
    /// assert!(schema.has_field("BuildId"));
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse_schema(content: &str) -> Result<BpsvSchema> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Err(Error::EmptyDocument);
        }

        let header_line = lines.first().ok_or(Error::MissingHeader)?;

        if !header_line.contains('!') {
            return Err(Error::InvalidHeader {
                reason: "Header line must contain field type specifications with '!'".to_string(),
            });
        }

        BpsvSchema::parse_header(header_line)
    }

    /// Parse and extract only the sequence number
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvParser;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234";
    ///
    /// let seqn = BpsvParser::parse_sequence_number(content)?;
    /// assert_eq!(seqn, Some(12345));
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn parse_sequence_number(content: &str) -> Result<Option<u32>> {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("## seqn") {
                return Self::parse_sequence_line(line);
            }
        }
        Ok(None)
    }

    /// Parse only the data rows (skip header and sequence)
    ///
    /// Returns raw rows without validation against schema.
    pub fn parse_raw_rows(content: &str) -> Result<Vec<Vec<String>>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut rows = Vec::new();
        let mut found_header = false;

        for line in lines {
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Skip header line
            if line.contains('!') && !found_header {
                found_header = true;
                continue;
            }

            // Skip sequence number line
            if line.starts_with("## seqn") {
                continue;
            }

            // Skip comment lines
            if line.starts_with("##") {
                continue;
            }

            // Parse data line
            let values = Self::parse_data_line(line)?;
            rows.push(values);
        }

        Ok(rows)
    }

    /// Validate BPSV content without full parsing
    ///
    /// Returns Ok(()) if the content is valid BPSV format, Err otherwise.
    pub fn validate(content: &str) -> Result<()> {
        let _document = Self::parse(content)?;
        Ok(())
    }

    /// Get basic statistics about BPSV content
    ///
    /// Returns (field_count, row_count, has_sequence_number)
    pub fn get_stats(content: &str) -> Result<(usize, usize, bool)> {
        let schema = Self::parse_schema(content)?;
        let raw_rows = Self::parse_raw_rows(content)?;
        let has_seqn = Self::parse_sequence_number(content)?.is_some();

        Ok((schema.field_count(), raw_rows.len(), has_seqn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_complete_document() {
        let content = r"Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4
## seqn = 12345
us|abcd1234abcd1234abcd1234abcd1234|1234
eu|1234abcd1234abcd1234abcd1234abcd|5678";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.sequence_number(), Some(12345));
        assert_eq!(doc.row_count(), 2);
        assert_eq!(doc.schema().field_count(), 3);

        let first_row = doc.get_row(0).unwrap();
        assert_eq!(first_row.get_raw(0), Some("us"));
        assert_eq!(
            first_row.get_raw(1),
            Some("abcd1234abcd1234abcd1234abcd1234")
        );
        assert_eq!(first_row.get_raw(2), Some("1234"));
    }

    #[test]
    fn test_parse_without_sequence() {
        let content = r"Region!STRING:0|BuildId!DEC:4
us|1234
eu|5678";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.sequence_number(), None);
        assert_eq!(doc.row_count(), 2);
    }

    #[test]
    fn test_parse_empty_fields() {
        let content = r"Region!STRING:0|BuildId!DEC:4|Optional!STRING:0
us|1234|
eu||optional_value";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.row_count(), 2);

        let first_row = doc.get_row(0).unwrap();
        assert_eq!(first_row.get_raw(2), Some(""));

        let second_row = doc.get_row(1).unwrap();
        assert_eq!(second_row.get_raw(1), Some(""));
        assert_eq!(second_row.get_raw(2), Some("optional_value"));
    }

    #[test]
    fn test_parse_sequence_variations() {
        let variations = [
            "## seqn = 12345",
            "##seqn=12345",
            "## seqn= 12345",
            "##  seqn  =  12345  ",
        ];

        for variation in &variations {
            let result = BpsvParser::parse_sequence_line(variation);
            assert_eq!(result.unwrap(), Some(12345), "Failed for: {variation}");
        }
    }

    #[test]
    fn test_invalid_sequence_lines() {
        let invalid = [
            "# seqn = 12345", // Single hash
            "## seq = 12345", // Wrong keyword
            "## seqn 12345",  // Missing equals
            "## seqn = abc",  // Invalid number
        ];

        for invalid_line in &invalid {
            let result = BpsvParser::parse_sequence_line(invalid_line);
            assert!(result.is_err(), "Should have failed for: {invalid_line}");
        }
    }

    #[test]
    fn test_parse_schema_only() {
        let content = r"Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4
## seqn = 12345
us|abcd1234abcd1234abcd1234abcd1234|1234";

        let schema = BpsvParser::parse_schema(content).unwrap();

        assert_eq!(schema.field_count(), 3);
        assert!(schema.has_field("Region"));
        assert!(schema.has_field("BuildConfig"));
        assert!(schema.has_field("BuildId"));
    }

    #[test]
    fn test_parse_raw_rows() {
        let content = r"Region!STRING:0|BuildId!DEC:4
## seqn = 12345
us|1234
eu|5678
kr|9999";

        let rows = BpsvParser::parse_raw_rows(content).unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["us", "1234"]);
        assert_eq!(rows[1], vec!["eu", "5678"]);
        assert_eq!(rows[2], vec!["kr", "9999"]);
    }

    #[test]
    fn test_get_stats() {
        let content = r"Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4
## seqn = 12345
us|abcd1234abcd1234abcd1234abcd1234|1234
eu|1234abcd1234abcd1234abcd1234abcd|5678";

        let (field_count, row_count, has_seqn) = BpsvParser::get_stats(content).unwrap();

        assert_eq!(field_count, 3);
        assert_eq!(row_count, 2);
        assert!(has_seqn);
    }

    #[test]
    fn test_invalid_documents() {
        // Empty document
        assert!(BpsvParser::parse("").is_err());

        // Missing header
        assert!(BpsvParser::parse("us|1234").is_err());

        // Invalid header format
        assert!(BpsvParser::parse("RegionBuildId\nus|1234").is_err());
    }

    #[test]
    fn test_schema_mismatch_in_parsing() {
        let content = r"Region!STRING:0|BuildId!DEC:4
us|1234|extra_field"; // Too many fields

        let result = BpsvParser::parse(content);
        assert!(matches!(result, Err(Error::RowValidation { .. })));
    }

    #[test]
    fn test_case_insensitive_types() {
        let content = r"Region!string:0|BuildId!dec:4
us|1234";

        let doc = BpsvParser::parse(content).unwrap();
        assert_eq!(doc.schema().field_count(), 2);
    }
}
