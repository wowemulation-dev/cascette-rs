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
    pub fn parse(data: &[u8]) -> Result<Self, EncodingError> {
        let mut entries = Vec::new();
        let mut current = Vec::new();

        for &byte in data {
            if byte == 0 {
                // Null terminator - end of string
                if !current.is_empty() {
                    // ESpec strings should be ASCII (a-z, 0-9, :, {, }, *, =, ,)
                    // but we'll be lenient and just check if it's valid UTF-8
                    let spec = if let Ok(spec) = String::from_utf8(current.clone()) {
                        spec
                    } else {
                        // If not valid UTF-8, convert as lossy
                        String::from_utf8_lossy(&current).to_string()
                    };
                    entries.push(spec);
                    current.clear();
                }
            } else {
                current.push(byte);
            }
        }

        // Add last string if not null-terminated
        if !current.is_empty() {
            // Use lossy conversion to handle any non-UTF-8 bytes
            let spec = String::from_utf8_lossy(&current).to_string();
            entries.push(spec);
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
