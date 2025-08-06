//! Key management service for TACT encryption.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::error::CryptoError;
use crate::keys::{hardcoded_keys, parse_key_hex, parse_key_name};

/// Service for managing encryption keys.
pub struct KeyService {
    /// Map of key ID to encryption key.
    keys: HashMap<u64, [u8; 16]>,
}

impl KeyService {
    /// Create a new key service with hardcoded keys.
    pub fn new() -> Self {
        let keys = hardcoded_keys();
        info!("Loaded {} hardcoded encryption keys", keys.len());

        Self { keys }
    }

    /// Create a key service with no pre-loaded keys.
    pub fn empty() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// Get a key by ID.
    pub fn get_key(&self, key_id: u64) -> Option<&[u8; 16]> {
        self.keys.get(&key_id)
    }

    /// Add a key to the service.
    pub fn add_key(&mut self, key_id: u64, key: [u8; 16]) {
        self.keys.insert(key_id, key);
    }

    /// Get the number of keys in the service.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Load keys from a file.
    pub fn load_key_file(&mut self, path: &Path) -> Result<usize, CryptoError> {
        let content = fs::read_to_string(path)?;

        // Detect format based on file extension or content
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        match ext {
            "csv" => self.load_csv_keys(&content),
            "tsv" => self.load_tsv_keys(&content),
            "txt" => self.load_txt_keys(&content),
            _ => {
                // Try to auto-detect format
                if content.contains(',') {
                    self.load_csv_keys(&content)
                } else if content.contains('\t') {
                    self.load_tsv_keys(&content)
                } else {
                    self.load_txt_keys(&content)
                }
            }
        }
    }

    /// Load keys from CSV format (keyname,keyhex).
    fn load_csv_keys(&mut self, content: &str) -> Result<usize, CryptoError> {
        let mut loaded = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 {
                warn!("Skipping invalid CSV line {}: {}", line_num + 1, line);
                continue;
            }

            let key_name = parts[0].trim();
            let key_hex = parts[1].trim();

            match (parse_key_name(key_name), parse_key_hex(key_hex)) {
                (Ok(key_id), Ok(key)) => {
                    self.add_key(key_id, key);
                    loaded += 1;
                }
                (Err(e), _) => {
                    warn!("Failed to parse key name on line {}: {}", line_num + 1, e);
                }
                (_, Err(e)) => {
                    warn!("Failed to parse key hex on line {}: {}", line_num + 1, e);
                }
            }
        }

