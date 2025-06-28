//! BPSV document parser

use crate::document::BpsvDocument;
use crate::error::{Error, Result};
use crate::schema::BpsvSchema;

/// Parser for BPSV documents
pub struct BpsvParser;

impl BpsvParser {
    /// Parse a complete BPSV document
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
    pub fn parse(content: &str) -> Result<BpsvDocument<'_>> {
        if content.is_empty() {
            return Err(Error::EmptyDocument);
        }

        let mut lines = content.lines();

        // Parse header
        let header_line = lines.next().ok_or(Error::MissingHeader)?;
        
        if !header_line.contains('!') {
            return Err(Error::InvalidHeader {
                reason: "Header line must contain field type specifications with '!'".to_string(),
            });
        }

        let schema = BpsvSchema::parse_header(header_line)?;
        let mut doc = BpsvDocument::new(content, schema);

        // Process remaining lines
        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Check for sequence number line
            if trimmed.starts_with("## seqn") {
                let seqn = Self::parse_sequence_line(trimmed)?;
                doc.set_sequence_number(seqn);
                continue;
            }

            // Skip other comment lines
            if trimmed.starts_with('#') {
                continue;
            }

            // Parse data line
            let values = Self::parse_data_line(trimmed);
            doc.add_row(values)?;
        }

        Ok(doc)
    }

    /// Parse a sequence number line
    ///
    /// Handles formats:
    /// - `## seqn = 12345`
    /// - `## seqn: 12345`
    /// - `## seqn 12345`
    fn parse_sequence_line(line: &str) -> Result<Option<u32>> {
        // Start after "## seqn"
        let after_seqn = &line[7..].trim_start();

        if after_seqn.is_empty() {
            return Err(Error::InvalidSequenceNumber {
                line: line.to_string(),
            });
        }

        // Handle different separators
        let number_str = if let Some(eq_pos) = after_seqn.find('=') {
            after_seqn[eq_pos + 1..].trim()
        } else if let Some(colon_pos) = after_seqn.find(':') {
            after_seqn[colon_pos + 1..].trim()
        } else {
            // No separator, just whitespace
            after_seqn
        };

        let seqn = number_str
            .parse::<u32>()
            .map_err(|_| Error::InvalidSequenceNumber {
                line: line.to_string(),
            })?;

        Ok(Some(seqn))
    }

    /// Parse a data line into field values (zero-copy)
    fn parse_data_line(line: &str) -> Vec<&str> {
        line.split('|').collect()
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
        let first_line = content
            .lines()
            .next()
            .ok_or(Error::EmptyDocument)?;

        if !first_line.contains('!') {
            return Err(Error::InvalidHeader {
                reason: "Header line must contain field type specifications with '!'".to_string(),
            });
        }

        BpsvSchema::parse_header(first_line)
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
    pub fn parse_raw_rows(content: &str) -> Result<Vec<Vec<&str>>> {
        let mut rows = Vec::new();
        let mut past_header = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // First non-empty line should be header
            if !past_header {
                if !trimmed.contains('!') {
                    return Err(Error::InvalidHeader {
                        reason: "First line must be header".to_string(),
                    });
                }
                past_header = true;
                continue;
            }

            // Skip comment lines
            if trimmed.starts_with('#') {
                continue;
            }

            // Parse data line
            rows.push(Self::parse_data_line(trimmed));
        }

        Ok(rows)
    }

    /// Get basic statistics about a BPSV document without full parsing
    ///
    /// # Examples
    ///
    /// ```
    /// use ngdp_bpsv::BpsvParser;
    ///
    /// let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678";
    ///
    /// let (field_count, row_count, has_seqn) = BpsvParser::get_stats(content)?;
    /// assert_eq!(field_count, 2);
    /// assert_eq!(row_count, 2);
    /// assert!(has_seqn);
    /// # Ok::<(), ngdp_bpsv::Error>(())
    /// ```
    pub fn get_stats(content: &str) -> Result<(usize, usize, bool)> {
        let mut field_count = 0;
        let mut row_count = 0;
        let mut has_seqn = false;
        let mut past_header = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // First non-empty line should be header
            if !past_header {
                if !trimmed.contains('!') {
                    return Err(Error::InvalidHeader {
                        reason: "First line must be header".to_string(),
                    });
                }
                field_count = trimmed.split('|').count();
                past_header = true;
                continue;
            }

            // Check for sequence number
            if trimmed.starts_with("## seqn") {
                has_seqn = true;
                continue;
            }

            // Skip other comment lines
            if trimmed.starts_with('#') {
                continue;
            }

            // Count data row
            row_count += 1;
        }

        Ok((field_count, row_count, has_seqn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_complete_document() {
        let content = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4\n## seqn = 12345\nus|abcd1234abcd1234abcd1234abcd1234|1234\neu|1234abcd1234abcd1234abcd1234abcd|5678";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.sequence_number(), Some(12345));
        assert_eq!(doc.row_count(), 2);
        assert_eq!(doc.schema().field_count(), 3);

        let row1 = doc.get_row(0).unwrap();
        assert_eq!(row1.get_raw(0), Some("us"));
        assert_eq!(row1.get_raw(1), Some("abcd1234abcd1234abcd1234abcd1234"));
        assert_eq!(row1.get_raw(2), Some("1234"));
    }

    #[test]
    fn test_parse_without_sequence() {
        let content = "Region!STRING:0|BuildId!DEC:4\nus|1234\neu|5678";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.sequence_number(), None);
        assert_eq!(doc.row_count(), 2);
    }

    #[test]
    fn test_parse_empty_fields() {
        let content = "Field1!STRING:0|Field2!STRING:0|Field3!STRING:0\na||c\n|b|";

        let doc = BpsvParser::parse(content).unwrap();

        let row1 = doc.get_row(0).unwrap();
        assert_eq!(row1.get_raw(0), Some("a"));
        assert_eq!(row1.get_raw(1), Some(""));
        assert_eq!(row1.get_raw(2), Some("c"));

        let row2 = doc.get_row(1).unwrap();
        assert_eq!(row2.get_raw(0), Some(""));
        assert_eq!(row2.get_raw(1), Some("b"));
        assert_eq!(row2.get_raw(2), Some(""));
    }

    #[test]
    fn test_parse_sequence_variations() {
        // Test with equals sign
        let result = BpsvParser::parse_sequence_line("## seqn = 12345").unwrap();
        assert_eq!(result, Some(12345));

        // Test with colon
        let result = BpsvParser::parse_sequence_line("## seqn: 67890").unwrap();
        assert_eq!(result, Some(67890));

        // Test with just space
        let result = BpsvParser::parse_sequence_line("## seqn 11111").unwrap();
        assert_eq!(result, Some(11111));

        // Test with extra spaces
        let result = BpsvParser::parse_sequence_line("## seqn   =   99999").unwrap();
        assert_eq!(result, Some(99999));
    }

    #[test]
    fn test_invalid_sequence_lines() {
        // No number
        let result = BpsvParser::parse_sequence_line("## seqn = ");
        assert!(result.is_err());

        // Invalid number
        let result = BpsvParser::parse_sequence_line("## seqn = abc");
        assert!(result.is_err());

        // Just the prefix
        let result = BpsvParser::parse_sequence_line("## seqn");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_schema_only() {
        let content = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4\n## seqn = 12345\nus|abcd|1234";

        let schema = BpsvParser::parse_schema(content).unwrap();

        assert_eq!(schema.field_count(), 3);
        assert!(schema.has_field("Region"));
        assert!(schema.has_field("BuildConfig"));
        assert!(schema.has_field("BuildId"));
    }

    #[test]
    fn test_parse_raw_rows() {
        let content = "Region!STRING:0|BuildId!DEC:4\n## seqn = 12345\nus|1234\neu|5678\n# comment\ncn|9999";

        let rows = BpsvParser::parse_raw_rows(content).unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["us", "1234"]);
        assert_eq!(rows[1], vec!["eu", "5678"]);
        assert_eq!(rows[2], vec!["cn", "9999"]);
    }

    #[test]
    fn test_get_stats() {
        let content = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4\n## seqn = 12345\nus|abcd|1234\neu|efgh|5678\n# comment\ncn|ijkl|9999";

        let (field_count, row_count, has_seqn) = BpsvParser::get_stats(content).unwrap();

        assert_eq!(field_count, 3);
        assert_eq!(row_count, 3);
        assert!(has_seqn);
    }

    #[test]
    fn test_empty_document() {
        let result = BpsvParser::parse("");
        assert!(matches!(result, Err(Error::EmptyDocument)));
    }

    #[test]
    fn test_invalid_documents() {
        // No header
        let result = BpsvParser::parse("us|1234");
        assert!(matches!(result, Err(Error::InvalidHeader { .. })));

        // Header without field types
        let result = BpsvParser::parse("Region|BuildId\nus|1234");
        assert!(matches!(result, Err(Error::InvalidHeader { .. })));
    }

    #[test]
    fn test_schema_mismatch_in_parsing() {
        let content = "Region!STRING:0|BuildId!DEC:4\nus|1234|extra\neu";

        // Parser should reject rows with wrong number of fields
        let result = BpsvParser::parse(content);
        assert!(result.is_err());
        
        // Ensure the error is about schema mismatch
        if let Err(e) = result {
            assert!(matches!(e, Error::SchemaMismatch { .. }));
        }
    }

    #[test]
    fn test_case_insensitive_types() {
        let content = "Region!string:0|BuildId!DEC:4|Config!hex:16\nus|1234|abcdabcdabcdabcdabcdabcdabcdabcd";

        let doc = BpsvParser::parse(content).unwrap();

        assert_eq!(doc.row_count(), 1);
        assert_eq!(doc.schema().field_count(), 3);
    }
}