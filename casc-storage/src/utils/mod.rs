//! Utility functions for CASC storage

mod jenkins;
mod shared_memory;

pub use jenkins::jenkins_lookup3;
pub use shared_memory::SharedMemory;