        info!("Loaded {} keys from CSV file", loaded);
        Ok(loaded)
    }

    /// Load keys from TSV format (keyname\tkeyhex).
    fn load_tsv_keys(&mut self, content: &str) -> Result<usize, CryptoError> {
        let mut loaded = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                warn!("Skipping invalid TSV line {}: {}", line_num + 1, line);
                continue;
            }

            let key_name = parts[0].trim();
            let key_hex = parts[1].trim();

            match (parse_key_name(key_name), parse_key_hex(key_hex)) {
                (Ok(key_id), Ok(key)) => {
                    self.add_key(key_id, key);
                    loaded += 1;
                }
                (Err(e), _) => {
                    warn!("Failed to parse key name on line {}: {}", line_num + 1, e);
                }
                (_, Err(e)) => {
                    warn!("Failed to parse key hex on line {}: {}", line_num + 1, e);
                }
            }
        }

        info!("Loaded {} keys from TSV file", loaded);
        Ok(loaded)
    }

    /// Load keys from TXT format (keyname keyhex [description]).
    fn load_txt_keys(&mut self, content: &str) -> Result<usize, CryptoError> {
        let mut loaded = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                warn!("Skipping invalid TXT line {}: {}", line_num + 1, line);
                continue;
            }

            let key_name = parts[0];
            let key_hex = parts[1];

            match (parse_key_name(key_name), parse_key_hex(key_hex)) {
                (Ok(key_id), Ok(key)) => {
                    self.add_key(key_id, key);
                    loaded += 1;
                }
                (Err(e), _) => {
                    warn!("Failed to parse key name on line {}: {}", line_num + 1, e);
                }
                (_, Err(e)) => {
                    warn!("Failed to parse key hex on line {}: {}", line_num + 1, e);
                }
            }
        }

        info!("Loaded {} keys from TXT file", loaded);
        Ok(loaded)
    }

    /// Load keys from standard directories.
    pub fn load_from_standard_dirs(&mut self) -> Result<usize, CryptoError> {
        let mut total_loaded = 0;

        // Check environment variable first
        if let Ok(path) = std::env::var("CASCETTE_KEYS_PATH") {
            let path = PathBuf::from(path);
            if path.exists() {
                if path.is_file() {
                    match self.load_key_file(&path) {
                        Ok(count) => {
                            total_loaded += count;
                            info!("Loaded {} keys from CASCETTE_KEYS_PATH", count);
                        }
                        Err(e) => {
                            warn!("Failed to load keys from CASCETTE_KEYS_PATH: {}", e);
                        }
                    }
                } else if path.is_dir() {
                    total_loaded += self.load_keys_from_dir(&path)?;
                }
            }
        }

        // Check home directory locations
        if let Some(home_dir) = dirs::home_dir() {
            // ~/.config/cascette/
            let config_dir = home_dir.join(".config").join("cascette");
            if config_dir.exists() {
                total_loaded += self.load_keys_from_dir(&config_dir)?;
            }

            // ~/.tactkeys/
            let tactkeys_dir = home_dir.join(".tactkeys");
            if tactkeys_dir.exists() {
                total_loaded += self.load_keys_from_dir(&tactkeys_dir)?;
            }
        }

        Ok(total_loaded)
    }

    /// Load all key files from a directory.
    fn load_keys_from_dir(&mut self, dir: &Path) -> Result<usize, CryptoError> {
        let mut total_loaded = 0;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Only load files with appropriate extensions
                if name.ends_with(".csv")
                    || name.ends_with(".tsv")
                    || name.ends_with(".txt")
                    || name.contains("key")
                {
                    match self.load_key_file(&path) {
                        Ok(count) => {
                            total_loaded += count;
                            debug!("Loaded {} keys from {:?}", count, path);
                        }
                        Err(e) => {
                            warn!("Failed to load keys from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(total_loaded)
    }
}

impl Default for KeyService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hardcoded_keys() {
        let service = KeyService::new();
        assert!(service.key_count() > 0);

        // Test a known key
        let key = service.get_key(0xFA505078126ACB3E);
        assert!(key.is_some());
    }

    #[test]
    fn test_add_key() {
        let mut service = KeyService::empty();
        let key_id = 0x1234567890ABCDEF;
        let key = [0u8; 16];

        service.add_key(key_id, key);
        assert_eq!(service.get_key(key_id), Some(&key));
    }

    #[test]
    fn test_load_csv() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "# Comment line")?;
        writeln!(file, "0x1234567890ABCDEF,00112233445566778899AABBCCDDEEFF")?;
        writeln!(file, "FEDCBA0987654321,FFEEDDCCBBAA99887766554433221100")?;

        let mut service = KeyService::empty();
        let loaded = service.load_key_file(file.path())?;
        assert_eq!(loaded, 2);

        assert!(service.get_key(0x1234567890ABCDEF).is_some());
        assert!(service.get_key(0xFEDCBA0987654321).is_some());

        Ok(())
    }

    #[test]
    fn test_load_txt() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "# Comment line")?;
        writeln!(
            file,
            "0x1234567890ABCDEF 00112233445566778899AABBCCDDEEFF Some description"
        )?;
        writeln!(file, "FEDCBA0987654321 FFEEDDCCBBAA99887766554433221100")?;

        let mut service = KeyService::empty();
        let loaded = service.load_key_file(file.path())?;
        assert_eq!(loaded, 2);

        assert!(service.get_key(0x1234567890ABCDEF).is_some());
        assert!(service.get_key(0xFEDCBA0987654321).is_some());

        Ok(())
    }
}
