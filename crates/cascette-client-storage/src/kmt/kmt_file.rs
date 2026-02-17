//! KMT on-disk format with atomic write support.
//!
//! The KMT file has two sections:
//! - Sorted section: binary-searchable, 0x20-byte buckets
//! - Update section: append-only, 0x400-byte pages with 0x19 entries each
//!
//! Writes use the atomic flush-and-bind pattern (see Phase 11):
//! 1. Write to temp file
//! 2. fsync the temp file
//! 3. Rename temp -> target
//! 4. Retry up to 3 times on failure

// Implementation will be added in Phase 4/5.
