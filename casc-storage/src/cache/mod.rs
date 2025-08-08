//! Cache implementations for CASC storage

mod lockfree_cache;

pub use lockfree_cache::{CacheStats, LockFreeCache};
