//! Main CASC storage implementation

use crate::archive::{Archive, ArchiveWriter};
use crate::error::{CascError, Result};
use crate::index::{GroupIndex, IdxParser, IndexFile};
use crate::types::{ArchiveLocation, CascConfig, EKey, StorageStats};
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Main CASC storage implementation
pub struct CascStorage {
    /// Configuration
    config: CascConfig,

    /// Bucket-based indices (0x00-0x0F)
    indices: Arc<DashMap<u8, IndexFile>>,

    /// Archive files
    archives: Arc<RwLock<HashMap<u16, Archive>>>,

    /// LRU cache for decompressed content
    cache: Arc<RwLock<LruCache<EKey, Vec<u8>>>>,

    /// Current archive for writing
    current_archive: Arc<RwLock<Option<ArchiveWriter>>>,

    /// Storage statistics
    stats: Arc<RwLock<StorageStats>>,
}

impl CascStorage {
    /// Create a new CASC storage instance
    pub fn new(config: CascConfig) -> Result<Self> {
        // Create data directories if they don't exist
        let data_path = &config.data_path;
        let indices_path = data_path.join("indices");
        let data_subpath = data_path.join("data");

        std::fs::create_dir_all(&indices_path)?;
        std::fs::create_dir_all(&data_subpath)?;

        let cache_size = NonZeroUsize::new((config.cache_size_mb as usize) * 1024 * 1024)
            .unwrap_or(NonZeroUsize::new(256 * 1024 * 1024).unwrap());

        Ok(Self {
            config,
            indices: Arc::new(DashMap::new()),
            archives: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
            current_archive: Arc::new(RwLock::new(None)),
            stats: Arc::new(RwLock::new(StorageStats::default())),
        })
    }

    /// Load indices from disk
    pub fn load_indices(&self) -> Result<()> {
        info!("Loading CASC indices from {:?}", self.config.data_path);

        let indices_path = self.config.data_path.join("indices");

        // Load .idx files (bucket-based indices)
        for entry in std::fs::read_dir(&indices_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("idx") {
                match IdxParser::parse_file(&path) {
                    Ok(parser) => {
                        let bucket = parser.bucket();
                        debug!(
                            "Loaded .idx file for bucket {:02x}: {} entries",
                            bucket,
                            parser.len()
                        );

                        let mut index = IndexFile::new(crate::index::IndexVersion::V7);
                        for (ekey, location) in parser.entries() {
                            index.add_entry(*ekey, *location);
                        }

                        self.indices.insert(bucket, index);
                    }
                    Err(e) => {
                        warn!("Failed to load index {:?}: {}", path, e);
                    }
                }
            }
        }

        // Load .index files (group indices)
        for entry in std::fs::read_dir(&indices_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("index") {
                match GroupIndex::parse_file(&path) {
                    Ok(group) => {
                        let bucket = group.bucket_index();
                        debug!(
                            "Loaded .index file for bucket {:02x}: {} entries",
                            bucket,
                            group.len()
                        );

                        // Merge with existing index or create new
                        self.indices
                            .entry(bucket)
                            .and_modify(|index| {
                                for (ekey, location) in group.entries() {
                                    index.add_entry(*ekey, *location);
                                }
                            })
                            .or_insert_with(|| {
                                let mut index = IndexFile::new(crate::index::IndexVersion::V7);
                                for (ekey, location) in group.entries() {
                                    index.add_entry(*ekey, *location);
                                }
                                index
                            });
                    }
                    Err(e) => {
                        warn!("Failed to load group index {:?}: {}", path, e);
                    }
                }
            }
        }

        info!("Loaded {} bucket indices", self.indices.len());
        Ok(())
    }

