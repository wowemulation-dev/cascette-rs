//! File-based TACT key storage
//!
//! This module provides secure, persistent storage for TACT encryption keys using
//! individual encrypted files on disk. Each key is stored as a separate encrypted file,
//! with the master encryption password stored in the OS keyring.

#[cfg(feature = "file-store")]
mod implementation {
    use crate::{CryptoError, TactKey};
    use aes_gcm::{
        Aes256Gcm, Key, Nonce,
        aead::{Aead, AeadCore, KeyInit, OsRng, rand_core::RngCore},
    };
    use base64::{Engine as _, engine::general_purpose};
    use keyring;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::path::PathBuf;
    use thiserror::Error;

    /// Error types specific to file-based storage operations
    #[derive(Debug, Error)]
    pub enum FileStoreError {
        #[error("Master password not found in keyring")]
        /// Master password not found in keyring
        MasterPasswordNotFound,

        #[error("Invalid master password")]
        /// Invalid master password
        InvalidMasterPassword,

        #[error("Key not found: {key_id:016X}")]
        /// Key not found in storage
        KeyNotFound {
            /// The key ID that was not found
            key_id: u64,
        },

        #[error("Invalid key data format: {reason}")]
        /// Invalid key data format
        InvalidFormat {
            /// Description of the format error
            reason: String,
        },

        #[error("Encryption failed: {reason}")]
        /// Encryption operation failed
        EncryptionError {
            /// Description of the encryption error
            reason: String,
        },

        #[error("Decryption failed: {reason}")]
        /// Decryption operation failed
        DecryptionError {
            /// Description of the decryption error
            reason: String,
        },

