//! Keyring Config file format implementation
//!
//! Keyring Config files contain encryption keys for decrypting protected CASC content.
//! Each entry maps an 8-byte key ID to a 16-byte encryption key. Agent.exe uses these
//! keys via its `tact::KeyGetter::LoadKeyring` function for Salsa20 decryption of BLTE
//! encrypted blocks.
//!
//! Keyring config hashes are referenced in the Ribbit versions response `KeyRing` column,
//! not in build configs. The config is fetched from CDN using the standard config path.

use std::io::{BufRead, BufReader, Read, Write};

use super::{is_valid_md5_hex, parse_line};

/// Keyring Configuration containing encryption key entries
///
/// Format: `key-{KEY_ID_HEX} = {KEY_VALUE_HEX}` per line, where KEY_ID is 16 hex
/// chars (8 bytes) and KEY_VALUE is 32 hex chars (16 bytes).
#[derive(Debug, Clone)]
pub struct KeyringConfig {
    /// Ordered list of keyring entries
    entries: Vec<KeyringEntry>,
}

/// A single encryption key entry from the keyring
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyringEntry {
    /// 8-byte key identifier as 16 hex characters (lowercase)
    pub key_id: String,
    /// 16-byte encryption key as 32 hex characters (lowercase)
    pub key_value: String,
}

/// Key ID prefix in config files
const KEY_PREFIX: &str = "key-";

impl KeyringConfig {
    /// Create a new empty `KeyringConfig`
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Parse `KeyringConfig` from a reader
    pub fn parse<R: Read>(reader: R) -> Result<Self, Box<dyn std::error::Error>> {
        let mut entries = Vec::new();
        let reader = BufReader::new(reader);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = parse_line(line) {
                // Keys must start with "key-"
                let Some(key_id) = key.strip_prefix(KEY_PREFIX) else {
                    continue;
                };

                // Normalize to lowercase for consistent lookups
                let key_id = key_id.to_ascii_lowercase();
                let key_value = value.to_ascii_lowercase();

                entries.push(KeyringEntry { key_id, key_value });
            }
        }

        Ok(Self { entries })
    }

    /// Build the config file content
    pub fn build(&self) -> Vec<u8> {
        let mut output = Vec::new();

        for entry in &self.entries {
            let _ = writeln!(
                output,
                "{}{} = {}",
                KEY_PREFIX, entry.key_id, entry.key_value
            );
        }

        output
    }

    /// Validate the keyring configuration
    pub fn validate(&self) -> Result<(), ValidationError> {
        for (i, entry) in self.entries.iter().enumerate() {
            // Key ID must be 16 hex characters (8 bytes)
            if entry.key_id.len() != 16 {
                return Err(ValidationError::IdLength {
                    index: i,
                    actual: entry.key_id.len(),
                });
            }
            if !entry.key_id.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ValidationError::IdFormat {
                    index: i,
                    value: entry.key_id.clone(),
                });
            }

            // Key value must be a valid 32-char hex string (16 bytes)
            if !is_valid_md5_hex(&entry.key_value) {
                return Err(ValidationError::KeyValue {
                    index: i,
                    key_id: entry.key_id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get all entries
    pub fn entries(&self) -> &[KeyringEntry] {
        &self.entries
    }

    /// Number of key entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the keyring is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a key by its 16-char hex key ID
    ///
    /// Returns the 32-char hex key value if found. The lookup is case-insensitive.
    pub fn get_key(&self, key_id: &str) -> Option<&str> {
        let key_id_lower = key_id.to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| e.key_id == key_id_lower)
            .map(|e| e.key_value.as_str())
    }

    /// Look up a key by its numeric u64 key ID
    ///
    /// Returns the 32-char hex key value if found.
    pub fn get_key_by_id(&self, id: u64) -> Option<&str> {
        let hex_id = format!("{id:016x}");
        self.get_key(&hex_id)
    }

    /// Add a key entry
    ///
    /// Both key_id and key_value are normalized to lowercase.
    pub fn add_entry(&mut self, key_id: impl Into<String>, key_value: impl Into<String>) {
        self.entries.push(KeyringEntry {
            key_id: key_id.into().to_ascii_lowercase(),
            key_value: key_value.into().to_ascii_lowercase(),
        });
    }
}

impl Default for KeyringConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyring config validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// Key ID has wrong length (expected 16 hex chars)
    #[error("entry {index}: key ID must be 16 hex chars, got {actual}")]
    IdLength { index: usize, actual: usize },
    /// Key ID contains non-hex characters
    #[error("entry {index}: key ID is not valid hex: {value}")]
    IdFormat { index: usize, value: String },
    /// Key value is not a valid 32-char hex string
    #[error("entry {index} (key {key_id}): key value must be 32 hex chars")]
    KeyValue { index: usize, key_id: String },
}

