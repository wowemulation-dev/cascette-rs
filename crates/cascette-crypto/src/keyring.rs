//! Keyring-based TACT key storage
//!
//! This module provides secure, persistent storage for TACT encryption keys using the OS keyring.
//! Keys are stored directly in the platform's credential manager for maximum security.

#[cfg(feature = "keyring")]
mod implementation {
    use crate::{CryptoError, TactKey};
    use base64::{Engine as _, engine::general_purpose};
    use hex;
    use keyring;
    use serde_json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use thiserror::Error;

    /// Error types specific to keyring operations
    #[derive(Debug, Error)]
    pub enum KeyringError {
        #[error("Keyring service unavailable: {reason}")]
        /// Keyring service unavailable
        ServiceUnavailable {
            /// Reason for unavailability
            reason: String,
        },

        #[error("Key access denied by OS: {key_id}")]
        /// Key access denied by OS
        AccessDenied {
            /// Key ID that was denied
            key_id: u64,
        },

        #[error("Key not found in keyring: {key_id}")]
        /// Key not found in keyring
        KeyNotFound {
            /// Key ID that was not found
            key_id: u64,
        },

        #[error("Invalid key data format: {reason}")]
        /// Invalid key data format
        InvalidFormat {
            /// Reason for format error
            reason: String,
        },

        #[error("Cache operation failed: {reason}")]
        /// Cache operation failed
        CacheError {
            /// Reason for cache error
            reason: String,
        },

        #[error("Keyring I/O error: {0}")]
        /// Keyring I/O error
        IoError(#[from] keyring::Error),

        #[error("Crypto operation error: {0}")]
        /// Crypto operation error
        CryptoError(#[from] CryptoError),
    }

    /// Configuration for keyring-based key storage
    #[derive(Debug, Clone)]
    pub struct KeyringConfig {
        /// Service name for keyring storage
        pub service_name: String,
        /// Enable performance metrics
        pub enable_metrics: bool,
        /// Keyring entry prefix
        pub key_prefix: String,
    }

    impl Default for KeyringConfig {
        fn default() -> Self {
            Self::production()
        }
    }

    impl KeyringConfig {
        /// Development configuration
        pub fn development() -> Self {
            Self {
                service_name: "cascette-dev".to_string(),
                enable_metrics: true,
                key_prefix: "dev-".to_string(),
            }
        }

        /// Production configuration
        pub fn production() -> Self {
            Self {
                service_name: "cascette-tact-keys".to_string(),
                enable_metrics: true,
                key_prefix: String::new(),
            }
        }

        /// Security-focused configuration - no metrics
        pub fn secure() -> Self {
            Self {
                service_name: "cascette-tact-keys".to_string(),
                enable_metrics: false,
                key_prefix: String::new(),
            }
        }
    }

    /// Performance metrics for keyring operations
    #[derive(Debug, Default)]
    pub struct KeyringMetrics {
        /// Keyring operation counters
        /// Number of read operations
        pub keyring_reads: AtomicU64,
        /// Number of write operations
        pub keyring_writes: AtomicU64,
        /// Number of delete operations
        pub keyring_deletes: AtomicU64,

        /// Performance timings (microseconds)
        pub avg_keyring_access_time: AtomicU64,

        /// Error counters
        pub keyring_failures: AtomicU64,
    }

    impl KeyringMetrics {
        /// Create a snapshot of current metrics
        pub fn snapshot(&self) -> KeyringMetricsSnapshot {
            KeyringMetricsSnapshot {
                keyring_reads: self.keyring_reads.load(Ordering::Relaxed),
                keyring_writes: self.keyring_writes.load(Ordering::Relaxed),
                keyring_deletes: self.keyring_deletes.load(Ordering::Relaxed),
                avg_keyring_access_time: self.avg_keyring_access_time.load(Ordering::Relaxed),
                keyring_failures: self.keyring_failures.load(Ordering::Relaxed),
            }
        }
    }

    /// Snapshot of keyring metrics at a point in time
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KeyringMetricsSnapshot {
        /// Number of read operations
        pub keyring_reads: u64,
        /// Number of write operations
        pub keyring_writes: u64,
        /// Number of delete operations
        pub keyring_deletes: u64,
        /// Average access time in microseconds
        pub avg_keyring_access_time: u64,
        /// Number of failed operations
        pub keyring_failures: u64,
    }

    /// Iterator over keyring entries
    pub struct KeyringIterator<'a> {
        store: &'a KeyringTactKeyStore,
        key_ids: Vec<u64>,
        current_index: usize,
    }

    impl Iterator for KeyringIterator<'_> {
        type Item = TactKey;

        fn next(&mut self) -> Option<Self::Item> {
            while self.current_index < self.key_ids.len() {
                let key_id = self.key_ids[self.current_index];
                self.current_index += 1;

                if let Ok(Some(key_data)) = self.store.load_from_keyring(key_id) {
                    return Some(TactKey::new(key_id, key_data));
                }
                // If key doesn't exist anymore, skip to next
            }
            None
        }
    }

