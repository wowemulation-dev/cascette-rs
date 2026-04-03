//! Key Mapping Table (KMT).
//!
//! The KMT is the primary on-disk key-to-location structure.
//!
//! # KMT v7 (IDX format)
//!
//! The `.idx` files ARE KMT v7 files -- same format, same operations.
//!
//! KMT v7 entries are 18 bytes with 9-byte truncated EKeys:
//!
//! | Offset | Size | Field |
//! |--------|------|-------|
//! | 0x00   | 9    | EKey (first 9 bytes of encoding key) |
//! | 0x09   | 5    | StorageOffset (big-endian, packed segment index + file offset) |
//! | 0x0E   | 4    | EncodedSize (little-endian, total encoded size including BLTE framing) |
//!
//! # KMT V8
//!
//! KMT V8 uses full 16-byte EKeys, 32-byte sorted entries, and
//! 40-byte update entries in 1024-byte pages. The outer header
//! carries a Jenkins hash for integrity, and the inner header
//! stores bucket and field size parameters.
//!
//! See [`kmt_file`] for the V8 parser.
//!
//! # Key State (v8 residency format)
//!
//! The "v8" referenced in Agent source paths (`key_state_v8.cpp`) refers
//! to the key state format, NOT the KMT file version. Key state tracks
//! per-key resident/non-resident status for partial download support.
//!
//! See [`key_state`] for residency tracking.

pub mod key_state;
pub mod kmt_file;

// Re-export key types from the index module.
// The KMT v7 IS the index -- same format, same file, same operations.
pub use crate::index::{ArchiveLocation, IndexEntry as KmtEntry, IndexManager as KmtManager};

// Re-export KMT V8 types.
pub use kmt_file::{
    KmtV8File, KmtV8InnerHeader, KmtV8OuterHeader, KmtV8SortedEntry, KmtV8UpdateEntry,
    KmtV8UpdatePage,
};