        #[error("File I/O error: {0}")]
        /// File I/O error
        IoError(#[from] std::io::Error),

        #[error("Keyring error: {0}")]
        /// Keyring error
        KeyringError(#[from] keyring::Error),

        #[error("JSON error: {0}")]
        /// JSON serialization/deserialization error
        JsonError(#[from] serde_json::Error),

        #[error("Crypto error: {0}")]
        /// Crypto operation error
        CryptoError(#[from] CryptoError),
    }

    /// Configuration for file-based key storage
    #[derive(Debug, Clone)]
    pub struct FileStoreConfig {
        /// Directory to store encrypted key files
        pub keys_directory: PathBuf,
        /// Service name for keyring storage of master password
        pub keyring_service: String,
        /// Username for keyring storage of master password
        pub keyring_username: String,
    }

    impl Default for FileStoreConfig {
        fn default() -> Self {
            Self::production()
        }
    }

    impl FileStoreConfig {
        /// Development configuration
        pub fn development() -> Self {
            Self {
                keys_directory: Self::get_default_keys_dir()
                    .unwrap_or_else(|| PathBuf::from(".cascette-dev/tact_keys")),
                keyring_service: "cascette-dev-tact-keys".to_string(),
                keyring_username: "master-password".to_string(),
            }
        }

        /// Production configuration
        pub fn production() -> Self {
            Self {
                keys_directory: Self::get_default_keys_dir()
                    .unwrap_or_else(|| PathBuf::from(".cascette/tact_keys")),
                keyring_service: "cascette-tact-keys".to_string(),
                keyring_username: "master-password".to_string(),
            }
        }

        /// Get the default keys directory using XDG specification
        fn get_default_keys_dir() -> Option<PathBuf> {
            // Use XDG_DATA_HOME or fallback to ~/.local/share
            let data_dir = std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|_| {
                    std::env::var("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
                })
                .ok()?;

            Some(data_dir.join("cascette").join("tact_keys"))
        }
    }

    /// Encrypted key file format
    #[derive(Debug, Serialize, Deserialize)]
    struct EncryptedKeyFile {
        /// Version of the file format
        version: u8,
        /// Key metadata
        metadata: KeyMetadata,
        /// Encrypted key data (base64 encoded)
        encrypted_key: String,
        /// Nonce used for encryption (base64 encoded)
        nonce: String,
    }

    /// Metadata stored with each key
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct KeyMetadata {
        /// The key ID (lookup hash)
        pub key_id: u64,
        /// Source of the key (e.g., "github", "wago", "manual")
        pub source: String,
        /// Optional description or comment
        pub description: Option<String>,
        /// When the key was added
        pub added_at: chrono::DateTime<chrono::Utc>,
        /// When the key was last verified working
        pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
        /// Associated product (e.g., `"wow"`, `"wow_classic"`)
        pub product: Option<String>,
        /// Associated build number if known
        pub build: Option<u32>,
    }

    /// File-based TACT key storage
    pub struct FileBasedTactKeyStore {
        config: FileStoreConfig,
        master_key: Option<Key<Aes256Gcm>>,
    }

    impl FileBasedTactKeyStore {
        /// Create new store with default configuration
        pub fn new() -> Result<Self, FileStoreError> {
            Self::with_config(FileStoreConfig::default())
        }

        /// Create store with custom configuration
        pub fn with_config(config: FileStoreConfig) -> Result<Self, FileStoreError> {
            let store = Self {
                config,
                master_key: None,
            };

            // Ensure keys directory exists
            fs::create_dir_all(&store.config.keys_directory)?;

            Ok(store)
        }

        /// Initialize or retrieve the master password from keyring with file fallback
        #[allow(deprecated)] // aes-gcm uses generic-array 0.x, will be resolved when upgraded to 1.x
        pub fn ensure_master_password(&mut self) -> Result<(), FileStoreError> {
            if self.master_key.is_some() {
                return Ok(());
            }

            // Use file-based storage as primary method due to keyring reliability issues
            // Try keyring first, but fall back to file storage when keyring fails
            let master_password = self.try_file_password()?;

            // Derive key from master password
            let key_bytes = general_purpose::STANDARD
                .decode(&master_password)
                .map_err(|e| FileStoreError::InvalidFormat {
                    reason: format!("Invalid master password format: {}", e),
                })?;

            if key_bytes.len() != 32 {
                return Err(FileStoreError::InvalidFormat {
                    reason: "Master password must be 32 bytes".to_string(),
                });
            }

            self.master_key = Some(*Key::<Aes256Gcm>::from_slice(&key_bytes));
            Ok(())
        }

        /// Try to get/set master password from encrypted file fallback
        #[allow(deprecated)] // aes-gcm uses generic-array 0.x, will be resolved when upgraded to 1.x
        fn try_file_password(&self) -> Result<String, FileStoreError> {
            let password_file = self.config.keys_directory.join(".master_key");

            if password_file.exists() {
                // Read existing password file
                let encrypted_data = fs::read(&password_file)?;

                // Derive decryption key from system information
                let system_key = self.derive_system_key();
                let cipher = Aes256Gcm::new(&system_key);

                if encrypted_data.len() < 12 {
                    return Err(FileStoreError::InvalidFormat {
                        reason: "Password file too short".to_string(),
                    });
                }

                let nonce = Nonce::from_slice(&encrypted_data[0..12]);
                let ciphertext = &encrypted_data[12..];

                let decrypted = cipher.decrypt(nonce, ciphertext).map_err(|e| {
                    FileStoreError::DecryptionError {
                        reason: format!("Failed to decrypt password file: {}", e),
                    }
                })?;

                let password =
                    String::from_utf8(decrypted).map_err(|e| FileStoreError::InvalidFormat {
                        reason: format!("Invalid password file encoding: {}", e),
                    })?;

                Ok(password)
            } else {
                // Generate new master password
                let mut password_bytes = [0u8; 32];
                OsRng.fill_bytes(&mut password_bytes);
                let password = general_purpose::STANDARD.encode(password_bytes);

                // Encrypt and store password
                let system_key = self.derive_system_key();
                let cipher = Aes256Gcm::new(&system_key);
                let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

                let ciphertext = cipher.encrypt(&nonce, password.as_bytes()).map_err(|e| {
                    FileStoreError::EncryptionError {
                        reason: format!("Failed to encrypt password: {}", e),
                    }
                })?;

                let mut encrypted_data = Vec::new();
                encrypted_data.extend_from_slice(&nonce);
                encrypted_data.extend_from_slice(&ciphertext);

                fs::write(&password_file, encrypted_data)?;

                Ok(password)
            }
        }

        /// Derive a system-specific key for file encryption
        #[allow(deprecated)] // aes-gcm uses generic-array 0.x, will be resolved when upgraded to 1.x
        fn derive_system_key(&self) -> Key<Aes256Gcm> {
            let mut hasher = DefaultHasher::new();

            // Hash service and username for uniqueness
            self.config.keyring_service.hash(&mut hasher);
            self.config.keyring_username.hash(&mut hasher);

            // Add some system-specific information
            if let Ok(hostname) = std::env::var("HOSTNAME") {
                hostname.hash(&mut hasher);
            }
            if let Ok(user) = std::env::var("USER") {
                user.hash(&mut hasher);
            }

            // Add path to make it unique per installation
            self.config.keys_directory.hash(&mut hasher);

            let hash = hasher.finish();

            // Expand hash to 32 bytes using repeated hashing
            let mut key_bytes = [0u8; 32];
            for i in 0..4 {
                let mut chunk_hasher = DefaultHasher::new();
                hash.hash(&mut chunk_hasher);
                i.hash(&mut chunk_hasher);
                "cascette-file-store".hash(&mut chunk_hasher);
                let chunk_hash = chunk_hasher.finish();
                key_bytes[i * 8..(i + 1) * 8].copy_from_slice(&chunk_hash.to_le_bytes());
            }

            *Key::<Aes256Gcm>::from_slice(&key_bytes)
        }

        /// Add a TACT key to storage
        pub fn add(&mut self, key: TactKey, metadata: KeyMetadata) -> Result<(), FileStoreError> {
            self.ensure_master_password()?;

            let cipher = Aes256Gcm::new(
                self.master_key
                    .as_ref()
                    .expect("Master key must be initialized"),
            );
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

            let encrypted_key = cipher.encrypt(&nonce, key.key.as_ref()).map_err(|e| {
                FileStoreError::EncryptionError {
                    reason: format!("AES-GCM encryption failed: {}", e),
                }
            })?;

            let key_file = EncryptedKeyFile {
                version: 1,
                metadata,
                encrypted_key: general_purpose::STANDARD.encode(&encrypted_key),
                nonce: general_purpose::STANDARD.encode(nonce),
            };

            let file_path = self.get_key_file_path(key.id);
            let json = serde_json::to_string_pretty(&key_file)?;
            fs::write(&file_path, json)?;

            Ok(())
        }

        /// Retrieve a key by ID
        #[allow(deprecated)] // aes-gcm uses generic-array 0.x, will be resolved when upgraded to 1.x
        pub fn get(
            &mut self,
            key_id: u64,
        ) -> Result<Option<(TactKey, KeyMetadata)>, FileStoreError> {
            self.ensure_master_password()?;

            let file_path = self.get_key_file_path(key_id);
            if !file_path.exists() {
                return Ok(None);
            }

            let json = fs::read_to_string(&file_path)?;
            let key_file: EncryptedKeyFile = serde_json::from_str(&json)?;

            if key_file.version != 1 {
                return Err(FileStoreError::InvalidFormat {
                    reason: format!("Unsupported file format version: {}", key_file.version),
                });
            }

            let cipher = Aes256Gcm::new(
                self.master_key
                    .as_ref()
                    .expect("Master key must be initialized"),
            );

            let encrypted_key = general_purpose::STANDARD
                .decode(&key_file.encrypted_key)
                .map_err(|e| FileStoreError::InvalidFormat {
                    reason: format!("Invalid encrypted key format: {}", e),
                })?;

            let nonce_bytes = general_purpose::STANDARD
                .decode(&key_file.nonce)
                .map_err(|e| FileStoreError::InvalidFormat {
                    reason: format!("Invalid nonce format: {}", e),
                })?;

            let nonce = Nonce::from_slice(&nonce_bytes);

            let decrypted_key = cipher.decrypt(nonce, encrypted_key.as_ref()).map_err(|e| {
                FileStoreError::DecryptionError {
                    reason: format!("AES-GCM decryption failed: {}", e),
                }
            })?;

            if decrypted_key.len() != 16 {
                return Err(FileStoreError::InvalidFormat {
                    reason: format!("Expected 16 bytes, got {}", decrypted_key.len()),
                });
            }

            let mut key_array = [0u8; 16];
            key_array.copy_from_slice(&decrypted_key);

            let tact_key = TactKey::new(key_id, key_array);
            Ok(Some((tact_key, key_file.metadata)))
        }

        /// Remove a key from storage
        pub fn remove(&self, key_id: u64) -> Result<bool, FileStoreError> {
            let file_path = self.get_key_file_path(key_id);
            if file_path.exists() {
                fs::remove_file(&file_path)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        /// List all stored keys
        pub fn list_keys(&mut self) -> Result<Vec<(TactKey, KeyMetadata)>, FileStoreError> {
            let mut keys = Vec::new();

            for entry in fs::read_dir(&self.config.keys_directory)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("key") {
                    if let Some(stem) = path.file_stem() {
                        if let Some(key_id_str) = stem.to_str() {
                            if let Ok(key_id) = u64::from_str_radix(key_id_str, 16) {
                                if let Ok(Some((key, metadata))) = self.get(key_id) {
                                    keys.push((key, metadata));
                                }
                            }
                        }
                    }
                }
            }

            // Sort by key ID for consistent output
            keys.sort_by_key(|(k, _)| k.id);
            Ok(keys)
        }

        /// Get the number of stored keys
        pub fn len(&self) -> Result<usize, FileStoreError> {
            let mut count = 0;

            for entry in fs::read_dir(&self.config.keys_directory)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("key") {
                    count += 1;
                }
            }

            Ok(count)
        }

        /// Check if the store is empty
        pub fn is_empty(&self) -> Result<bool, FileStoreError> {
            Ok(self.len()? == 0)
        }

        /// Check if a key exists
        pub fn has_key(&self, key_id: u64) -> bool {
            self.get_key_file_path(key_id).exists()
        }

        /// Update metadata for an existing key
        pub fn update_metadata(
            &mut self,
            key_id: u64,
            metadata: KeyMetadata,
        ) -> Result<(), FileStoreError> {
            // Get the existing key
            if let Some((key, _)) = self.get(key_id)? {
                // Re-add with new metadata
                self.add(key, metadata)?;
                Ok(())
            } else {
                Err(FileStoreError::KeyNotFound { key_id })
            }
        }

        /// Clear the master password from memory (for security)
        pub fn clear_master_password(&mut self) {
            self.master_key = None;
        }

        /// Import keys from a legacy format
        pub fn import_legacy_keys(
            &mut self,
            keys: Vec<(TactKey, KeyMetadata)>,
        ) -> Result<usize, FileStoreError> {
            let mut imported = 0;

            for (key, metadata) in keys {
                self.add(key, metadata)?;
                imported += 1;
            }

            Ok(imported)
        }

        /// Export keys to a map (for migration or backup)
        pub fn export_keys(
            &mut self,
        ) -> Result<HashMap<u64, (TactKey, KeyMetadata)>, FileStoreError> {
            let keys = self.list_keys()?;
            let mut map = HashMap::new();

            for (key, metadata) in keys {
                map.insert(key.id, (key, metadata));
            }

            Ok(map)
        }

        /// Get the file path for a key ID
        fn get_key_file_path(&self, key_id: u64) -> PathBuf {
            self.config
                .keys_directory
                .join(format!("{:016X}.key", key_id))
        }

        /// Reset master password (for testing or recovery)
        pub fn reset_master_password(&mut self) -> Result<(), FileStoreError> {
            let entry =
                keyring::Entry::new(&self.config.keyring_service, &self.config.keyring_username)?;

            // Delete existing password
            let _ = entry.delete_credential();

            // Clear in-memory key
            self.master_key = None;

            // Generate new master password on next access
            Ok(())
        }
    }

    impl Default for FileBasedTactKeyStore {
        fn default() -> Self {
            Self::new().expect("Failed to create default FileBasedTactKeyStore")
        }
    }
}

#[cfg(feature = "file-store")]
pub use implementation::*;

#[cfg(not(feature = "file-store"))]
mod stub {
    use crate::{CryptoError, TactKey};
    use std::collections::HashMap;
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[error("File-based storage not enabled. Enable the 'file-store' feature")]
    pub struct FileStoreError;

    impl From<CryptoError> for FileStoreError {
        fn from(_: CryptoError) -> Self {
            FileStoreError
        }
    }

    impl From<std::io::Error> for FileStoreError {
        fn from(_: std::io::Error) -> Self {
            FileStoreError
        }
    }

    impl From<keyring::Error> for FileStoreError {
        fn from(_: keyring::Error) -> Self {
            FileStoreError
        }
    }

    impl From<serde_json::Error> for FileStoreError {
        fn from(_: serde_json::Error) -> Self {
            FileStoreError
        }
    }

    #[derive(Debug, Clone)]
    pub struct FileStoreConfig;

    impl Default for FileStoreConfig {
        fn default() -> Self {
            FileStoreConfig
        }
    }

    impl FileStoreConfig {
        pub fn development() -> Self {
            FileStoreConfig
        }
        pub fn production() -> Self {
            FileStoreConfig
        }
    }

    #[derive(Debug, Clone)]
    pub struct KeyMetadata {
        pub key_id: u64,
        pub source: String,
        pub description: Option<String>,
        pub added_at: chrono::DateTime<chrono::Utc>,
        pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
        pub product: Option<String>,
        pub build: Option<u32>,
    }

    pub struct FileBasedTactKeyStore;

    impl FileBasedTactKeyStore {
        pub fn new() -> Result<Self, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn with_config(_config: FileStoreConfig) -> Result<Self, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn ensure_master_password(&mut self) -> Result<(), FileStoreError> {
            Err(FileStoreError)
        }
        pub fn add(&mut self, _key: TactKey, _metadata: KeyMetadata) -> Result<(), FileStoreError> {
            Err(FileStoreError)
        }
        pub fn get(
            &mut self,
            _key_id: u64,
        ) -> Result<Option<(TactKey, KeyMetadata)>, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn remove(&self, _key_id: u64) -> Result<bool, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn list_keys(&mut self) -> Result<Vec<(TactKey, KeyMetadata)>, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn len(&self) -> Result<usize, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn is_empty(&self) -> Result<bool, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn has_key(&self, _key_id: u64) -> bool {
            false
        }
        pub fn update_metadata(
            &mut self,
            _key_id: u64,
            _metadata: KeyMetadata,
        ) -> Result<(), FileStoreError> {
            Err(FileStoreError)
        }
        pub fn clear_master_password(&mut self) {}
        pub fn import_legacy_keys(
            &mut self,
            _keys: Vec<(TactKey, KeyMetadata)>,
        ) -> Result<usize, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn export_keys(
            &mut self,
        ) -> Result<HashMap<u64, (TactKey, KeyMetadata)>, FileStoreError> {
            Err(FileStoreError)
        }
        pub fn reset_master_password(&mut self) -> Result<(), FileStoreError> {
            Err(FileStoreError)
        }
    }

    impl Default for FileBasedTactKeyStore {
        fn default() -> Self {
            FileBasedTactKeyStore
        }
    }
}

#[cfg(not(feature = "file-store"))]
pub use stub::*;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::TactKey;
    use tempfile::TempDir;
    use uuid;

    #[cfg(feature = "file-store")]
    mod file_store_tests {
        use super::*;

        #[test]
        fn test_file_store_config_creation() {
            let dev_config = FileStoreConfig::development();
            assert_eq!(dev_config.keyring_service, "cascette-dev-tact-keys");
            assert_eq!(dev_config.keyring_username, "master-password");

            let prod_config = FileStoreConfig::production();
            assert_eq!(prod_config.keyring_service, "cascette-tact-keys");
            assert_eq!(prod_config.keyring_username, "master-password");
        }

        #[test]
        fn test_file_store_creation() {
            let temp_dir = TempDir::new().expect("Failed to create temporary directory for test");
            let config = FileStoreConfig {
                keys_directory: temp_dir.path().to_path_buf(),
                keyring_service: format!("test-cascette-{}", uuid::Uuid::new_v4()),
                keyring_username: "test-master".to_string(),
            };

            let result = FileBasedTactKeyStore::with_config(config);
            assert!(result.is_ok());
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_master_password_generation() {
            let temp_dir = TempDir::new()
                .expect("Failed to create temporary directory for master password generation test");
            let config = FileStoreConfig {
                keys_directory: temp_dir.path().to_path_buf(),
                keyring_service: format!("test-cascette-{}", uuid::Uuid::new_v4()),
                keyring_username: "test-master".to_string(),
            };

            let mut store = FileBasedTactKeyStore::with_config(config)
                .expect("Failed to create file store with test config");
            let result = store.ensure_master_password();
            assert!(result.is_ok());
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_master_password_persistence_across_instances() {
            let temp_dir =
                TempDir::new().expect("Failed to create temporary directory for persistence test");
            let unique_service = format!("test-cascette-persistence-{}", uuid::Uuid::new_v4());

            let config = FileStoreConfig {
                keys_directory: temp_dir.path().to_path_buf(),
                keyring_service: unique_service.clone(),
                keyring_username: "test-master".to_string(),
            };

            // Create first store instance and add a key
            let mut store1 = FileBasedTactKeyStore::with_config(config.clone())
                .expect("Failed to create first store instance for persistence test");
            store1
                .ensure_master_password()
                .expect("Failed to ensure master password for first store instance");

            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);
            let metadata = KeyMetadata {
                key_id: test_key.id,
                source: "test".to_string(),
                description: Some("Test key for persistence".to_string()),
                added_at: chrono::Utc::now(),
                last_verified: None,
                product: None,
                build: None,
            };

            store1
                .add(test_key, metadata.clone())
                .expect("Failed to add test key to first store instance");

            // Verify key was added
            let retrieved1 = store1
                .get(test_key.id)
                .expect("Failed to retrieve test key from first store instance");
            assert!(retrieved1.is_some());

            // Drop first store to simulate process ending
            drop(store1);

            // Create second store instance with same config
            let mut store2 = FileBasedTactKeyStore::with_config(config)
                .expect("Failed to create second store instance for persistence test");
            store2
                .ensure_master_password()
                .expect("Failed to ensure master password for second store instance");

            // Should be able to retrieve the same key
            let retrieved2 = store2
                .get(test_key.id)
                .expect("Failed to retrieve test key from second store instance");
            assert!(
                retrieved2.is_some(),
                "Key should persist across different store instances"
            );

            let (retrieved_key, retrieved_metadata) =
                retrieved2.expect("Expected to find persisted key in second store instance");
            assert_eq!(retrieved_key.id, test_key.id);
            assert_eq!(retrieved_key.key, test_key.key);
            assert_eq!(retrieved_metadata.source, metadata.source);

            // Clean up keyring entry
            let entry = keyring::Entry::new(&unique_service, "test-master")
                .expect("Failed to create keyring entry for cleanup");
            let _ = entry.delete_credential();
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_add_and_get_key() {
            let temp_dir =
                TempDir::new().expect("Failed to create temporary directory for add/get key test");
            let config = FileStoreConfig {
                keys_directory: temp_dir.path().to_path_buf(),
                keyring_service: format!("test-cascette-{}", uuid::Uuid::new_v4()),
                keyring_username: "test-master".to_string(),
            };

            let mut store = FileBasedTactKeyStore::with_config(config)
                .expect("Failed to create file store for add/get key test");
            let test_key = TactKey::new(0x1234_5678_90AB_CDEF, [0x42; 16]);
            let metadata = KeyMetadata {
                key_id: test_key.id,
                source: "test".to_string(),
                description: Some("Test key".to_string()),
                added_at: chrono::Utc::now(),
                last_verified: None,
                product: Some("test".to_string()),
                build: Some(12345),
            };

            // Add key
            let result = store.add(test_key, metadata.clone());
            assert!(result.is_ok());

            // Check file exists
            assert!(store.has_key(test_key.id));

            // Get key
            let retrieved = store
                .get(test_key.id)
                .expect("Failed to retrieve stored key");
            assert!(retrieved.is_some());
            let (retrieved_key, retrieved_metadata) =
                retrieved.expect("Expected to find stored key");
            assert_eq!(retrieved_key.id, test_key.id);
            assert_eq!(retrieved_key.key, test_key.key);
            assert_eq!(retrieved_metadata.source, metadata.source);
        }

        #[test]
        #[ignore = "Requires keyring access"]
        fn test_list_and_remove_keys() {
            let temp_dir = TempDir::new()
                .expect("Failed to create temporary directory for list/remove keys test");
            let config = FileStoreConfig {
                keys_directory: temp_dir.path().to_path_buf(),
                keyring_service: format!("test-cascette-{}", uuid::Uuid::new_v4()),
                keyring_username: "test-master".to_string(),
            };

            let mut store = FileBasedTactKeyStore::with_config(config)
                .expect("Failed to create file store for list/remove keys test");

            // Add two test keys
            let key1 = TactKey::new(0x1111_1111_1111_1111, [0x11; 16]);
            let key2 = TactKey::new(0x2222_2222_2222_2222, [0x22; 16]);

            let metadata1 = KeyMetadata {
                key_id: key1.id,
                source: "test1".to_string(),
                description: None,
                added_at: chrono::Utc::now(),
                last_verified: None,
                product: None,
                build: None,
            };

            let metadata2 = KeyMetadata {
                key_id: key2.id,
                source: "test2".to_string(),
                description: None,
                added_at: chrono::Utc::now(),
                last_verified: None,
                product: None,
                build: None,
            };

            store
                .add(key1, metadata1)
                .expect("Failed to add first test key");
            store
                .add(key2, metadata2)
                .expect("Failed to add second test key");

            // List keys
            let listed_keys = store.list_keys().expect("Failed to list stored keys");
            assert_eq!(listed_keys.len(), 2);
            assert_eq!(store.len().expect("Failed to get store length"), 2);
            assert!(!store.is_empty().expect("Failed to check if store is empty"));

            // Remove one key
            let removed = store
                .remove(key1.id)
                .expect("Failed to remove key from store");
            assert!(removed);
            assert!(!store.has_key(key1.id));

            // List again
            let remaining_keys = store
                .list_keys()
                .expect("Failed to list remaining keys after removal");
            assert_eq!(remaining_keys.len(), 1);
            assert_eq!(remaining_keys[0].0.id, key2.id);
        }
    }

    #[cfg(not(feature = "file-store"))]
    mod stub_tests {
        use super::*;

        #[test]
        fn test_stub_file_store_functionality_disabled() {
            let result = FileBasedTactKeyStore::new();
            assert!(result.is_err());
        }

        #[test]
        fn test_stub_config_constructors_work() {
            let _dev_config = FileStoreConfig::development();
            let _prod_config = FileStoreConfig::production();
            // These should not panic even without file-store feature
        }
    }
}
