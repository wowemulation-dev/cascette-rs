//! Shared memory protocol v4/v5 for multi-process coordination.
//!
//! CASC uses shared memory to coordinate access between the game
//! client and the Agent process. Protocol versions < 4 or > 5 are rejected.
//!
//! Protocol layout:
//! - Offset 0x02: Initialization byte (must be non-zero)
//! - Offset 0x42: Free space table format (must be 0x2AB8)
//! - Offset 0x43: Data size (must be non-zero)
//! - Offset 0x54: V5 exclusive access flag (bit 0)
//!
//! PID tracking uses "PID : name : mode" format in slot array.

pub mod control_block;
pub mod platform_unix;
#[cfg(target_os = "windows")]
pub mod platform_windows;

// Re-export legacy types from old shmem module for backward compatibility
// during the transition period.
mod legacy;

pub use control_block::ShmemControlBlock;
pub use legacy::*;
