//! TACT encryption key management
//!
//! This module manages encryption keys used for decrypting protected CASC content.
//! Keys are identified by their 64-bit key name (hash of the actual key name).

use std::collections::HashMap;
use std::fmt;

use crate::error::CryptoError;

/// A TACT encryption key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TactKey {
    /// Key identifier (hash of key name)
    pub id: u64,
    /// 16-byte encryption key
    pub key: [u8; 16],
}

impl TactKey {
    /// Create a new TACT key
    pub fn new(id: u64, key: [u8; 16]) -> Self {
        Self { id, key }
    }

    /// Parse key from hex string
    pub fn from_hex(id: u64, hex: &str) -> Result<Self, CryptoError> {
        let hex = hex.trim();
        let bytes = hex::decode(hex)
            .map_err(|e| CryptoError::InvalidKeyFormat(format!("invalid hex: {e}")))?;

        if bytes.len() != 16 {
            return Err(CryptoError::InvalidKeySize {
                expected: 16,
                actual: bytes.len(),
            });
        }

        let mut key = [0u8; 16];
        key.copy_from_slice(&bytes);
        Ok(Self::new(id, key))
    }
}

impl fmt::Display for TactKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016X}: {}", self.id, hex::encode_upper(self.key))
    }
}

/// Store for TACT encryption keys
#[derive(Debug, Clone)]
pub struct TactKeyStore {
    keys: HashMap<u64, [u8; 16]>,
}

impl TactKeyStore {
    /// Create a new key store with hardcoded keys
    pub fn new() -> Self {
        let mut store = Self {
            keys: HashMap::new(),
        };
        store.load_hardcoded_keys();
        store
    }

    /// Create an empty key store
    pub fn empty() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// Load hardcoded `WoW` encryption keys
    fn load_hardcoded_keys(&mut self) {
        // Battle for Azeroth
        self.add_key_from_hex(0xFA50_5078_126A_CB3E, "BDC51862ABED79B2DE48C8E7E66C6200");
        self.add_key_from_hex(0xFF81_3F7D_062A_C0BC, "AA0B5C77F088CCC2D39049BD267F066D");
        self.add_key_from_hex(0xD1E9_B5ED_F928_3668, "8E4A2579894E38B4AB9058BA5C7328EE");

        // Shadowlands
        self.add_key_from_hex(0xB767_2964_1141_CB34, "9849D1AA7B1FD09819C5C66283A326EC");
        self.add_key_from_hex(0xFFB9_469F_F16E_6BF8, "D514BD1909A9E5DC8703F4B8BB1DFD9A");

        // The War Within
        self.add_key_from_hex(0x0EBE_36B5_010D_FD7F, "9A89CC7E3ACB29CF14C60BC13B1E4616");

        // Classic
        self.add_key_from_hex(0xDEE3_A052_1EFF_6F03, "AD740CE3FFFF9231468126985708E1B9");

        // Additional well-known keys
        self.add_key_from_hex(0x4F0F_E18E_9FA1_AC1A, "89381C748F6531BBFCD97753D06CC3CD");
        self.add_key_from_hex(0x7758_B2CF_1E4E_3E1B, "3DE60D37C664723595F27C5CDBF08BFA");
        self.add_key_from_hex(0xE531_7801_B356_1125, "7D1E61BF5FD58346972365D53ACC66DC");
    }

    /// Add a key from hex string (internal helper)
    fn add_key_from_hex(&mut self, id: u64, hex: &str) {
        if let Ok(key) = TactKey::from_hex(id, hex) {
            self.keys.insert(key.id, key.key);
        }
    }

    /// Get a key by ID
    pub fn get(&self, id: u64) -> Option<&[u8; 16]> {
        self.keys.get(&id)
    }

    /// Add a key to the store
    pub fn add(&mut self, key: TactKey) {
        self.keys.insert(key.id, key.key);
    }

    /// Remove a key from the store
    pub fn remove(&mut self, id: u64) -> Option<[u8; 16]> {
        self.keys.remove(&id)
    }

