//! Patch application pipeline
//!
//! Implements the three patch application strategies from the TACT system:
//!
//! - **BsDiff** (`bsdiff`): Full-file binary differential patching using ZBSDIFF1
//! - **Block Patch** (`block_patch`): Block-by-block byte-level diff for BLTE-chunked files
//! - **Re-encode** (`reencode`): Decode and re-encode with a different ESpec
//!
//! These strategies match the agent.exe state machines:
//! `FileBsDiffPatchState`, `FileBlockPatchState`, and `FileReEncodePatchState`.

/// Patch chain application (multi-step orchestrator)
pub mod applicator;
/// Block-level diff patching
pub mod block_patch;
/// BsDiff full-file patching (ZBSDIFF1 + BLTE)
pub mod bsdiff;
/// Patch application error types
pub mod error;
/// Re-encode patching (change ESpec without changing content)
pub mod reencode;
/// Patch data resolution from CDN archives
pub mod resolver;

pub use applicator::apply_patch_chain;
pub use block_patch::apply_block_patch;
pub use bsdiff::apply_bsdiff_patch;
pub use error::PatchError;
pub use reencode::apply_reencode_patch;
pub use resolver::PatchResolver;