impl crate::CascFormat for KeyringConfig {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        Self::parse(data)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(self.build())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_entry() {
        let data = b"key-4eb4869f95f23b53 = c9316739348dcc033aa8112f9a3acf5d\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(config.len(), 1);
        assert_eq!(config.entries()[0].key_id, "4eb4869f95f23b53");
        assert_eq!(
            config.entries()[0].key_value,
            "c9316739348dcc033aa8112f9a3acf5d"
        );
    }

    #[test]
    fn test_parse_multiple_entries() {
        let data = b"key-1b3e4e1ecfb25877 = 3de60d37c664723595f27c5cdbf08bfa\n\
                      key-205901f51aabb942 = c68778823c964c6f247acc0f4a2584f8\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(config.len(), 2);
        assert_eq!(config.entries()[0].key_id, "1b3e4e1ecfb25877");
        assert_eq!(config.entries()[1].key_id, "205901f51aabb942");
    }

    #[test]
    fn test_parse_skips_comments_and_empty() {
        let data = b"# Keyring Config\n\
                      \n\
                      key-4eb4869f95f23b53 = c9316739348dcc033aa8112f9a3acf5d\n\
                      \n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(config.len(), 1);
    }

    #[test]
    fn test_parse_skips_non_key_lines() {
        let data = b"key-4eb4869f95f23b53 = c9316739348dcc033aa8112f9a3acf5d\n\
                      other-field = some_value\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(config.len(), 1);
    }

    #[test]
    fn test_parse_normalizes_case() {
        let data = b"key-4EB4869F95F23B53 = C9316739348DCC033AA8112F9A3ACF5D\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(config.entries()[0].key_id, "4eb4869f95f23b53");
        assert_eq!(
            config.entries()[0].key_value,
            "c9316739348dcc033aa8112f9a3acf5d"
        );
    }

    #[test]
    fn test_get_key() {
        let data = b"key-4eb4869f95f23b53 = c9316739348dcc033aa8112f9a3acf5d\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(
            config.get_key("4eb4869f95f23b53"),
            Some("c9316739348dcc033aa8112f9a3acf5d")
        );
        // Case-insensitive
        assert_eq!(
            config.get_key("4EB4869F95F23B53"),
            Some("c9316739348dcc033aa8112f9a3acf5d")
        );
        assert!(config.get_key("0000000000000000").is_none());
    }

    #[test]
    fn test_get_key_by_id() {
        let data = b"key-4eb4869f95f23b53 = c9316739348dcc033aa8112f9a3acf5d\n";
        let config = KeyringConfig::parse(&data[..]).unwrap();

        assert_eq!(
            config.get_key_by_id(0x4eb4869f95f23b53),
            Some("c9316739348dcc033aa8112f9a3acf5d")
        );
        assert!(config.get_key_by_id(0).is_none());
    }

    #[test]
    fn test_validate_valid() {
        let mut config = KeyringConfig::new();
        config.add_entry("4eb4869f95f23b53", "c9316739348dcc033aa8112f9a3acf5d");

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_empty() {
        let config = KeyringConfig::new();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_key_id_length() {
        let mut config = KeyringConfig::new();
        config.add_entry("short", "c9316739348dcc033aa8112f9a3acf5d");

        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("16 hex chars"));
    }

    #[test]
    fn test_validate_bad_key_value() {
        let mut config = KeyringConfig::new();
        config.add_entry("4eb4869f95f23b53", "tooshort");

        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("32 hex chars"));
    }

    #[test]
    fn test_round_trip() {
        let mut config = KeyringConfig::new();
        config.add_entry("4eb4869f95f23b53", "c9316739348dcc033aa8112f9a3acf5d");
        config.add_entry("1b3e4e1ecfb25877", "3de60d37c664723595f27c5cdbf08bfa");

        let built = config.build();
        let reparsed = KeyringConfig::parse(&built[..]).unwrap();

        assert_eq!(reparsed.len(), 2);
        assert_eq!(reparsed.entries()[0].key_id, "4eb4869f95f23b53");
        assert_eq!(reparsed.entries()[1].key_id, "1b3e4e1ecfb25877");
        assert_eq!(
            reparsed.entries()[0].key_value,
            "c9316739348dcc033aa8112f9a3acf5d"
        );
    }

    #[test]
    fn test_add_entry_normalizes() {
        let mut config = KeyringConfig::new();
        config.add_entry("4EB4869F95F23B53", "C9316739348DCC033AA8112F9A3ACF5D");

        assert_eq!(config.entries()[0].key_id, "4eb4869f95f23b53");
        assert_eq!(
            config.entries()[0].key_value,
            "c9316739348dcc033aa8112f9a3acf5d"
        );
    }

    #[test]
    fn test_is_empty() {
        let config = KeyringConfig::new();
        assert!(config.is_empty());

        let mut config2 = KeyringConfig::new();
        config2.add_entry("4eb4869f95f23b53", "c9316739348dcc033aa8112f9a3acf5d");
        assert!(!config2.is_empty());
    }
}