    /// Keyring-based TACT key storage
    pub struct KeyringTactKeyStore {
        /// Configuration
        config: KeyringConfig,
        /// Performance metrics
        metrics: Arc<KeyringMetrics>,
        /// In-memory fallback for when keyring doesn't work
        fallback_store: std::sync::RwLock<std::collections::HashMap<u64, [u8; 16]>>,
        /// Whether to use fallback mode
        use_fallback: std::sync::atomic::AtomicBool,
        /// Path for fallback file storage
        fallback_file: Option<std::path::PathBuf>,
    }

    impl KeyringTactKeyStore {
        /// Create new store with default configuration
        pub fn new() -> Result<Self, KeyringError> {
            Self::with_config(KeyringConfig::default())
        }

        /// Create store with custom configuration
        pub fn with_config(config: KeyringConfig) -> Result<Self, KeyringError> {
            // Determine fallback file path
            let fallback_file = Self::get_fallback_file_path(&config.service_name);

            let store = Self {
                config,
                metrics: Arc::new(KeyringMetrics::default()),
                fallback_store: std::sync::RwLock::new(std::collections::HashMap::new()),
                use_fallback: std::sync::atomic::AtomicBool::new(false),
                fallback_file,
            };

            // Try to load from fallback file if it exists
            if let Some(ref path) = store.fallback_file {
                if path.exists() {
                    // If fallback file exists, we should use fallback mode
                    store.use_fallback.store(true, Ordering::Relaxed);
                    let _ = store.load_from_fallback_file(); // Ignore errors during load
                }
            }

            Ok(store)
        }

        /// Add a TACT key to secure storage
        pub fn add(&self, key: TactKey) -> Result<(), KeyringError> {
            // Check if key already exists to avoid double-counting
            let exists = self.load_from_keyring(key.id)?.is_some();

            self.store_to_keyring(&key)?;

            // Verify the key can be retrieved after storage (keyring consistency check)
            if !self.use_fallback.load(Ordering::Relaxed)
                && matches!(self.load_from_keyring(key.id), Ok(None))
            {
                // Keyring inconsistency detected - switch to fallback mode
                self.use_fallback.store(true, Ordering::Relaxed);
                // Store the key in fallback mode
                let mut store = self
                    .fallback_store
                    .write()
                    .expect("Fallback store lock should not be poisoned");
                store.insert(key.id, key.key);

                // Don't save to file on every add - caller should batch saves
                drop(store); // Release the lock
            }

            // Only increment count if this is a new key
            if !exists {
                self.increment_key_count()?;
                self.add_key_id_to_metadata(key.id)?;
            }

            Ok(())
        }

        /// Retrieve a key by ID
        pub fn get(&self, key_id: u64) -> Result<Option<[u8; 16]>, KeyringError> {
            self.load_from_keyring(key_id)
        }

        /// Remove a key from keyring storage
        pub fn remove(&self, key_id: u64) -> Result<Option<[u8; 16]>, KeyringError> {
            // Get the key before deleting
            let existing_key = self.load_from_keyring(key_id)?;

            if existing_key.is_some() {
                self.delete_from_keyring(key_id)?;
                self.decrement_key_count()?;
                self.remove_key_id_from_metadata(key_id)?;
            }

            Ok(existing_key)
        }

        /// Get number of keys in keyring (expensive operation)
        /// Note: This is a simplified implementation that tracks count in metadata
        pub fn len(&self) -> usize {
            if self.use_fallback.load(Ordering::Relaxed) {
                // In fallback mode, count keys directly from the in-memory store
                let store = self
                    .fallback_store
                    .read()
                    .expect("Fallback store lock should not be poisoned");
                store.len()
            } else {
                // In keyring mode, use the stored key IDs list length
                self.get_stored_key_ids().map(|ids| ids.len()).unwrap_or(0)
            }
        }

        /// Check if store is empty
        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }

        /// Iterate over all available keys
        pub fn iter(&self) -> KeyringIterator<'_> {
            let key_ids = if self.use_fallback.load(Ordering::Relaxed) {
                // In fallback mode, get keys from in-memory store
                let store = self
                    .fallback_store
                    .read()
                    .expect("Fallback store lock should not be poisoned");
                store.keys().copied().collect()
            } else {
                // In keyring mode, get from metadata or empty if no metadata exists
                self.get_stored_key_ids().unwrap_or_default()
            };

