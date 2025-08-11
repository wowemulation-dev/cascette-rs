//! Support for loose files (individual files not in archives)

use crate::error::{CascError, Result};
use crate::types::EKey;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::PathBuf;
use tracing::{debug, info};

/// Storage for loose files (not in archives)
pub struct LooseFileStorage {
    /// Mapping from EKey to file path
    files: HashMap<EKey, PathBuf>,
    /// Base directory for loose files
    base_path: PathBuf,
}

impl LooseFileStorage {
    /// Parse a hex string filename into an EKey
    fn parse_hex_filename(filename: &str) -> Option<EKey> {
        if filename.len() != 32 {
            return None;
        }

        let mut bytes = [0u8; 16];
        for i in 0..16 {
            let hex_pair = &filename[i * 2..i * 2 + 2];
            bytes[i] = u8::from_str_radix(hex_pair, 16).ok()?;
        }

        EKey::from_slice(&bytes)
    }

    /// Create a new loose file storage
    pub fn new(base_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            files: HashMap::new(),
            base_path,
        })
    }

    /// Scan directory for loose files
    pub fn scan(&mut self) -> Result<()> {
        info!("Scanning for loose files in {:?}", self.base_path);

        self.files.clear();

        // Look for files with EKey-based names
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && let Some(filename) = path.file_stem().and_then(|s| s.to_str())
            {
                // Try to parse filename as hex EKey
                if filename.len() == 32
                    && let Some(ekey) = Self::parse_hex_filename(filename)
                {
                    debug!("Found loose file: {}", ekey);
                    self.files.insert(ekey, path);
                }
            }
        }

        info!("Found {} loose files", self.files.len());
        Ok(())
    }

    /// Read a loose file
    pub fn read(&self, ekey: &EKey) -> Result<Vec<u8>> {
        let path = self
            .files
            .get(ekey)
            .ok_or_else(|| CascError::EntryNotFound(ekey.to_string()))?;

        debug!("Reading loose file {} from {:?}", ekey, path);

        // Use streaming approach to avoid loading file twice
        let mut file = std::fs::File::open(path)?;

        // Check if file is BLTE compressed by reading magic
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;

        if magic == blte::BLTE_MAGIC {
            debug!(
                "Loose file {} is BLTE compressed, using streaming decompression",
                ekey
            );
            // Seek back to beginning for BLTE parser
            file.seek(std::io::SeekFrom::Start(0))?;

            // Create streaming BLTE reader
            let mut stream = blte::create_streaming_reader(file, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            let mut result = Vec::new();
            stream
                .read_to_end(&mut result)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;
            Ok(result)
        } else {
            debug!("Loose file {} is uncompressed", ekey);
            // Seek back to beginning and read entire file
            file.seek(std::io::SeekFrom::Start(0))?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            Ok(data)
        }
    }

    /// Write a loose file
    pub fn write(&mut self, ekey: &EKey, data: &[u8], compress: bool) -> Result<()> {
        let filename = format!("{ekey}");
        let path = self.base_path.join(&filename);

        debug!("Writing loose file {} to {:?}", ekey, path);

        // Optionally compress
        let output = if compress {
            blte::compress_data_single(data.to_vec(), blte::CompressionMode::ZLib, None)?
        } else {
            data.to_vec()
        };

        std::fs::write(&path, output)?;
        self.files.insert(*ekey, path);

        Ok(())
    }

    /// Remove a loose file
    pub fn remove(&mut self, ekey: &EKey) -> Result<()> {
        if let Some(path) = self.files.remove(ekey) {
            debug!("Removing loose file {} at {:?}", ekey, path);
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Check if a loose file exists
    pub fn contains(&self, ekey: &EKey) -> bool {
        self.files.contains_key(ekey)
    }

    /// Get the number of loose files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Iterate over all loose files
    pub fn iter(&self) -> impl Iterator<Item = (&EKey, &PathBuf)> {
        self.files.iter()
    }

    /// Get total size of all loose files
    pub fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;
        for path in self.files.values() {
            total += std::fs::metadata(path)?.len();
        }
        Ok(total)
    }
}