    /// Load archive files
    pub fn load_archives(&self) -> Result<()> {
        info!("Loading CASC archives from {:?}", self.config.data_path);

        let data_path = self.config.data_path.join("data");
        let mut archives = self.archives.write();

        for entry in std::fs::read_dir(&data_path)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            if filename.starts_with("data.") {
                // Extract archive ID from filename (data.XXX)
                if let Some(id_str) = filename.strip_prefix("data.") {
                    if let Ok(id) = id_str.parse::<u16>() {
                        match Archive::new(id, path.clone()) {
                            Ok(archive) => {
                                debug!("Loaded archive {}: size={}", id, archive.size);
                                archives.insert(id, archive);
                            }
                            Err(e) => {
                                warn!("Failed to load archive {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        info!("Loaded {} archives", archives.len());
        Ok(())
    }

    /// Read a file by its encoding key
    pub fn read(&self, ekey: &EKey) -> Result<Vec<u8>> {
        // Check cache first
        {
            let mut cache = self.cache.write();
            if let Some(data) = cache.get(ekey) {
                debug!("Cache hit for {}", ekey);
                return Ok(data.clone());
            }
        }

        // Find the file in indices
        let bucket = ekey.bucket_index();
        let location = self
            .indices
            .get(&bucket)
            .and_then(|index| index.lookup(ekey).copied())
            .ok_or_else(|| CascError::EntryNotFound(ekey.to_string()))?;

        debug!(
            "Found {} in archive {} at offset {:x}",
            ekey, location.archive_id, location.offset
        );

        // Read from archive
        let compressed_data = {
            let mut archives = self.archives.write();
            let archive = archives
                .get_mut(&location.archive_id)
                .ok_or(CascError::ArchiveNotFound(location.archive_id))?;

            archive.read_at(&location)?
        };

        // Decompress using BLTE (no key service needed for now)
        let decompressed = blte::decompress_blte(compressed_data, None)?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.put(*ekey, decompressed.clone());
        }

        Ok(decompressed)
    }

    /// Write a file with the given encoding key
    pub fn write(&self, ekey: &EKey, data: &[u8]) -> Result<()> {
        if self.config.read_only {
            return Err(CascError::ReadOnly);
        }

        // Check if already exists
        let bucket = ekey.bucket_index();
        if let Some(index) = self.indices.get(&bucket) {
            if index.lookup(ekey).is_some() {
                debug!("File {} already exists, skipping write", ekey);
                return Ok(());
            }
        }

        // Compress data using BLTE
        let compressed =
            blte::compress_data_single(data.to_vec(), blte::CompressionMode::ZLib, None)?;

        // Get or create current archive
        let location = self.write_to_archive(&compressed)?;

        // Update index
        self.indices
            .entry(bucket)
            .or_insert_with(|| IndexFile::new(crate::index::IndexVersion::V7))
            .add_entry(*ekey, location);

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.put(*ekey, data.to_vec());
        }

        debug!(
            "Wrote {} to archive {} at offset {:x}",
            ekey, location.archive_id, location.offset
        );
        Ok(())
    }

    /// Write compressed data to the current archive
    fn write_to_archive(&self, data: &[u8]) -> Result<ArchiveLocation> {
        let mut current_archive = self.current_archive.write();

        // Check if we need a new archive
        if current_archive.is_none()
            || current_archive.as_ref().unwrap().current_offset() + data.len() as u64
                > self.config.max_archive_size
        {
            // Create new archive
            let archive_id = self.get_next_archive_id();
            let archive_path = self
                .config
                .data_path
                .join("data")
                .join(format!("data.{archive_id:03}"));

            *current_archive = Some(ArchiveWriter::create(&archive_path, archive_id)?);

            // Register the new archive
            let mut archives = self.archives.write();
            archives.insert(archive_id, Archive::new(archive_id, archive_path)?);
        }

        let writer = current_archive.as_mut().unwrap();
        let offset = writer.write(data)?;

        Ok(ArchiveLocation {
            archive_id: writer.archive_id(),
            offset,
            size: data.len() as u32,
        })
    }

    /// Get the next available archive ID
    fn get_next_archive_id(&self) -> u16 {
        let archives = self.archives.read();
        archives.keys().max().map(|id| id + 1).unwrap_or(0)
    }

    /// Verify storage integrity
    pub fn verify(&self) -> Result<Vec<EKey>> {
        info!("Verifying CASC storage integrity");
        let mut errors = Vec::new();

        for index_ref in self.indices.iter() {
            let index = index_ref.value();
            for (ekey, _location) in index.entries() {
                // Try to read the file
                match self.read(ekey) {
                    Ok(_) => {
                        // Successfully read and decompressed
                    }
                    Err(e) => {
                        warn!("Verification failed for {}: {}", ekey, e);
                        errors.push(*ekey);
                    }
                }
            }
        }

        if errors.is_empty() {
            info!("Storage verification complete: all files OK");
        } else {
            warn!("Storage verification found {} errors", errors.len());
        }

        Ok(errors)
    }

    /// Build indices from scratch by scanning archives
    pub fn rebuild_indices(&self) -> Result<()> {
        if self.config.read_only {
            return Err(CascError::ReadOnly);
        }

        info!("Rebuilding CASC indices");

        // Clear existing indices
        self.indices.clear();

        // Scan all archives
        let archives = self.archives.read();
        for (_id, archive) in archives.iter() {
            // This would require parsing the archive format
            // For now, this is a placeholder
            warn!(
                "Archive scanning not yet implemented for {:?}",
                archive.path()
            );
        }

        Ok(())
    }

    /// Get storage statistics
    pub fn stats(&self) -> StorageStats {
        let stats = self.stats.read();
        StorageStats {
            total_archives: stats.total_archives,
            total_indices: stats.total_indices,
            total_size: stats.total_size,
            file_count: stats.file_count,
            duplicate_count: stats.duplicate_count,
            compression_ratio: stats.compression_ratio,
        }
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
        debug!("Cache cleared");
    }

    /// Flush any pending writes
    pub fn flush(&self) -> Result<()> {
        if let Some(writer) = self.current_archive.write().as_mut() {
            writer.flush()?;
        }
        Ok(())
    }
}
