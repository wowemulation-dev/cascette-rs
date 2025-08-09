//! NGDP patch system for incremental game updates
//!
//! This crate implements the ZBSDIFF patch format used by Blizzard's NGDP
//! for applying incremental updates to game files.

pub mod error;
pub mod patch_entry;
pub mod patch_file;
pub mod zbsdiff;

pub use error::{PatchError, Result};
pub use patch_entry::{PatchEntry, PatchRecord};
pub use patch_file::{PatchFile, PatchHeader};
pub use zbsdiff::{PatchFormat, apply_patch, create_patch};

/// Content key type (16 bytes MD5)
pub type ContentKey = [u8; 16];

/// Encoding key type (16 bytes MD5)
pub type EncodingKey = [u8; 16];
