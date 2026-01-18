use crate::bpsv::document::BpsvDocument;
use crate::bpsv::schema::BpsvSchema;
use crate::bpsv::types::BpsvError;
use std::io::{BufRead, BufReader, Read};

/// BPSV document reader
pub struct BpsvReader<R> {
    reader: BufReader<R>,
}

impl<R: Read> BpsvReader<R> {
    /// Create a new reader from any `Read` source
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
        }
    }

    /// Read and parse a complete BPSV document
    pub fn read_document(&mut self) -> Result<BpsvDocument, BpsvError> {
        let mut lines = Vec::new();
        let mut line_buffer = String::new();

        // Read all lines
        while self.reader.read_line(&mut line_buffer)? > 0 {
            lines.push(line_buffer.trim_end().to_string());
            line_buffer.clear();
        }

        if lines.is_empty() {
            return Err(BpsvError::EmptyDocument);
        }

        // Parse header (first line)
        let header_line = &lines[0];
        if !header_line.contains('!') {
            return Err(BpsvError::InvalidHeader(
                "Header must contain field type specifications".to_string(),
            ));
        }

        let schema = BpsvSchema::parse(header_line)?;
        let mut document = BpsvDocument::new(schema);

        // Process remaining lines
        for line in lines.iter().skip(1) {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Check for sequence number
            if trimmed.starts_with("## seqn") {
                let seqn = parse_sequence_line(trimmed)?;
                if let Some(n) = seqn {
                    document.set_sequence_number(n);
                }
                continue;
            }

            // Skip other comments
            if trimmed.starts_with('#') {
                continue;
            }

            // Parse data row
            let values: Vec<String> = trimmed
                .split('|')
                .map(std::string::ToString::to_string)
                .collect();
            document.add_raw_row(values)?;
        }

        Ok(document)
    }

    /// Read only the schema without parsing data rows
    pub fn read_schema(&mut self) -> Result<BpsvSchema, BpsvError> {
        let mut line = String::new();

        // Read first non-empty line
        loop {
            if self.reader.read_line(&mut line)? == 0 {
                return Err(BpsvError::EmptyDocument);
            }

            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
            line.clear();
        }

        BpsvSchema::parse(line.trim())
    }
}

impl<'a> BpsvReader<&'a [u8]> {
    /// Create a reader from a byte slice
    #[must_use]
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Self::new(bytes)
    }
}

impl BpsvReader<std::fs::File> {
    /// Create a reader from a file path
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        Ok(Self::new(file))
    }
}

/// Parse a BPSV document from a string
pub fn parse(content: &str) -> Result<BpsvDocument, BpsvError> {
    let mut reader = BpsvReader::from_bytes(content.as_bytes());
    reader.read_document()
}

/// Parse only the schema from a string
pub fn parse_schema(content: &str) -> Result<BpsvSchema, BpsvError> {
    let mut reader = BpsvReader::from_bytes(content.as_bytes());
    reader.read_schema()
}

/// Parse a sequence number line
fn parse_sequence_line(line: &str) -> Result<Option<u32>, BpsvError> {
    // Handle "## seqn = 12345" format and variations
    let after_seqn = line
        .strip_prefix("## seqn")
        .ok_or_else(|| BpsvError::InvalidSequenceNumber(line.to_string()))?
        .trim_start();

    if after_seqn.is_empty() {
        return Err(BpsvError::InvalidSequenceNumber(line.to_string()));
    }

    // Handle different separators
    let number_str = if let Some(pos) = after_seqn.find('=') {
        after_seqn[pos + 1..].trim()
    } else if let Some(pos) = after_seqn.find(':') {
        after_seqn[pos + 1..].trim()
    } else {
        after_seqn
    };

    let seqn = number_str
        .parse::<u32>()
        .map_err(|_| BpsvError::InvalidSequenceNumber(line.to_string()))?;

    Ok(Some(seqn))
}

