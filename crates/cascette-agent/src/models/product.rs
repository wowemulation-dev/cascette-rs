//! Product state machine.
//!
//! Products represent installed (or installable) game products. Their status
//! is driven by operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Product lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductStatus {
    /// Can be installed but is not currently installed.
    Available,
    /// Installation in progress.
    Installing,
    /// Installed and ready to use.
    Installed,
    /// Update in progress.
    Updating,
    /// Repair operation in progress.
    Repairing,
    /// Verification in progress.
    Verifying,
    /// Removal in progress.
    Uninstalling,
    /// Installed but verification detected corruption.
    Corrupted,
}

impl ProductStatus {
    /// Check whether the transition from `self` to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            // Installation flow
            (Self::Available, Self::Installing)
                | (
                    Self::Installing
                        | Self::Updating
                        | Self::Repairing
                        | Self::Verifying
                        | Self::Uninstalling,
                    Self::Installed
                )
                | (Self::Installing | Self::Uninstalling, Self::Available)
                | (
                    Self::Installed,
                    Self::Updating | Self::Repairing | Self::Verifying | Self::Uninstalling
                )
                | (Self::Verifying | Self::Repairing, Self::Corrupted)
                | (Self::Corrupted, Self::Repairing | Self::Uninstalling)
        )
    }

    /// Whether the product is currently in an active operation.
    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Installing
                | Self::Updating
                | Self::Repairing
                | Self::Verifying
                | Self::Uninstalling
        )
    }

    /// Whether the product is in an installed state (possibly with issues).
    #[must_use]
    pub fn is_installed(self) -> bool {
        matches!(
            self,
            Self::Installed | Self::Updating | Self::Repairing | Self::Verifying | Self::Corrupted
        )
    }
}

impl std::fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::Installing => write!(f, "installing"),
            Self::Installed => write!(f, "installed"),
            Self::Updating => write!(f, "updating"),
            Self::Repairing => write!(f, "repairing"),
            Self::Verifying => write!(f, "verifying"),
            Self::Uninstalling => write!(f, "uninstalling"),
            Self::Corrupted => write!(f, "corrupted"),
        }
    }
}

/// Installation mode for a product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallationMode {
    /// Container-based CASC installation.
    Casc,
    /// Containerless (loose file) installation.
    Containerless,
}

impl std::fmt::Display for InstallationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Casc => write!(f, "casc"),
            Self::Containerless => write!(f, "containerless"),
        }
    }
}

/// A game product managed by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    /// Unique product identifier (e.g., "wow", "wow_classic_era").
    pub product_code: String,
    /// Human-readable product name.
    pub name: String,
    /// Current product status.
    pub status: ProductStatus,
    /// Installed version string.
    pub version: Option<String>,
    /// Absolute path to installation directory.
    pub install_path: Option<String>,
    /// Total installation size in bytes.
    pub size_bytes: Option<u64>,
    /// CDN region (us, eu, kr, cn, tw).
    pub region: Option<String>,
    /// Game locale (e.g., enUS).
    pub locale: Option<String>,
    /// Installation type.
    pub installation_mode: Option<InstallationMode>,
    /// Whether a newer version is available.
    pub is_update_available: bool,
    /// Version string of the available update.
    pub available_version: Option<String>,
    /// Patch server URL from product registration.
    pub patch_url: Option<String>,
    /// Protocol identifier (e.g., "ngdp").
    pub protocol: Option<String>,
    /// Build config hash (hex) of the currently installed version.
    pub build_config: Option<String>,
    /// CDN config hash (hex) of the currently installed version.
    pub cdn_config: Option<String>,
    /// Subdirectory within the install directory (from registration).
    pub subfolder: Option<String>,
    /// Patch region hint from the launcher (e.g., "us").
    pub patch_region_hint: Option<String>,
    /// When the product was registered.
    pub created_at: DateTime<Utc>,
    /// Last status change.
    pub updated_at: DateTime<Utc>,
}

impl Product {
    /// Create a new product in Available status.
    #[must_use]
    pub fn new(product_code: String, name: String) -> Self {
        let now = Utc::now();
        Self {
            product_code,
            name,
            status: ProductStatus::Available,
            version: None,
            install_path: None,
            size_bytes: None,
            region: None,
            locale: None,
            installation_mode: None,
            is_update_available: false,
            available_version: None,
            patch_url: None,
            protocol: None,
            build_config: None,
            cdn_config: None,
            subfolder: None,
            patch_region_hint: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Attempt to transition to a new status.
    ///
    /// # Errors
    ///
    /// Returns `AgentError::InvalidTransition` if the transition is not valid.
    pub fn transition_to(
        &mut self,
        new_status: ProductStatus,
    ) -> Result<(), crate::error::AgentError> {
        if !self.status.can_transition_to(new_status) {
            return Err(crate::error::AgentError::InvalidTransition {
                from: self.status.to_string(),
                to: new_status.to_string(),
            });
        }
        self.status = new_status;
        self.updated_at = Utc::now();
        Ok(())
    }
}
