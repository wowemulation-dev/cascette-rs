//! Per-key resident/non-resident state tracking.
//!
//! Telemetry counters:
//! - `dynamic_container.key_state.mark_fully_resident`
//! - `dynamic_container.key_state.mark_fully_nonresident`
//! - `dynamic_container.key_state.grew_update_buffer`

/// Per-key residency state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key's data is fully downloaded and available.
    Resident,
    /// Key's data has been evicted or not yet downloaded.
    NonResident,
}

/// Key state tracker for a container.
///
/// Tracks how many keys have been marked resident/non-resident
/// for telemetry and maintains per-key state.
#[derive(Debug, Default)]
pub struct KeyStateTracker {
    /// Number of times `mark_fully_resident` was called.
    pub resident_count: u64,
    /// Number of times `mark_fully_nonresident` was called.
    pub non_resident_count: u64,
    /// Number of times the update buffer grew.
    pub grew_update_buffer_count: u64,
}

impl KeyStateTracker {
    /// Create a new key state tracker.
    pub const fn new() -> Self {
        Self {
            resident_count: 0,
            non_resident_count: 0,
            grew_update_buffer_count: 0,
        }
    }

    /// Mark a key as fully resident.
    pub fn mark_fully_resident(&mut self) {
        self.resident_count += 1;
    }

    /// Mark a key as fully non-resident.
    pub fn mark_fully_non_resident(&mut self) {
        self.non_resident_count += 1;
    }
}
