//! Unix shared memory implementation using `shm_open`.
//!
//! Provides `shm_open`-based shared memory with `flock` for writer
//! exclusion and file permissions matching the DACL intent from the
//! Windows implementation.

// Platform-specific implementation will be added in Phase 8.
// This file establishes the module structure.
