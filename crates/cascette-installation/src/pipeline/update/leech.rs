//! Alternate container leeching.
//!
//! Copies CASC data from a secondary installation instead of downloading
//! from CDN. Matches agent.exe behavior for shared installations (e.g.,
//! PTR client reusing live client archives).

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{debug, warn};

use cascette_client_storage::Installation;
use cascette_client_storage::container::AccessMode;
use cascette_client_storage::container::residency::ResidencyContainer;

use crate::error::{InstallationError, InstallationResult};

/// A read-only alternate CASC installation used as a file source.
pub struct AlternateSource {
    installation: Arc<Installation>,
    residency: ResidencyContainer,
}

/// Result of a single leech attempt.
#[derive(Debug)]
pub enum LeechResult {
    /// File was copied from the alternate source.
    Copied {
        /// Bytes copied.
        bytes: u64,
    },
    /// Key is not available in the alternate source.
    NotAvailable,
    /// Copy attempted but failed.
    Failed {
        /// Error description.
        error: String,
    },
}

/// Aggregate statistics for leech operations.
#[derive(Debug, Default)]
pub struct LeechStats {
    /// Number of files leeched.
    pub count: usize,
    /// Total bytes leeched.
    pub bytes: u64,
    /// Number of failed leech attempts.
    pub failed: usize,
}

impl AlternateSource {
    /// Open an alternate CASC installation for leeching.
    ///
    /// The installation is opened in read-only mode. Returns an error if
    /// the path does not contain a valid CASC installation.
    pub async fn open(alt_path: PathBuf) -> InstallationResult<Self> {
        let data_path = alt_path.join("Data");
        let installation = Installation::open(data_path).map_err(|e| {
            InstallationError::AlternateSource(format!(
                "failed to open alternate installation at {}: {e}",
                alt_path.display()
            ))
        })?;
        installation.initialize().await.map_err(|e| {
            InstallationError::AlternateSource(format!(
                "failed to initialize alternate installation: {e}"
            ))
        })?;

        let mut residency = ResidencyContainer::new(
            String::new(),
            AccessMode::ReadOnly,
            alt_path.join("Data").join("data"),
        );
        residency.initialize().await.map_err(|e| {
            InstallationError::AlternateSource(format!(
                "failed to initialize alternate residency container: {e}"
            ))
        })?;

        debug!(
            path = %alt_path.display(),
            keys = residency.resident_count(),
            "alternate source opened"
        );

        Ok(Self {
            installation: Arc::new(installation),
            residency,
        })
    }

    /// Check if a key is resident in the alternate container.
    pub fn is_available(&self, ekey: &[u8; 16]) -> bool {
        self.residency.is_resident(ekey)
    }

    /// Copy a file from the alternate to the primary installation.
    pub async fn leech(
        &self,
        ekey: &[u8; 16],
        target: &Installation,
    ) -> InstallationResult<LeechResult> {
        if !self.is_available(ekey) {
            return Ok(LeechResult::NotAvailable);
        }

        let ekey_obj = cascette_crypto::EncodingKey::from_bytes(*ekey);
        match self.installation.read_file_by_encoding_key(&ekey_obj).await {
            Ok(data) => {
                let bytes = data.len() as u64;
                match target.write_file(data, false).await {
                    Ok(_) => {
                        debug!(ekey = %hex::encode(ekey), bytes, "file leeched from alternate");
                        Ok(LeechResult::Copied { bytes })
                    }
                    Err(e) => {
                        warn!(ekey = %hex::encode(ekey), error = %e, "failed to write leeched file");
                        Ok(LeechResult::Failed {
                            error: e.to_string(),
                        })
                    }
                }
            }
            Err(e) => {
                warn!(ekey = %hex::encode(ekey), error = %e, "failed to read from alternate");
                Ok(LeechResult::Failed {
                    error: e.to_string(),
                })
            }
        }
    }

    /// Batch-check availability of encoding keys.
    pub fn filter_available(&self, ekeys: &[[u8; 16]]) -> Vec<[u8; 16]> {
        ekeys
            .iter()
            .filter(|k| self.is_available(k))
            .copied()
            .collect()
    }

    /// Access the underlying residency container.
    pub fn residency(&self) -> &ResidencyContainer {
        &self.residency
    }
}
