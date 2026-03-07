//! Configuration for containerless storage.

use std::path::{Path, PathBuf};

/// Configuration for opening or creating a containerless storage instance.
#[derive(Debug, Clone)]
pub struct ContainerlessConfig {
    /// Root directory for loose file storage.
    pub root: PathBuf,

    /// Salsa20 encryption key for the SQLite database (16 bytes).
    /// When `None`, the database is treated as plaintext.
    pub db_key: Option<[u8; 16]>,

    /// Initialization vector for Salsa20 encryption.
    /// When `None` and `db_key` is set, IV is derived from the key.
    pub db_iv: Option<Vec<u8>>,

    /// Path to the SQLite database file.
    /// Defaults to `{root}/.product.db` if not set.
    pub db_path: Option<PathBuf>,
}

impl ContainerlessConfig {
    /// Create a config with defaults for the given root directory.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            db_key: None,
            db_iv: None,
            db_path: None,
        }
    }

    /// Set the database encryption key.
    #[must_use]
    pub fn with_db_key(mut self, key: [u8; 16]) -> Self {
        self.db_key = Some(key);
        self
    }

    /// Set the database encryption IV.
    #[must_use]
    pub fn with_db_iv(mut self, iv: Vec<u8>) -> Self {
        self.db_iv = Some(iv);
        self
    }

    /// Set the database file path.
    #[must_use]
    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.db_path = Some(path);
        self
    }

    /// Resolved database path.
    #[must_use]
    pub fn resolved_db_path(&self) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| self.root.join(".product.db"))
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), crate::error::ContainerlessError> {
        if self.root.as_os_str().is_empty() {
            return Err(crate::error::ContainerlessError::InvalidConfig(
                "root path is empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Return the root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}
