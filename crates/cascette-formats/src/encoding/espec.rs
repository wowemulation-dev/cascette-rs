//! `ESpec` table for compression specifications

use crate::encoding::error::EncodingError;

/// `ESpec` table for compression specifications
#[derive(Debug, Clone, Default)]
pub struct ESpecTable {
    /// List of `ESpec` compression specification strings
    pub entries: Vec<String>,
}

impl ESpecTable {
    /// Parse `ESpec` table from raw bytes
    ///
    /// Agent.exe requires:
    /// - No empty strings (consecutive null bytes are rejected)
    /// - Block must be null-terminated (no trailing non-null data)
    pub fn parse(data: &[u8]) -> Result<Self, EncodingError> {
        let mut entries = Vec::new();
        let mut current = Vec::new();

        for &byte in data {
            if byte == 0 {
                // Null terminator - end of string
                if current.is_empty() {
                    // Consecutive nulls or leading null = empty ESpec string
                    return Err(EncodingError::EmptyESpec);
                }
                let spec = String::from_utf8(current.clone())
                    .unwrap_or_else(|_| String::from_utf8_lossy(&current).to_string());
                entries.push(spec);
                current.clear();
            } else {
                current.push(byte);
            }
        }

        // Trailing non-null data means the block is not null-terminated
        if !current.is_empty() {
            return Err(EncodingError::UnterminatedESpec);
        }

        Ok(Self { entries })
    }

    /// Build `ESpec` table into raw bytes
    pub fn build(&self) -> Vec<u8> {
        let mut data = Vec::new();

        for spec in &self.entries {
            data.extend_from_slice(spec.as_bytes());
            data.push(0); // Null terminator
        }

        data
    }

    /// Get `ESpec` string by index (u32)
    pub fn get(&self, index: u32) -> Option<&str> {
        self.entries
            .get(index as usize)
            .map(std::string::String::as_str)
    }

    /// Get `ESpec` string by index (usize)
    pub fn get_by_usize(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(std::string::String::as_str)
    }

    /// Add a new `ESpec` and return its index
    #[allow(clippy::expect_used)] // 4+ billion entries would be unrealistic
    pub fn add(&mut self, spec: String) -> u32 {
        let index = u32::try_from(self.entries.len()).expect("Too many ESpec entries");
        self.entries.push(spec);
        index
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_espec_table() {
        // Two entries, both null-terminated
        let data = b"n:{*=z}\0b:{*=z}\0";
        let table = ESpecTable::parse(data).expect("Should parse");
        assert_eq!(table.entries.len(), 2);
        assert_eq!(table.entries[0], "n:{*=z}");
        assert_eq!(table.entries[1], "b:{*=z}");
    }

    #[test]
    fn test_single_entry() {
        let data = b"n:{*=z}\0";
        let table = ESpecTable::parse(data).expect("Should parse");
        assert_eq!(table.entries.len(), 1);
        assert_eq!(table.entries[0], "n:{*=z}");
    }

    #[test]
    fn test_empty_data() {
        let data = b"";
        let table = ESpecTable::parse(data).expect("Should parse");
        assert!(table.entries.is_empty());
    }

    #[test]
    fn test_consecutive_nulls_rejected() {
        // "spec\0\0" has an empty string between nulls
        let data = b"spec\0\0";
        let result = ESpecTable::parse(data);
        assert!(matches!(result, Err(EncodingError::EmptyESpec)));
    }

    #[test]
    fn test_leading_null_rejected() {
        let data = b"\0spec\0";
        let result = ESpecTable::parse(data);
        assert!(matches!(result, Err(EncodingError::EmptyESpec)));
    }

    #[test]
    fn test_unterminated_data_rejected() {
        // Data not ending with null byte
        let data = b"spec";
        let result = ESpecTable::parse(data);
        assert!(matches!(result, Err(EncodingError::UnterminatedESpec)));
    }

    #[test]
    fn test_round_trip() {
        let mut table = ESpecTable::default();
        table.add("n:{*=z}".to_string());
        table.add("b:{*=z}".to_string());

        let data = table.build();
        let parsed = ESpecTable::parse(&data).expect("Should parse");
        assert_eq!(parsed.entries, table.entries);
    }
}