    /// Get the number of keys in the store
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Load keys from CSV-formatted string content (format: `key_id,key_hex`)
    ///
    /// Lines starting with `#` are treated as comments.
    /// Returns the number of keys successfully loaded.
    ///
    /// # Example
    ///
    /// ```
    /// use cascette_crypto::keys::TactKeyStore;
    ///
    /// let csv_content = r#"
    /// # Comment line
    /// FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200
    /// 0xFF813F7D062AC0BC,AA0B5C77F088CCC2D39049BD267F066D
    /// "#;
    ///
    /// let mut store = TactKeyStore::empty();
    /// let count = store.load_from_csv(csv_content);
    /// assert_eq!(count, 2);
    /// ```
    pub fn load_from_csv(&mut self, content: &str) -> usize {
        let mut count = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() != 2 {
                continue;
            }

            if let Ok(id) = parse_key_id(parts[0].trim()) {
                let hex = parts[1].trim();
                if let Ok(key) = TactKey::from_hex(id, hex) {
                    self.add(key);
                    count += 1;
                }
            }
        }

        count
    }

    /// Load keys from text content (format: `key_id key_hex` per line)
    ///
    /// Lines starting with `#` or `//` are treated as comments.
    /// Returns the number of keys successfully loaded.
    ///
    /// # Example
    ///
    /// ```
    /// use cascette_crypto::keys::TactKeyStore;
    ///
    /// let txt_content = r#"
    /// # Comment line
    /// FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200
    /// 0xFF813F7D062AC0BC AA0B5C77F088CCC2D39049BD267F066D
    /// "#;
    ///
    /// let mut store = TactKeyStore::empty();
    /// let count = store.load_from_txt(txt_content);
    /// assert_eq!(count, 2);
    /// ```
    pub fn load_from_txt(&mut self, content: &str) -> usize {
        let mut count = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            if let Ok(id) = parse_key_id(parts[0]) {
                let hex = parts[1];
                if let Ok(key) = TactKey::from_hex(id, hex) {
                    self.add(key);
                    count += 1;
                }
            }
        }

        count
    }

    /// Iterate over all keys
    pub fn iter(&self) -> impl Iterator<Item = TactKey> + '_ {
        self.keys.iter().map(|(&id, &key)| TactKey::new(id, key))
    }
}

impl Default for TactKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a key ID from string (hex or decimal)
fn parse_key_id(s: &str) -> Result<u64, CryptoError> {
    let s = s.trim();

    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16)
            .map_err(|e| CryptoError::InvalidKeyFormat(format!("invalid hex key ID: {e}")))
    } else if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() == 16 {
        // Assume 16-char string is hex
        u64::from_str_radix(s, 16)
            .map_err(|e| CryptoError::InvalidKeyFormat(format!("invalid hex key ID: {e}")))
    } else {
        s.parse()
            .map_err(|e| CryptoError::InvalidKeyFormat(format!("invalid decimal key ID: {e}")))
    }
}

// TactKeyProvider implementation is in store_trait.rs to avoid circular dependency

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tact_key_from_hex() {
        let key = TactKey::from_hex(0x1234_5678_90AB_CDEF, "0123456789ABCDEF0123456789ABCDEF")
            .expect("Valid TACT key hex should parse");
        assert_eq!(key.id, 0x1234_5678_90AB_CDEF);
        assert_eq!(key.key[0], 0x01);
        assert_eq!(key.key[15], 0xEF);
    }

    #[test]
    fn test_tact_key_invalid_size() {
        let result = TactKey::from_hex(0x1234, "0123456789ABCDEF"); // Only 8 bytes
        assert!(matches!(result, Err(CryptoError::InvalidKeySize { .. })));
    }

    #[test]
    fn test_key_store_hardcoded() {
        let store = TactKeyStore::new();
        assert!(!store.is_empty());

        // Check a known key
        let key = store.get(0xFA50_5078_126A_CB3E);
        assert!(key.is_some());
    }

    #[test]
    fn test_key_store_operations() {
        let mut store = TactKeyStore::empty();
        assert_eq!(store.len(), 0);

        let key = TactKey::new(0x1234, [0x42; 16]);
        store.add(key);
        assert_eq!(store.len(), 1);

        let retrieved = store.get(0x1234);
        assert_eq!(retrieved, Some(&[0x42; 16]));

        store.remove(0x1234);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_parse_key_id() {
        assert_eq!(
            parse_key_id("0x1234").expect("Valid key ID should parse"),
            0x1234
        );
        assert_eq!(
            parse_key_id("0X1234").expect("Valid key ID should parse"),
            0x1234
        );
        assert_eq!(
            parse_key_id("DEADBEEF12345678").expect("Valid key ID should parse"),
            0xDEAD_BEEF_1234_5678
        );
        assert_eq!(
            parse_key_id("1000").expect("Valid key ID should parse"),
            1000
        );
    }
}
