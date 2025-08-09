//! Shared memory support for inter-process communication

use crate::error::Result;
use crate::types::SharedMemoryFlags;
use std::path::Path;
use tracing::debug;

/// Shared memory structure for CASC storage
#[derive(Debug)]
pub struct SharedMemory {
    /// Version of the shared memory format
    pub version: u32,
    /// Build number of the game
    pub build_number: u32,
    /// Game region (e.g., "US", "EU")
    pub region: [u8; 4],
    /// Status flags
    pub flags: SharedMemoryFlags,
    /// Number of archives
    pub archive_count: u32,
    /// Number of indices
    pub index_count: u32,
    /// Total storage size in bytes
    pub total_size: u64,
    /// Available space in bytes
    pub free_space: u64,
    /// Path to data directory
    pub data_path: String,
}

impl SharedMemory {
    /// Create a new shared memory structure
    pub fn new(data_path: String) -> Self {
        Self {
            version: 1,
            build_number: 0,
            region: [b'U', b'S', 0, 0],
            flags: SharedMemoryFlags {
                is_ready: false,
                is_updating: false,
                needs_repair: false,
            },
            archive_count: 0,
            index_count: 0,
            total_size: 0,
            free_space: 0,
            data_path,
        }
    }

    /// Write shared memory to a file
    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        debug!("Writing shared memory to {:?}", path);

        // For now, we just write a simple JSON representation
        // In production, this would be a binary format or actual shared memory
        let json = serde_json::json!({
            "version": self.version,
            "build_number": self.build_number,
            "region": String::from_utf8_lossy(&self.region),
            "flags": {
                "is_ready": self.flags.is_ready,
                "is_updating": self.flags.is_updating,
                "needs_repair": self.flags.needs_repair,
            },
            "archive_count": self.archive_count,
            "index_count": self.index_count,
            "total_size": self.total_size,
            "free_space": self.free_space,
            "data_path": self.data_path,
        });

        std::fs::write(path, json.to_string())?;
        Ok(())
    }

    /// Read shared memory from a file
    pub fn read_from_file(path: &Path) -> Result<Self> {
        debug!("Reading shared memory from {:?}", path);

        let content = std::fs::read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let region_str = json["region"].as_str().unwrap_or("US");
        let mut region = [0u8; 4];
        region[..region_str.len().min(4)].copy_from_slice(region_str.as_bytes());

        Ok(Self {
            version: json["version"].as_u64().unwrap_or(1) as u32,
            build_number: json["build_number"].as_u64().unwrap_or(0) as u32,
            region,
            flags: SharedMemoryFlags {
                is_ready: json["flags"]["is_ready"].as_bool().unwrap_or(false),
                is_updating: json["flags"]["is_updating"].as_bool().unwrap_or(false),
                needs_repair: json["flags"]["needs_repair"].as_bool().unwrap_or(false),
            },
            archive_count: json["archive_count"].as_u64().unwrap_or(0) as u32,
            index_count: json["index_count"].as_u64().unwrap_or(0) as u32,
            total_size: json["total_size"].as_u64().unwrap_or(0),
            free_space: json["free_space"].as_u64().unwrap_or(0),
            data_path: json["data_path"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Update statistics
    pub fn update_stats(&mut self, archive_count: u32, index_count: u32, total_size: u64) {
        self.archive_count = archive_count;
        self.index_count = index_count;
        self.total_size = total_size;
    }

    /// Mark as ready
    pub fn set_ready(&mut self, ready: bool) {
        self.flags.is_ready = ready;
    }

    /// Mark as updating
    pub fn set_updating(&mut self, updating: bool) {
        self.flags.is_updating = updating;
    }

    /// Mark as needing repair
    pub fn set_needs_repair(&mut self, needs_repair: bool) {
        self.flags.needs_repair = needs_repair;
    }
}

// Add serde_json to dependencies for this module
use serde_json;
