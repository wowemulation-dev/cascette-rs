//! Key Mapping Table (KMT) v7.
//!
//! The KMT is the primary on-disk key-to-location structure.
//! The `.idx` files ARE the KMT files -- they are the same format.
//!
//!
//! # KMT Entry Format (18 bytes)
//!
//! This is identical to the `IndexEntry` format in the `index` module:
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0x00   | 9    | EKey (first 9 bytes of encoding key) |
//! | 0x09   | 5    | StorageOffset (big-endian, packed segment index + file offset) |
//! | 0x0E   | 4    | EncodedSize (little-endian, total encoded size including BLTE framing) |
//!
//! The StorageOffset packs two values using FileOffsetBits (typically 30):
//! - Upper bits (30-39): segment index (up to 1023)
//! - Lower bits (0-29): byte offset within the segment (up to 1 GiB)
//!
//! # File Format
//!
//! KMT files use the IDX v7 format with guarded blocks:
//!
//! ```text
//! [GuardedBlockHeader: 8 bytes]     -- header protection
//! [KMT Header: 16 bytes]           -- version 0x07, field sizes, FileOffsetBits
//! [Padding: 8 bytes]
//! [GuardedBlockHeader: 8 bytes]     -- sorted section protection
//! [Sorted Section: N × 18 bytes]   -- binary-searchable entries
//! [Padding to 64KB boundary]
//! [Update Section: paged entries]   -- append-only log (LSM-tree L0)
//! ```
//!
//! # Key State (v8 format, separate concept)
//!
//! The "v8" referenced in Source paths (`key_state_v8.cpp`) refers
//! to the key state format, NOT the KMT file version. Key state tracks
//! per-key resident/non-resident status for partial download support.

pub mod key_state;
pub mod kmt_file;

// Re-export key types from the index module.
// The KMT IS the index -- same format, same file, same operations.
pub use crate::index::{ArchiveLocation, IndexEntry as KmtEntry, IndexManager as KmtManager};
