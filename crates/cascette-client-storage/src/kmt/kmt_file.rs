//! KMT on-disk format operations.
//!
//! The KMT file format is identical to the IDX v7 format. All read/write
//! operations are handled by `IndexManager` in the `index` module.
//!
//! This module exists for documentation and future extensions:
//! - The sorted section uses per-entry `hashlittle2()` hash accumulation
//! - The update section uses 0x1000-byte pages with per-entry 32-bit hashes
//! - Compaction merges the update section into the sorted section
//!
//! For current implementation, see:
//! - `IndexManager::load_index()` -- reads IDX v7 sorted section
//! - `IndexManager::save_index()` -- writes IDX v7 with guarded blocks
//! - `IndexManager::lookup()` -- binary search in sorted entries