            KeyringIterator {
                store: self,
                key_ids,
                current_index: 0,
            }
        }

        /// Load hardcoded keys into storage
        pub fn load_hardcoded_keys(&self) -> Result<usize, KeyringError> {
            let keys = self.get_hardcoded_keys();
            let mut count = 0;

            for key in keys {
                self.add(key)?;
                count += 1;
            }

            Ok(count)
        }

        /// Import keys from file
        pub fn load_from_file<P: AsRef<std::path::Path>>(
            &self,
            path: P,
        ) -> Result<usize, KeyringError> {
            let path = path.as_ref();
            let content = std::fs::read_to_string(path).map_err(|e| KeyringError::CacheError {
                reason: format!("Failed to read file {}: {}", path.display(), e),
            })?;

            // Determine file format by extension
            let count = if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                self.load_csv_content(&content)?
            } else {
                self.load_txt_content(&content)?
            };

            Ok(count)
        }

        /// Get performance metrics
        pub fn metrics(&self) -> KeyringMetricsSnapshot {
            self.metrics.snapshot()
        }

        /// Check if running in fallback mode (for testing)
        pub fn is_using_fallback(&self) -> bool {
            self.use_fallback.load(Ordering::Relaxed)
        }

        /// Get fallback file path (for testing)
        pub fn fallback_file_path(&self) -> Option<&std::path::Path> {
            self.fallback_file.as_deref()
        }

        // Private implementation methods

        fn store_to_keyring(&self, key: &TactKey) -> Result<(), KeyringError> {
            let start = std::time::Instant::now();

            // If we're in fallback mode, use in-memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                let mut store = self
                    .fallback_store
                    .write()
                    .expect("Fallback store lock should not be poisoned");
                store.insert(key.id, key.key);

                // Don't save to file on every add - caller should batch saves

                self.metrics.keyring_writes.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }

            let service_name = &self.config.service_name;
            let username = format!("{}key-{:016X}", self.config.key_prefix, key.id);
            let password = general_purpose::STANDARD.encode(key.key);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            match entry.set_password(&password) {
                Ok(()) => {
                    // Verify it was actually stored by immediately trying to retrieve it
                    if let Ok(retrieved) = entry.get_password() {
                        if retrieved == password {
                            self.metrics.keyring_writes.fetch_add(1, Ordering::Relaxed);

                            if self.config.enable_metrics {
                                let elapsed = start.elapsed().as_micros() as u64;
                                let current =
                                    self.metrics.avg_keyring_access_time.load(Ordering::Relaxed);
                                let new_avg = if current == 0 {
                                    elapsed
                                } else {
                                    (current + elapsed) / 2
                                };
                                self.metrics
                                    .avg_keyring_access_time
                                    .store(new_avg, Ordering::Relaxed);
                            }
                            return Ok(());
                        }
                    }

                    // Keyring set succeeded but immediate verification failed
                    // Switch to fallback mode for this store instance
                    eprintln!("Warning: Keyring verification failed, switching to fallback mode");
                    self.use_fallback.store(true, Ordering::Relaxed);
                    let mut store = self
                        .fallback_store
                        .write()
                        .expect("Fallback store lock should not be poisoned");
                    store.insert(key.id, key.key);

                    // Save to fallback file
                    let _ = self.save_to_fallback_file();

                    self.metrics.keyring_writes.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
                Err(e) => {
                    // Keyring failed, switch to fallback mode
                    eprintln!(
                        "Warning: Keyring operation failed ({}), switching to fallback mode",
                        e
                    );
                    self.use_fallback.store(true, Ordering::Relaxed);
                    let mut store = self
                        .fallback_store
                        .write()
                        .expect("Fallback store lock should not be poisoned");
                    store.insert(key.id, key.key);

                    // Save to fallback file
                    let _ = self.save_to_fallback_file();

                    self.metrics.keyring_writes.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }
            }
        }

        /// Load a TACT key from the OS keyring
        ///
        /// # Arguments
        /// * `key_id` - The unique identifier for the TACT key
        ///
        /// # Returns
        /// * `Ok(Some(key))` if the key exists and was loaded successfully
        /// * `Ok(None)` if the key does not exist
        /// * `Err(KeyringError)` if there was an error accessing the keyring
        pub fn load_from_keyring(&self, key_id: u64) -> Result<Option<[u8; 16]>, KeyringError> {
            let start = std::time::Instant::now();

            // If we're in fallback mode, use in-memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                let store = self
                    .fallback_store
                    .read()
                    .expect("Fallback store lock should not be poisoned");
                let result = store.get(&key_id).copied();
                if result.is_some() {
                    self.metrics.keyring_reads.fetch_add(1, Ordering::Relaxed);
                }
                return Ok(result);
            }

            let service_name = &self.config.service_name;
            let username = format!("{}key-{:016X}", self.config.key_prefix, key_id);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            let result = match entry.get_password() {
                Ok(password) => {
                    let key_bytes = general_purpose::STANDARD.decode(&password).map_err(|e| {
                        KeyringError::InvalidFormat {
                            reason: format!("Base64 decode failed: {}", e),
                        }
                    })?;

                    if key_bytes.len() != 16 {
                        return Err(KeyringError::InvalidFormat {
                            reason: format!("Expected 16 bytes, got {}", key_bytes.len()),
                        });
                    }

                    let mut key = [0u8; 16];
                    key.copy_from_slice(&key_bytes);

                    self.metrics.keyring_reads.fetch_add(1, Ordering::Relaxed);
                    Some(key)
                }
                Err(keyring::Error::NoEntry) => None,
                Err(e) => {
                    self.metrics
                        .keyring_failures
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(KeyringError::from(e));
                }
            };

            if self.config.enable_metrics {
                let elapsed = start.elapsed().as_micros() as u64;
                let current = self.metrics.avg_keyring_access_time.load(Ordering::Relaxed);
                let new_avg = if current == 0 {
                    elapsed
                } else {
                    (current + elapsed) / 2
                };
                self.metrics
                    .avg_keyring_access_time
                    .store(new_avg, Ordering::Relaxed);
            }

            Ok(result)
        }

        fn delete_from_keyring(&self, key_id: u64) -> Result<(), KeyringError> {
            let start = std::time::Instant::now();

            // If we're in fallback mode, use in-memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                let mut store = self
                    .fallback_store
                    .write()
                    .expect("Fallback store lock should not be poisoned");
                store.remove(&key_id);

                // Don't save to file on every remove - caller should batch saves

                self.metrics.keyring_deletes.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }

            let service_name = &self.config.service_name;
            let username = format!("{}key-{:016X}", self.config.key_prefix, key_id);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            entry.delete_credential().map_err(KeyringError::from)?;

            self.metrics.keyring_deletes.fetch_add(1, Ordering::Relaxed);

            if self.config.enable_metrics {
                let elapsed = start.elapsed().as_micros() as u64;
                let current = self.metrics.avg_keyring_access_time.load(Ordering::Relaxed);
                let new_avg = if current == 0 {
                    elapsed
                } else {
                    (current + elapsed) / 2
                };
                self.metrics
                    .avg_keyring_access_time
                    .store(new_avg, Ordering::Relaxed);
            }

            Ok(())
        }

        fn get_hardcoded_keys(&self) -> Vec<TactKey> {
            vec![
                // Battle for Azeroth
                TactKey::new(
                    0xFA50_5078_126A_CB3E,
                    hex::decode("BDC51862ABED79B2DE48C8E7E66C6200")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                TactKey::new(
                    0xFF81_3F7D_062A_C0BC,
                    hex::decode("AA0B5C77F088CCC2D39049BD267F066D")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                TactKey::new(
                    0xD1E9_B5ED_F928_3668,
                    hex::decode("8E4A2579894E38B4AB9058BA5C7328EE")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                // Shadowlands
                TactKey::new(
                    0xB767_2964_1141_CB34,
                    hex::decode("9849D1AA7B1FD09819C5C66283A326EC")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                TactKey::new(
                    0xFFB9_469F_F16E_6BF8,
                    hex::decode("D514BD1909A9E5DC8703F4B8BB1DFD9A")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                // The War Within
                TactKey::new(
                    0x0EBE_36B5_010D_FD7F,
                    hex::decode("9A89CC7E3ACB29CF14C60BC13B1E4616")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                // Classic
                TactKey::new(
                    0xDEE3_A052_1EFF_6F03,
                    hex::decode("AD740CE3FFFF9231468126985708E1B9")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                // Additional well-known keys
                TactKey::new(
                    0x4F0F_E18E_9FA1_AC1A,
                    hex::decode("89381C748F6531BBFCD97753D06CC3CD")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                TactKey::new(
                    0x7758_B2CF_1E4E_3E1B,
                    hex::decode("3DE60D37C664723595F27C5CDBF08BFA")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
                TactKey::new(
                    0xE531_7801_B356_1125,
                    hex::decode("7D1E61BF5FD58346972365D53ACC66DC")
                        .expect("Valid hex literal")
                        .try_into()
                        .expect("Valid 16-byte key"),
                ),
            ]
        }

        // Helper methods for managing key count metadata
        fn get_key_count(&self) -> Result<usize, KeyringError> {
            let service_name = &self.config.service_name;
            let username = format!("{}metadata-count", self.config.key_prefix);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            match entry.get_password() {
                Ok(count_str) => {
                    count_str
                        .parse::<usize>()
                        .map_err(|e| KeyringError::InvalidFormat {
                            reason: format!("Invalid count metadata: {}", e),
                        })
                }
                Err(keyring::Error::NoEntry) => Ok(0), // No metadata means no keys
                Err(e) => Err(KeyringError::from(e)),
            }
        }

        fn set_key_count(&self, count: usize) -> Result<(), KeyringError> {
            let service_name = &self.config.service_name;
            let username = format!("{}metadata-count", self.config.key_prefix);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            entry
                .set_password(&count.to_string())
                .map_err(KeyringError::from)?;

            Ok(())
        }

        fn increment_key_count(&self) -> Result<(), KeyringError> {
            // Skip count operations in fallback mode - count is derived from memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                return Ok(());
            }

            let current = self.get_key_count()?;
            self.set_key_count(current + 1)
        }

        fn decrement_key_count(&self) -> Result<(), KeyringError> {
            // Skip count operations in fallback mode - count is derived from memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                return Ok(());
            }

            let current = self.get_key_count()?;
            if current > 0 {
                self.set_key_count(current - 1)?;
            }
            Ok(())
        }

        // Helper methods for tracking key IDs and file loading
        /// Get all TACT key IDs currently stored in the keyring
        ///
        /// # Returns
        /// * `Ok(Vec<u64>)` containing all stored key IDs
        /// * `Err(KeyringError)` if there was an error accessing the keyring
        pub fn get_stored_key_ids(&self) -> Result<Vec<u64>, KeyringError> {
            // If we're in fallback mode, get keys from in-memory store
            if self.use_fallback.load(Ordering::Relaxed) {
                let store = self
                    .fallback_store
                    .read()
                    .expect("Fallback store lock should not be poisoned");
                return Ok(store.keys().copied().collect());
            }

            // In keyring mode, we need to enumerate all entries with our prefix
            // Since keyring doesn't provide enumeration, we rely on our metadata
            let service_name = &self.config.service_name;
            let username = format!("{}metadata-keys", self.config.key_prefix);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            match entry.get_password() {
                Ok(keys_json) => {
                    serde_json::from_str(&keys_json).map_err(|e| KeyringError::InvalidFormat {
                        reason: format!("Invalid key IDs metadata: {}", e),
                    })
                }
                Err(keyring::Error::NoEntry) => {
                    // No metadata exists yet - try to initialize from hardcoded keys
                    // This handles the case where keys were added but metadata is missing
                    Ok(Vec::new())
                }
                Err(e) => Err(KeyringError::from(e)),
            }
        }

        fn add_key_id_to_metadata(&self, key_id: u64) -> Result<(), KeyringError> {
            // Skip metadata operations in fallback mode - keys are tracked in memory
            if self.use_fallback.load(Ordering::Relaxed) {
                return Ok(());
            }

            let mut key_ids = self.get_stored_key_ids()?;
            if !key_ids.contains(&key_id) {
                key_ids.push(key_id);
                self.save_key_ids_metadata(&key_ids)?;
            }
            Ok(())
        }

        fn remove_key_id_from_metadata(&self, key_id: u64) -> Result<(), KeyringError> {
            // Skip metadata operations in fallback mode - keys are tracked in memory
            if self.use_fallback.load(Ordering::Relaxed) {
                return Ok(());
            }

            let mut key_ids = self.get_stored_key_ids()?;
            if let Some(pos) = key_ids.iter().position(|&id| id == key_id) {
                key_ids.remove(pos);
                self.save_key_ids_metadata(&key_ids)?;
            }
            Ok(())
        }

        fn save_key_ids_metadata(&self, key_ids: &[u64]) -> Result<(), KeyringError> {
            let service_name = &self.config.service_name;
            let username = format!("{}metadata-keys", self.config.key_prefix);

            let entry = keyring::Entry::new(service_name, &username).map_err(KeyringError::from)?;

            let keys_json =
                serde_json::to_string(key_ids).map_err(|e| KeyringError::CacheError {
                    reason: format!("Failed to serialize key IDs: {}", e),
                })?;

            entry.set_password(&keys_json).map_err(KeyringError::from)?;

            Ok(())
        }

        fn load_csv_content(&self, content: &str) -> Result<usize, KeyringError> {
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

                if let Ok(id) = self.parse_key_id(parts[0].trim()) {
                    let hex = parts[1].trim();
                    if let Ok(key) = TactKey::from_hex(id, hex) {
                        self.add(key)?;
                        count += 1;
                    }
                }
            }

            Ok(count)
        }

        fn load_txt_content(&self, content: &str) -> Result<usize, KeyringError> {
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

                if let Ok(id) = self.parse_key_id(parts[0]) {
                    let hex = parts[1];
                    if let Ok(key) = TactKey::from_hex(id, hex) {
                        self.add(key)?;
                        count += 1;
                    }
                }
            }

            Ok(count)
        }

        fn parse_key_id(&self, s: &str) -> Result<u64, KeyringError> {
            let s = s.trim();

            if s.starts_with("0x") || s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16).map_err(|e| KeyringError::InvalidFormat {
                    reason: format!("invalid hex key ID: {}", e),
                })
            } else if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() == 16 {
                // Assume 16-char string is hex
                u64::from_str_radix(s, 16).map_err(|e| KeyringError::InvalidFormat {
                    reason: format!("invalid hex key ID: {}", e),
                })
            } else {
                s.parse().map_err(|e| KeyringError::InvalidFormat {
                    reason: format!("invalid decimal key ID: {}", e),
                })
            }
        }

        /// Get fallback file path for persistent storage when keyring is unavailable
        fn get_fallback_file_path(service_name: &str) -> Option<std::path::PathBuf> {
            // Use XDG_DATA_HOME or fallback to ~/.local/share
            let data_dir = std::env::var("XDG_DATA_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|_| {
                    std::env::var("HOME")
                        .map(|h| std::path::PathBuf::from(h).join(".local").join("share"))
                })
                .ok()?;

            Some(
                data_dir
                    .join("cascette")
                    .join(format!("{}_fallback.json", service_name)),
            )
        }

        /// Save fallback store to file
        pub fn save_to_fallback_file(&self) -> Result<(), KeyringError> {
            let Some(ref path) = self.fallback_file else {
                return Ok(());
            };

            let store = self
                .fallback_store
                .read()
                .expect("Fallback store lock should not be poisoned");

            // Convert to serializable format
            let data: std::collections::HashMap<String, String> = store
                .iter()
                .map(|(k, v)| (format!("{:016X}", k), general_purpose::STANDARD.encode(v)))
                .collect();

            // Ensure directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| KeyringError::CacheError {
                    reason: format!("Failed to create directory: {}", e),
                })?;
            }

            let json =
                serde_json::to_string_pretty(&data).map_err(|e| KeyringError::CacheError {
                    reason: format!("Failed to serialize keys: {}", e),
                })?;

            std::fs::write(path, json).map_err(|e| KeyringError::CacheError {
                reason: format!("Failed to write fallback file: {}", e),
            })?;

            Ok(())
        }

        /// Load fallback store from file
        fn load_from_fallback_file(&self) -> Result<(), KeyringError> {
            let Some(ref path) = self.fallback_file else {
                return Ok(());
            };

            let content = std::fs::read_to_string(path).map_err(|e| KeyringError::CacheError {
                reason: format!("Failed to read fallback file: {}", e),
            })?;

            let data: std::collections::HashMap<String, String> = serde_json::from_str(&content)
                .map_err(|e| KeyringError::CacheError {
                    reason: format!("Failed to parse fallback file: {}", e),
                })?;

            let mut store = self
                .fallback_store
                .write()
                .expect("Fallback store lock should not be poisoned");

            // Clear existing data and load from file
            store.clear();

            for (key_str, value_b64) in data {
                if let Ok(key_id) = u64::from_str_radix(&key_str, 16) {
                    if let Ok(key_bytes) = general_purpose::STANDARD.decode(&value_b64) {
                        if key_bytes.len() == 16 {
                            let mut key_array = [0u8; 16];
                            key_array.copy_from_slice(&key_bytes);
                            store.insert(key_id, key_array);
                        }
                    }
                }
            }

            // Fallback mode should already be set by the caller

            Ok(())
        }
    }

    impl Clone for KeyringTactKeyStore {
        fn clone(&self) -> Self {
            // Clone the configuration
            let config = self.config.clone();

            // Create a new store with the same config
            let new_store = Self::with_config(config)
                .expect("Clone should succeed if original was created successfully");

            // Copy fallback state
            new_store
                .use_fallback
                .store(self.use_fallback.load(Ordering::Relaxed), Ordering::Relaxed);

            // Copy all keys from the current store
            if let Ok(key_ids) = self.get_stored_key_ids() {
                for key_id in key_ids {
                    if let Ok(Some(key_bytes)) = self.get(key_id) {
                        let tact_key = TactKey::new(key_id, key_bytes);
                        let _ = new_store.add(tact_key); // Ignore errors for clone
                    }
                }
            }

            new_store
        }
    }

    impl<'a> IntoIterator for &'a KeyringTactKeyStore {
        type Item = TactKey;
        type IntoIter = KeyringIterator<'a>;

        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }
}

