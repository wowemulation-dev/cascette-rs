//! Shared utilities for examples.

use std::path::PathBuf;

/// Get the default WoW Classic installation path.
#[must_use]
pub fn default_wow_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join("Downloads").join("wow_classic")
}
