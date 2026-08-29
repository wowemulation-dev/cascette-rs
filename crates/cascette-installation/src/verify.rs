//! Installation verification.
//!
//! Checks integrity of an existing CASC installation by verifying
//! index entries against their expected encoding keys and sizes.

use tracing::{debug, info, warn};

use cascette_client_storage::Installation;

use crate::config::{VerifyConfig, VerifyMode};
use crate::error::InstallationResult;
use crate::progress::ProgressEvent;

/// Report from verification.
#[derive(Debug)]
pub struct VerifyReport {
    /// Total entries checked.
    pub total: usize,
    /// Entries that passed verification.
    pub valid: usize,
    /// Entries that failed verification.
    pub invalid: usize,
    /// Entries that were missing.
    pub missing: usize,
    /// Keys of invalid entries (hex).
    pub invalid_keys: Vec<String>,
}

/// Verification pipeline.
///
/// Checks the integrity of a local CASC installation.
pub struct VerifyPipeline {
    config: VerifyConfig,
}

impl VerifyPipeline {
    /// Create a new verify pipeline.
    #[must_use]
    pub fn new(config: VerifyConfig) -> Self {
        Self { config }
    }

    /// Run verification.
    pub async fn run(
        self,
        progress: impl Fn(ProgressEvent) + Send + Sync,
    ) -> InstallationResult<VerifyReport> {
        let installation = Installation::open(self.config.install_path.join("Data"))?;
        installation.initialize().await?;

        let entries = installation.get_all_index_entries().await;
        let total = entries.len();

        info!(total = total, mode = ?self.config.mode, "starting verification");

        let mut valid: usize = 0;
        let mut invalid: usize = 0;
        let mut missing: usize = 0;
        let mut invalid_keys: Vec<String> = Vec::new();

        for entry in &entries {
            let key_hex = hex::encode(entry.key);

            match self.config.mode {
                VerifyMode::Existence => {
                    // Just check the index entry exists (it does, since we got it)
                    valid += 1;
                    progress(ProgressEvent::VerifyResult {
                        path: key_hex,
                        valid: true,
                    });
                }
                VerifyMode::Size => {
                    // Validate the archive entry can be read
                    match installation
                        .validate_entry(
                            entry.archive_location.archive_id,
                            entry.archive_location.archive_offset,
                            entry.size,
                        )
                        .await
                    {
                        Ok(true) => {
                            valid += 1;
                            progress(ProgressEvent::VerifyResult {
                                path: key_hex,
                                valid: true,
                            });
                        }
                        Ok(false) => {
                            invalid += 1;
                            invalid_keys.push(key_hex.clone());
                            progress(ProgressEvent::VerifyResult {
                                path: key_hex,
                                valid: false,
                            });
                        }
                        Err(e) => {
                            warn!(key = %key_hex, error = %e, "validation error");
                            missing += 1;
                            invalid_keys.push(key_hex.clone());
                            progress(ProgressEvent::VerifyResult {
                                path: key_hex,
                                valid: false,
                            });
                        }
                    }
                }
                VerifyMode::Full => {
                    // Read and decompress the file to verify BLTE integrity
                    match installation
                        .read_from_archive(
                            entry.archive_location.archive_id,
                            entry.archive_location.archive_offset,
                            entry.size,
                        )
                        .await
                    {
                        Ok(_data) => {
                            valid += 1;
                            debug!(key = %key_hex, "verified");
                            progress(ProgressEvent::VerifyResult {
                                path: key_hex,
                                valid: true,
                            });
                        }
                        Err(e) => {
                            warn!(key = %key_hex, error = %e, "verification failed");
                            invalid += 1;
                            invalid_keys.push(key_hex.clone());
                            progress(ProgressEvent::VerifyResult {
                                path: key_hex,
                                valid: false,
                            });
                        }
                    }
                }
            }
        }

        info!(
            valid = valid,
            invalid = invalid,
            missing = missing,
            "verification complete"
        );

        Ok(VerifyReport {
            total,
            valid,
            invalid,
            missing,
            invalid_keys,
        })
    }
}