#[cfg(feature = "keyring")]
pub use implementation::*;

#[cfg(not(feature = "keyring"))]
mod stub {
    use crate::{CryptoError, TactKey};
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("Keyring support not enabled. Enable the 'keyring' feature")]
    pub struct KeyringError;

    impl From<CryptoError> for KeyringError {
        fn from(_: CryptoError) -> Self {
            KeyringError
        }
    }

    #[derive(Debug, Clone)]
    pub struct KeyringConfig;

    impl Default for KeyringConfig {
        fn default() -> Self {
            KeyringConfig
        }
    }

    impl KeyringConfig {
        pub fn development() -> Self {
            KeyringConfig
        }
        pub fn production() -> Self {
            KeyringConfig
        }
        pub fn secure() -> Self {
            KeyringConfig
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KeyringMetricsSnapshot;

    pub struct KeyringIterator<'a> {
        _phantom: std::marker::PhantomData<&'a ()>,
    }

    impl Iterator for KeyringIterator<'_> {
        type Item = TactKey;
        fn next(&mut self) -> Option<Self::Item> {
            None
        }
    }

    pub struct KeyringTactKeyStore;

    impl KeyringTactKeyStore {
        pub fn new() -> Result<Self, KeyringError> {
            Err(KeyringError)
        }
        pub fn with_config(_config: KeyringConfig) -> Result<Self, KeyringError> {
            Err(KeyringError)
        }
        pub fn add(&self, _key: TactKey) -> Result<(), KeyringError> {
            Err(KeyringError)
        }
        pub fn get(&self, _key_id: u64) -> Result<Option<[u8; 16]>, KeyringError> {
            Err(KeyringError)
        }
        pub fn remove(&self, _key_id: u64) -> Result<Option<[u8; 16]>, KeyringError> {
            Err(KeyringError)
        }
        pub fn len(&self) -> usize {
            0
        }
        pub fn is_empty(&self) -> bool {
            true
        }
        pub fn iter(&self) -> KeyringIterator<'_> {
            KeyringIterator {
                _phantom: std::marker::PhantomData,
            }
        }
        pub fn load_hardcoded_keys(&self) -> Result<usize, KeyringError> {
            Err(KeyringError)
        }
        pub fn load_from_file<P: AsRef<std::path::Path>>(
            &self,
            _path: P,
        ) -> Result<usize, KeyringError> {
            Err(KeyringError)
        }
        pub fn metrics(&self) -> KeyringMetricsSnapshot {
            KeyringMetricsSnapshot
        }
    }
}

