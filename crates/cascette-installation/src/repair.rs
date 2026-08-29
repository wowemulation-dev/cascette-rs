//! Verify-then-redownload repair pipeline.
//!
//! Runs verification first, then re-downloads any files that failed
//! integrity checks from CDN.

use tracing::info;

use cascette_client_storage::Installation;
use cascette_protocol::ContentType;

use crate::cdn_source::CdnSource;
use crate::config::RepairConfig;
use crate::error::InstallationResult;
use crate::progress::ProgressEvent;
use crate::verify::{VerifyPipeline, VerifyReport};

/// Report from a repair operation.
#[derive(Debug)]
pub struct RepairReport {
    /// Verification results.
    pub verify: VerifyReport,
    /// Number of files re-downloaded.
    pub repaired: usize,
    /// Number of repair attempts that failed.
    pub repair_failed: usize,
    /// Keys that could not be repaired (hex).
    pub unrepaired_keys: Vec<String>,
}

/// Repair pipeline.
///
/// Verifies the installation, then re-downloads any invalid entries.
pub struct RepairPipeline {
    config: RepairConfig,
}

impl RepairPipeline {
    /// Create a new repair pipeline.
    #[must_use]
    pub fn new(config: RepairConfig) -> Self {
        Self { config }
    }

    /// Run the repair pipeline.
    pub async fn run<S: CdnSource>(
        self,
        cdn: &S,
        progress: impl Fn(ProgressEvent) + Send + Sync,
    ) -> InstallationResult<RepairReport> {
        // Step 1: Verify
        let verify_config = crate::config::VerifyConfig {
            install_path: self.config.install_path.clone(),
            mode: self.config.verify_mode,
        };

        let verify_pipeline = VerifyPipeline::new(verify_config);
        let verify_report = verify_pipeline.run(&progress).await?;

        if verify_report.invalid_keys.is_empty() {
            info!("no invalid entries found, nothing to repair");
            return Ok(RepairReport {
                verify: verify_report,
                repaired: 0,
                repair_failed: 0,
                unrepaired_keys: Vec::new(),
            });
        }

        info!(
            invalid = verify_report.invalid_keys.len(),
            "re-downloading invalid entries"
        );

        // Step 2: Re-download invalid entries
        let installation = Installation::open(self.config.install_path.join("Data"))?;
        installation.initialize().await?;

        let endpoint = self.config.endpoints.first().ok_or_else(|| {
            crate::error::InstallationError::InvalidConfig("no CDN endpoints".to_string())
        })?;

        let mut repaired: usize = 0;
        let mut repair_failed: usize = 0;
        let mut unrepaired_keys: Vec<String> = Vec::new();

        for key_hex in &verify_report.invalid_keys {
            progress(ProgressEvent::RepairDownloading {
                path: key_hex.clone(),
            });

            let key_bytes = match hex::decode(key_hex) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!(key = %key_hex, error = %e, "invalid hex key");
                    repair_failed += 1;
                    unrepaired_keys.push(key_hex.clone());
                    continue;
                }
            };

            match cdn.download(endpoint, ContentType::Data, &key_bytes).await {
                Ok(data) => match installation.write_raw_blte(data).await {
                    Ok(_) => {
                        repaired += 1;
                        progress(ProgressEvent::RepairComplete {
                            path: key_hex.clone(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!(key = %key_hex, error = %e, "failed to write repaired file");
                        repair_failed += 1;
                        unrepaired_keys.push(key_hex.clone());
                    }
                },
                Err(e) => {
                    tracing::warn!(key = %key_hex, error = %e, "failed to download repair file");
                    repair_failed += 1;
                    unrepaired_keys.push(key_hex.clone());
                }
            }
        }

        info!(
            repaired = repaired,
            failed = repair_failed,
            "repair complete"
        );

        Ok(RepairReport {
            verify: verify_report,
            repaired,
            repair_failed,
            unrepaired_keys,
        })
    }
}
