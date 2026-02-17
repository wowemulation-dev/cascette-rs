//! Windows shared memory implementation using `CreateFileMapping`.
//!
//! Win32 specifics:
//! - `FILE_ATTRIBUTE_TEMPORARY` with `FILE_SHARE_READ | FILE_SHARE_WRITE`
//! - 10-retry bind with `Sleep(0)` between attempts
//! - `ERROR_DISK_FULL` returns error code 7
//! - Writer lock via named global mutex (`Global\` prefix)
//! - DACL: `D:(A;;GA;;;WD)(A;;GA;;;AN)`

// Platform-specific implementation will be added in Phase 8.
// This file establishes the module structure.