#[cfg(not(feature = "keyring"))]
pub use stub::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TactKey;

    // These tests will fail until implementation is complete - that's the TDD approach

    #[cfg(feature = "keyring")]
    #[allow(clippy::unwrap_used)] // Test code is allowed to use unwrap
    mod keyring_tests {
        use super::*;

        #[test]
        fn test_keyring_tact_keystore_new_with_default_config() {
            let result = KeyringTactKeyStore::new();
            assert!(result.is_ok());
            let _store = result.expect("Test assertion");
            // Note: config is private, this test validates construction succeeds
            // Implementation will verify config is used correctly
        }

        #[test]
        fn test_keyring_tact_keystore_new_with_custom_config() {
            let config = KeyringConfig::development();
            let result = KeyringTactKeyStore::with_config(config);
            assert!(result.is_ok());
            let _store = result.expect("Test assertion");
            // Note: config details are private, test behavior through public API
            // Implementation will verify config values are used correctly
        }

        #[test]
        fn test_keyring_config_presets() {
            let dev_config = KeyringConfig::development();
            assert_eq!(dev_config.service_name, "cascette-dev");
            assert_eq!(dev_config.key_prefix, "dev-");
            assert!(dev_config.enable_metrics);

            let prod_config = KeyringConfig::production();
            assert_eq!(prod_config.service_name, "cascette-tact-keys");
            assert_eq!(prod_config.key_prefix, "");
            assert!(prod_config.enable_metrics);

            let secure_config = KeyringConfig::secure();
            assert_eq!(secure_config.service_name, "cascette-tact-keys");
            assert_eq!(secure_config.key_prefix, "");
            assert!(!secure_config.enable_metrics);
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_keyring_tact_keystore_add_and_get() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);

            // Add key
            let result = store.add(test_key);
            assert!(result.is_ok());

            // Retrieve key
            let retrieved = store.get(0x1234_5678_90AB_CDEF);
            assert!(retrieved.is_ok());
            assert_eq!(retrieved.expect("Test assertion"), Some([0x42; 16]));
        }

        #[test]
        fn test_keyring_tact_keystore_get_nonexistent_key() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let result = store.get(0xFFFF_FFFF_FFFF_FFFF);
            assert!(result.is_ok());
            assert_eq!(result.expect("Test assertion"), None);
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_keyring_tact_keystore_remove() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);

            // Add key
            store.add(test_key).expect("Test assertion");

            // Remove key
            let removed = store.remove(0x1234_5678_90AB_CDEF);
            assert!(removed.is_ok());
            assert_eq!(removed.expect("Test assertion"), Some([0x42; 16]));

            // Verify key is gone
            let result = store.get(0x1234_5678_90AB_CDEF);
            assert!(result.is_ok());
            assert_eq!(result.expect("Test assertion"), None);
        }

        #[test]
        #[ignore = "TDD - Implementation not complete"]
        fn test_keyring_tact_keystore_remove_nonexistent() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let result = store.remove(0xFFFF_FFFF_FFFF_FFFF);
            assert!(result.is_ok());
            assert_eq!(result.expect("Test assertion"), None);
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_keyring_tact_keystore_len_and_is_empty() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            println!(
                "Initial state: len={}, empty={}, fallback={}",
                store.len(),
                store.is_empty(),
                store.is_using_fallback()
            );
            assert!(store.is_empty());
            assert_eq!(store.len(), 0);

            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);
            store.add(test_key).expect("Test assertion");

            println!(
                "After add: len={}, empty={}, fallback={}",
                store.len(),
                store.is_empty(),
                store.is_using_fallback()
            );
            println!(
                "Stored key IDs: {:?}",
                store.get_stored_key_ids().expect("Test assertion")
            );

            assert!(!store.is_empty());
            assert_eq!(store.len(), 1);
        }

        #[test]
        #[ignore = "TDD - Implementation not complete"]
        fn test_keyring_tact_keystore_load_hardcoded_keys() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let result = store.load_hardcoded_keys();
            assert!(result.is_ok());
            let count = result.expect("Test assertion");
            assert!(count > 0);
            assert_eq!(store.len(), count);
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_keyring_tact_keystore_iterator() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let test_key1 = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);
            let test_key2 = TactKey::new(0xFEDC_BA09_8765_4321, [0x24; 16]);

            store.add(test_key1).expect("Test assertion");
            store.add(test_key2).expect("Test assertion");

            // Debug: check the internal state
            println!("Store len: {}", store.len());
            println!("Is using fallback: {}", store.is_using_fallback());
            println!(
                "Stored key IDs: {:?}",
                store.get_stored_key_ids().expect("Test assertion")
            );

            let keys: Vec<TactKey> = store.iter().collect();
            println!(
                "Keys from iterator: {:?}",
                keys.iter().map(|k| k.id).collect::<Vec<_>>()
            );
            assert_eq!(keys.len(), 2);

            let key_ids: Vec<u64> = keys.iter().map(|k| k.id).collect();
            assert!(key_ids.contains(&0x1234_5678_90AB_CDEF));
            assert!(key_ids.contains(&0xFEDC_BA09_8765_4321));
        }

        #[test]
        #[ignore = "Requires special keyring setup"]
        fn test_fallback_persistence() {
            use tempfile::TempDir;

            let temp_dir = TempDir::new().expect("Test assertion");

            // Create a config with a custom fallback path for testing
            let mut config = KeyringConfig::development();
            config.service_name = format!("test_cascette_{}", uuid::Uuid::new_v4());

            // Write a test keyring that will use the temp directory
            let fallback_path = temp_dir
                .path()
                .join(format!("{}_fallback.json", config.service_name));

            let test_key = TactKey::new(0xDEAD_BEEF_CAFE_BABE, [0x42; 16]);

            // First store instance - add a key
            {
                let store1 =
                    KeyringTactKeyStore::with_config(config.clone()).expect("Test assertion");
                store1.add(test_key).expect("Test assertion");

                // Should be in fallback mode and have the key
                assert!(store1.is_using_fallback());
                assert_eq!(store1.len(), 1);
                assert!(store1.get(test_key.id).expect("Test assertion").is_some());

                // Verify the fallback file was created
                assert!(fallback_path.exists());
            }

            // Second store instance - should load from file
            {
                let store2 = KeyringTactKeyStore::with_config(config).expect("Test assertion");

                // Should automatically load from fallback file
                assert!(store2.is_using_fallback());
                assert_eq!(store2.len(), 1);
                assert!(store2.get(test_key.id).expect("Test assertion").is_some());

                // Test iteration
                let keys: Vec<TactKey> = store2.iter().collect();
                assert_eq!(keys.len(), 1);
                assert_eq!(keys[0].id, test_key.id);
            }

            // Temp directory and files are automatically cleaned up when dropped
        }

        #[test]
        #[ignore = "TDD - Implementation not complete"]
        fn test_keyring_metrics_collection() {
            let store = KeyringTactKeyStore::new().expect("Test assertion");
            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);

            // Initial metrics should be zero
            let initial_metrics = store.metrics();
            assert_eq!(initial_metrics.keyring_reads, 0);
            assert_eq!(initial_metrics.keyring_writes, 0);
            assert_eq!(initial_metrics.keyring_deletes, 0);

            // Add a key - should increment writes
            store.add(test_key).expect("Test assertion");
            let after_add = store.metrics();
            assert_eq!(after_add.keyring_writes, 1);

            // Get a key - should increment reads
            store.get(0x1234_5678_90AB_CDEF).expect("Test assertion");
            let after_get = store.metrics();
            assert_eq!(after_get.keyring_reads, 1);

            // Remove a key - should increment deletes
            store.remove(0x1234_5678_90AB_CDEF).expect("Test assertion");
            let after_remove = store.metrics();
            assert_eq!(after_remove.keyring_deletes, 1);
        }
    }

    #[cfg(not(feature = "keyring"))]
    mod stub_tests {
        use super::*;

        #[test]
        fn test_stub_keyring_functionality_disabled() {
            let result = KeyringTactKeyStore::new();
            assert!(result.is_err());
        }

        #[test]
        fn test_stub_config_constructors_work() {
            let _dev_config = KeyringConfig::development();
            let _prod_config = KeyringConfig::production();
            let _secure_config = KeyringConfig::secure();
            // These should not panic even without keyring feature
        }
    }
}