// Implement std::io::Error conversion for BpsvError
impl From<std::io::Error> for BpsvError {
    fn from(_: std::io::Error) -> Self {
        Self::EmptyDocument // Simple conversion for now
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_complete_document() {
        let content = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4
## seqn = 12345
us|abcd1234abcd1234abcd1234abcd1234|1234
eu|1234abcd1234abcd1234abcd1234abcd|5678";

        let doc = parse(content).expect("Test operation should succeed");
        assert_eq!(doc.sequence_number(), Some(12345));
        assert_eq!(doc.row_count(), 2);
        assert_eq!(doc.schema().field_count(), 3);

        let row1 = doc.get_row(0).expect("Test operation should succeed");
        assert_eq!(row1.get_raw(0), Some("us"));
        assert_eq!(row1.get_raw(1), Some("abcd1234abcd1234abcd1234abcd1234"));
        assert_eq!(row1.get_raw(2), Some("1234"));
    }

    #[test]
    fn test_parse_without_sequence() {
        let content = "Region!STRING:0|BuildId!DEC:4
us|1234
eu|5678";

        let doc = parse(content).expect("Test operation should succeed");
        assert_eq!(doc.sequence_number(), None);
        assert_eq!(doc.row_count(), 2);
    }

    #[test]
    fn test_parse_empty_fields() {
        let content = "Field1!STRING:0|Field2!STRING:0|Field3!STRING:0
a||c
|b|";

        let doc = parse(content).expect("Test operation should succeed");

        let row1 = doc.get_row(0).expect("Test operation should succeed");
        assert_eq!(row1.get_raw(0), Some("a"));
        assert_eq!(row1.get_raw(1), Some(""));
        assert_eq!(row1.get_raw(2), Some("c"));

        let row2 = doc.get_row(1).expect("Test operation should succeed");
        assert_eq!(row2.get_raw(0), Some(""));
        assert_eq!(row2.get_raw(1), Some("b"));
        assert_eq!(row2.get_raw(2), Some(""));
    }

    #[test]
    fn test_parse_with_comments() {
        let content = "Region!STRING:0|BuildId!DEC:4
## seqn = 99999
# This is a comment
us|1234
# Another comment
eu|5678
### More comments
cn|9999";

        let doc = parse(content).expect("Test operation should succeed");
        assert_eq!(doc.sequence_number(), Some(99999));
        assert_eq!(doc.row_count(), 3);
    }

    #[test]
    fn test_parse_sequence_variations() {
        // Test with equals
        let result = parse_sequence_line("## seqn = 12345").expect("Test operation should succeed");
        assert_eq!(result, Some(12345));

        // Test with colon
        let result = parse_sequence_line("## seqn: 67890").expect("Test operation should succeed");
        assert_eq!(result, Some(67890));

        // Test with just space
        let result = parse_sequence_line("## seqn 11111").expect("Test operation should succeed");
        assert_eq!(result, Some(11111));

        // Test with extra spaces
        let result =
            parse_sequence_line("## seqn   =   99999").expect("Test operation should succeed");
        assert_eq!(result, Some(99999));
    }

    #[test]
    fn test_invalid_sequence_lines() {
        // No number
        let result = parse_sequence_line("## seqn = ");
        assert!(result.is_err());

        // Invalid number
        let result = parse_sequence_line("## seqn = abc");
        assert!(result.is_err());

        // Just the prefix
        let result = parse_sequence_line("## seqn");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_schema_only() {
        let content = "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4
## seqn = 12345
us|abcd|1234";

        let schema = parse_schema(content).expect("Test operation should succeed");
        assert_eq!(schema.field_count(), 3);
        assert!(schema.has_field("Region"));
        assert!(schema.has_field("BuildConfig"));
        assert!(schema.has_field("BuildId"));
    }

    #[test]
    fn test_empty_document() {
        let result = parse("");
        assert!(matches!(result, Err(BpsvError::EmptyDocument)));
    }

    #[test]
    fn test_invalid_header() {
        // No field types
        let result = parse(
            "Region|BuildId
us|1234",
        );
        assert!(matches!(result, Err(BpsvError::InvalidHeader(_))));
    }

    #[test]
    fn test_reader_from_bytes() {
        let content = b"Field!STRING:0
value";

        let mut reader = BpsvReader::from_bytes(&content[..]);
        let doc = reader
            .read_document()
            .expect("Test operation should succeed");
        assert_eq!(doc.row_count(), 1);
    }

    #[test]
    fn test_case_insensitive_types() {
        let content = "Region!string:0|BuildId!DEC:4|Config!hex:16
us|1234|abcdabcdabcdabcdabcdabcdabcdabcd";

        let doc = parse(content).expect("Test operation should succeed");
        assert_eq!(doc.row_count(), 1);
        assert_eq!(doc.schema().field_count(), 3);
    }
}
