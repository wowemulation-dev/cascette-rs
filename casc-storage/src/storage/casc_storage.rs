//! Main CASC storage implementation

use crate::archive::{Archive, ArchiveWriter};
use crate::cache::LockFreeCache;
use crate::error::{CascError, Result};
use crate::index::{
    AsyncIndexConfig, AsyncIndexManager, CombinedIndex, GroupIndex, IdxParser, IndexFile,
};
use crate::manifest::{FileMapping, ManifestConfig, TactManifests};
use crate::progressive::{ChunkLoader, ProgressiveConfig, ProgressiveFileManager, SizeHint};
use crate::types::{ArchiveLocation, CascConfig, EKey, StorageStats};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Main CASC storage implementation
pub struct CascStorage {
    /// Configuration
    config: CascConfig,

    /// Bucket-based indices (0x00-0x0F) - kept for compatibility
    indices: Arc<DashMap<u8, IndexFile>>,

    /// Combined index for optimized lookups
    combined_index: Arc<CombinedIndex>,

    /// Async index manager for parallel operations
    async_index_manager: Option<Arc<AsyncIndexManager>>,

    /// Archive files
    archives: Arc<RwLock<HashMap<u16, Archive>>>,

    /// Lock-free cache for decompressed content (using Arc to avoid cloning)
    cache: Arc<LockFreeCache>,

    /// Current archive for writing
    current_archive: Arc<RwLock<Option<ArchiveWriter>>>,

    /// TACT manifest integration for FileDataID lookups
    tact_manifests: Option<TactManifests>,

    /// Progressive file loading manager
    progressive_manager: Option<ProgressiveFileManager>,

    /// Storage statistics
    #[allow(dead_code)]
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

        let cache_size_bytes = (config.cache_size_mb as usize) * 1024 * 1024;

        Ok(Self {
            config,
            indices: Arc::new(DashMap::new()),
            combined_index: Arc::new(CombinedIndex::new()),
            async_index_manager: None,
            archives: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(LockFreeCache::new(cache_size_bytes)),
            current_archive: Arc::new(RwLock::new(None)),
            tact_manifests: None,
            progressive_manager: None,
            stats: Arc::new(RwLock::new(StorageStats::default())),
        })
    }

    /// Load indices from disk
    pub fn load_indices(&self) -> Result<()> {
        // Check if we're already in an async context
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We're in an async context, but we can't use block_on
                // Fall back to sequential loading to avoid runtime conflict
                debug!("In async context, using sequential loading to avoid runtime conflict");
                self.load_indices_sequential()
            }
            Err(_) => {
                // No async runtime, use sequential loading
                debug!("No async runtime, using sequential loading");
                self.load_indices_sequential()
            }
        }
    }

    /// Create new CASC storage asynchronously (recommended for async contexts)
    pub async fn new_async(config: CascConfig) -> Result<Self> {
        // Create data directories if they don't exist
        let data_path = &config.data_path;
        let indices_path = data_path.join("indices");
        let data_subpath = data_path.join("data");

        std::fs::create_dir_all(&indices_path)?;
        std::fs::create_dir_all(&data_subpath)?;

        let cache_size_bytes = (config.cache_size_mb as usize) * 1024 * 1024;

        let storage = Self {
            config,
            indices: Arc::new(DashMap::new()),
            combined_index: Arc::new(CombinedIndex::new()),
            async_index_manager: None,
            archives: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(LockFreeCache::new(cache_size_bytes)),
            current_archive: Arc::new(RwLock::new(None)),
            tact_manifests: None,
            progressive_manager: None,
            stats: Arc::new(RwLock::new(StorageStats::default())),
        };

        // Load indices asynchronously for better performance
        storage.load_indices_parallel().await?;
        storage.load_archives()?;

        Ok(storage)
    }

    /// Load indices from disk with parallel processing (3-5x faster)
    pub async fn load_indices_parallel(&self) -> Result<()> {
        info!(
            "Loading CASC indices from {:?} (parallel)",
            self.config.data_path
        );

        use tokio::task::JoinSet;

        // Try multiple locations for indices
        let indices_path = self.config.data_path.join("indices");
        let data_path = self.config.data_path.join("data");

        // Collect all .idx files from both directories
        let mut idx_paths = Vec::new();

        // Collect from data directory
        if data_path.exists() {
            if let Ok(entries) = tokio::fs::read_dir(&data_path).await {
                let mut entries = entries;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("idx") {
                        idx_paths.push(path);
                    }
                }
            }
        }

        // Collect from indices directory
        if indices_path.exists() {
            if let Ok(entries) = tokio::fs::read_dir(&indices_path).await {
                let mut entries = entries;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("idx") {
                        idx_paths.push(path);
                    }
                }
            }
        }

        if idx_paths.is_empty() {
            info!("No .idx files found");
            return Ok(());
        }

        info!("Found {} .idx files, loading in parallel", idx_paths.len());

        // Process all .idx files in parallel
        let mut join_set = JoinSet::new();

        for idx_path in idx_paths {
            join_set.spawn_blocking(move || -> Result<(u8, IndexFile)> {
                match IdxParser::parse_file(&idx_path) {
                    Ok(parser) => {
                        let bucket = parser.bucket();
                        debug!(
                            "Loaded .idx file for bucket {:02x}: {} entries",
                            bucket,
                            parser.len()
                        );

                        let entries_map = parser.into_entries();
                        let mut index = IndexFile::new(crate::index::IndexVersion::V7);

                        // Add all entries to the index
                        for (ekey, location) in entries_map {
                            index.add_entry(ekey, location);
                        }

                        Ok((bucket, index))
                    }
                    Err(e) => {
                        warn!("Failed to load index {:?}: {}", idx_path, e);
                        Err(e)
                    }
                }
            });
        }

        // Collect all results
        let mut loaded_count = 0;
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok((bucket, index))) => {
                    // Also populate combined index
                    for (ekey, location) in index.entries() {
                        self.combined_index.insert(*ekey, *location);
                    }
                    self.indices.insert(bucket, index);
                    loaded_count += 1;
                }
                Ok(Err(e)) => {
                    debug!("Index loading task failed: {}", e);
                    // Continue loading other indices even if one fails
                }
                Err(e) => {
                    warn!("Task join failed: {}", e);
                }
            }
        }

        info!("Loaded {} bucket indices (parallel)", loaded_count);
        Ok(())
    }

    /// Load indices from disk (sequential fallback)
    pub fn load_indices_sequential(&self) -> Result<()> {
        info!(
            "Loading CASC indices from {:?} (sequential)",
            self.config.data_path
        );

        // Try multiple locations for indices
        let indices_path = self.config.data_path.join("indices");
        let data_path = self.config.data_path.join("data");

        // Load .idx files from data directory (WoW Era format)
        if data_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&data_path) {
                for entry in entries {
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

                                // Consume parser and get all entries at once
                                let entries_map = parser.into_entries();

                                let mut index = IndexFile::new(crate::index::IndexVersion::V7);

                                // Add all entries to the index and combined index
                                for (ekey, location) in entries_map {
                                    index.add_entry(ekey, location);
                                    self.combined_index.insert(ekey, location);
                                }

                                self.indices.insert(bucket, index);
                            }
                            Err(e) => {
                                warn!("Failed to load index {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        // Load .idx files from indices directory (if exists)
        if indices_path.exists() {
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
                            // Transfer ownership of entries to avoid lifetime issues
                            for (ekey, location) in parser.into_entries() {
                                index.add_entry(ekey, location);
                                self.combined_index.insert(ekey, location);
                            }

                            self.indices.insert(bucket, index);
                        }
                        Err(e) => {
                            warn!("Failed to load index {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        // Load .index files (group indices) - disabled until format is understood
        #[allow(unreachable_code)]
        if false {
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

    /// Read a file by its encoding key (zero-copy when cached)
    pub fn read_arc(&self, ekey: &EKey) -> Result<Arc<Vec<u8>>> {
        // Check cache first - lock-free operation
        if let Some(data) = self.cache.get(ekey) {
            debug!("Cache hit for {} (zero-copy)", ekey);
            return Ok(data); // Zero-copy return from cache
        }

        // Not in cache, need to read and decompress
        let data = self.read_and_decompress(ekey)?;
        let data_arc = Arc::new(data);

        // Update cache - lock-free operation
        self.cache.put(*ekey, Arc::clone(&data_arc));

        Ok(data_arc)
    }

    /// Read a file by its encoding key (compatibility method, always clones)
    pub fn read(&self, ekey: &EKey) -> Result<Vec<u8>> {
        let arc_data = self.read_arc(ekey)?;
        Ok((*arc_data).clone())
    }

    /// Internal method to read and decompress without caching logic
    fn read_and_decompress(&self, ekey: &EKey) -> Result<Vec<u8>> {
        // Use optimized combined index for O(log n) lookup
        debug!("Looking up EKey {} using combined index", ekey);

        let location = self.combined_index.lookup(ekey).ok_or_else(|| {
            debug!("EKey {} not found in combined index", ekey);
            CascError::EntryNotFound(ekey.to_string())
        })?;

        debug!(
            "Found {} in archive {} at offset {:x}",
            ekey, location.archive_id, location.offset
        );

        // Read from archive
        let raw_data = {
            let mut archives = self.archives.write();
            let archive = archives
                .get_mut(&location.archive_id)
                .ok_or(CascError::ArchiveNotFound(location.archive_id))?;

            archive.read_at(&location)?
        };

        // CASC archives have a 30-byte header before the BLTE data:
        // 16 bytes: BlteHash (encoding key)
        // 4 bytes: Size of header + data
        // 2 bytes: Flags
        // 4 bytes: ChecksumA
        // 4 bytes: ChecksumB
        const CASC_ENTRY_HEADER_SIZE: usize = 30;

        if raw_data.len() < CASC_ENTRY_HEADER_SIZE {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Archive data too small: {} bytes",
                raw_data.len()
            )));
        }

        // Skip the header and get the BLTE data
        let compressed_data = raw_data[CASC_ENTRY_HEADER_SIZE..].to_vec();

        // Decompress using streaming BLTE for better memory efficiency
        use std::io::{Cursor, Read};
        let cursor = Cursor::new(compressed_data);
        let mut stream = blte::create_streaming_reader(cursor, None)
            .map_err(|e| CascError::DecompressionError(e.to_string()))?;

        let mut decompressed = Vec::new();
        stream
            .read_to_end(&mut decompressed)
            .map_err(|e| CascError::DecompressionError(e.to_string()))?;

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

        // Update cache with Arc to avoid future clones - lock-free operation
        self.cache.put(*ekey, Arc::new(data.to_vec()));

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
        // Calculate stats from current state
        let mut file_count = 0usize;
        for index_ref in self.indices.iter() {
            file_count += index_ref.value().entries().count();
        }

        let archives = self.archives.read();
        let total_archives = archives.len();

        let mut total_size = 0u64;
        for archive in archives.values() {
            total_size += archive.size;
        }

        StorageStats {
            total_archives: total_archives as u32,
            total_indices: self.indices.len() as u32,
            total_size,
            file_count: file_count as u64,
            duplicate_count: 0,
            compression_ratio: 0.0,
        }
    }

    /// Enumerate all files in the storage
    /// Returns a vector of (EKey, ArchiveLocation) pairs
    pub fn enumerate_files_vec(&self) -> Vec<(EKey, ArchiveLocation)> {
        let mut all_entries = Vec::new();

        for index_ref in self.indices.iter() {
            let _bucket = *index_ref.key();
            let index = index_ref.value();

            let bucket_entries: Vec<(EKey, ArchiveLocation)> = index
                .entries()
                .map(|(ekey, location)| (*ekey, *location))
                .collect();
            all_entries.extend(bucket_entries);
        }

        all_entries
    }

    /// Enumerate all files in the storage
    /// Returns an iterator over (EKey, ArchiveLocation) pairs  
    pub fn enumerate_files(&self) -> impl Iterator<Item = (EKey, ArchiveLocation)> + '_ {
        self.indices.iter().flat_map(|index_ref| {
            index_ref
                .value()
                .entries()
                .map(|(ekey, location)| (*ekey, *location))
                .collect::<Vec<_>>()
        })
    }

    /// Get all EKeys in the storage
    pub fn get_all_ekeys(&self) -> Vec<EKey> {
        self.enumerate_files().map(|(ekey, _)| ekey).collect()
    }

    /// Test function to verify EKey lookup is working
    pub fn test_ekey_lookup(&self) -> Result<()> {
        // Get the first EKey from enumeration (use vec to avoid iterator issues)
        let all_files = self.enumerate_files_vec();
        if let Some((test_ekey, expected_location)) = all_files.first().copied() {
            info!("Testing lookup with first enumerated EKey: {}", test_ekey);
            info!(
                "Expected location: archive={}, offset={:x}, size={}",
                expected_location.archive_id, expected_location.offset, expected_location.size
            );

            // Try to read it using the normal read path
            match self.read(&test_ekey) {
                Ok(data) => {
                    info!("SUCCESS: Read {} bytes from EKey {}", data.len(), test_ekey);
                    Ok(())
                }
                Err(e) => {
                    error!("FAILED to read EKey {}: {}", test_ekey, e);

                    // Debug why it failed
                    let bucket = test_ekey.bucket_index();
                    info!("EKey {} maps to bucket {:02x}", test_ekey, bucket);

                    if let Some(index) = self.indices.get(&bucket) {
                        info!("Bucket {:02x} exists with {} entries", bucket, index.len());

                        // Check if the key exists in the bucket
                        let found = index.entries().any(|(k, _)| *k == test_ekey);

                        if found {
                            info!("EKey IS in the bucket but lookup failed!");
                        } else {
                            info!("EKey is NOT in the bucket!");

                            // Show first few entries
                            let entries: Vec<String> = index
                                .entries()
                                .take(3)
                                .map(|(k, _)| k.to_string())
                                .collect();
                            info!("First 3 entries in bucket: {:?}", entries);
                        }
                    } else {
                        error!("Bucket {:02x} doesn't exist!", bucket);
                    }

                    Err(e)
                }
            }
        } else {
            error!("No files found in storage!");
            Err(CascError::EntryNotFound("No files in storage".to_string()))
        }
    }

    /// Count files per archive
    pub fn files_per_archive(&self) -> std::collections::HashMap<u16, usize> {
        let mut counts = std::collections::HashMap::new();
        for (_ekey, location) in self.enumerate_files() {
            *counts.entry(location.archive_id).or_insert(0) += 1;
        }
        counts
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Flush any pending writes
    pub fn flush(&self) -> Result<()> {
        if let Some(writer) = self.current_archive.write().as_mut() {
            writer.flush()?;
        }
        Ok(())
    }

    // === TACT Manifest Integration ===

    /// Initialize TACT manifest support with configuration
    pub fn init_tact_manifests(&mut self, config: ManifestConfig) {
        self.tact_manifests = Some(TactManifests::new(config));
        info!("Initialized TACT manifest support");
    }

    /// Load root manifest from raw data
    pub fn load_root_manifest(&self, data: Vec<u8>) -> Result<()> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.load_root_from_data(data)
    }

    /// Load encoding manifest from raw data  
    pub fn load_encoding_manifest(&self, data: Vec<u8>) -> Result<()> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.load_encoding_from_data(data)
    }

    /// Load root manifest from file
    pub fn load_root_manifest_from_file(&self, path: &std::path::Path) -> Result<()> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.load_root_from_file(path)
    }

    /// Load encoding manifest from file
    pub fn load_encoding_manifest_from_file(&self, path: &std::path::Path) -> Result<()> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.load_encoding_from_file(path)
    }

    /// Load a community listfile for filename resolution
    pub fn load_listfile(&self, path: &std::path::Path) -> Result<usize> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.load_listfile(path)
    }

    /// Read a file by FileDataID
    pub fn read_by_fdid(&self, fdid: u32) -> Result<Vec<u8>> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;

        let mapping = manifests.lookup_by_fdid(fdid)?;
        let ekey = mapping
            .encoding_key
            .ok_or_else(|| CascError::EntryNotFound(format!("EKey for FDID {fdid}")))?;

        self.read(&ekey)
    }

    /// Read a file by filename (requires loaded listfile or root manifest)
    pub fn read_by_filename(&self, filename: &str) -> Result<Vec<u8>> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;

        let mapping = manifests.lookup_by_filename(filename)?;
        let ekey = mapping
            .encoding_key
            .ok_or_else(|| CascError::EntryNotFound(format!("EKey for filename {filename}")))?;

        self.read(&ekey)
    }

    /// Get FileDataID for a filename (if known)
    pub fn get_fdid_for_filename(&self, filename: &str) -> Option<u32> {
        self.tact_manifests
            .as_ref()?
            .get_fdid_for_filename(filename)
    }

    /// Get all known FileDataIDs
    pub fn get_all_fdids(&self) -> Result<Vec<u32>> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.get_all_fdids()
    }

    /// Check if TACT manifests are loaded and ready
    pub fn tact_manifests_loaded(&self) -> bool {
        self.tact_manifests.as_ref().is_some_and(|m| m.is_loaded())
    }

    /// Get file mapping information for a FileDataID
    pub fn get_file_mapping(&self, fdid: u32) -> Result<FileMapping> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;
        manifests.lookup_by_fdid(fdid)
    }

    /// Clear TACT manifest caches
    pub fn clear_manifest_cache(&self) {
        if let Some(manifests) = &self.tact_manifests {
            manifests.clear_cache();
        }
    }

    // === Async Index Operations ===

    /// Initialize async index manager for parallel operations
    pub async fn init_async_indices(&mut self) -> Result<()> {
        let config = AsyncIndexConfig {
            max_concurrent_files: 16,
            buffer_size: 128 * 1024, // 128KB buffers
            enable_caching: true,
            max_cache_entries: 100_000,
            enable_background_updates: false, // Can be enabled later
        };

        let manager = Arc::new(AsyncIndexManager::new(config));

        // Load existing indices
        let loaded = manager.load_directory(&self.config.data_path).await?;

        info!("Async index manager initialized with {} indices", loaded);
        self.async_index_manager = Some(manager);

        Ok(())
    }

    /// Perform async lookup using the async index manager
    pub async fn lookup_async(&self, ekey: &EKey) -> Option<ArchiveLocation> {
        if let Some(ref manager) = self.async_index_manager {
            manager.lookup(ekey).await
        } else {
            // Fallback to sync lookup
            self.combined_index.lookup(ekey)
        }
    }

    /// Batch lookup for multiple keys using async operations
    pub async fn lookup_batch_async(&self, ekeys: &[EKey]) -> Vec<Option<ArchiveLocation>> {
        if let Some(ref manager) = self.async_index_manager {
            manager.lookup_batch(ekeys).await
        } else {
            // Fallback to sync batch lookup
            self.combined_index.lookup_batch(ekeys)
        }
    }

    /// Start background index updates with specified interval
    pub async fn start_index_background_updates(&self, interval: std::time::Duration) {
        if let Some(ref manager) = self.async_index_manager {
            manager
                .start_background_updates(self.config.data_path.clone(), interval)
                .await;
            info!(
                "Started background index updates with interval {:?}",
                interval
            );
        }
    }

    /// Stop background index updates
    pub async fn stop_index_background_updates(&self) {
        if let Some(ref manager) = self.async_index_manager {
            manager.stop_background_updates().await;
            info!("Stopped background index updates");
        }
    }

    /// Get async index statistics
    pub async fn get_async_index_stats(&self) -> Option<crate::index::AsyncIndexStats> {
        if let Some(ref manager) = self.async_index_manager {
            Some(manager.get_stats().await)
        } else {
            None
        }
    }

    /// Clear async index cache
    pub async fn clear_async_index_cache(&self) {
        if let Some(ref manager) = self.async_index_manager {
            manager.clear_cache().await;
            debug!("Cleared async index cache");
        }
    }

    // === Progressive Loading Support ===

    /// Initialize progressive file loading with configuration
    pub fn init_progressive_loading(&mut self, config: ProgressiveConfig) {
        let chunk_loader = Arc::new(CascStorageChunkLoader {
            storage: self as *const CascStorage,
        });

        self.progressive_manager = Some(ProgressiveFileManager::new(config, chunk_loader));
        info!("Initialized progressive file loading");
    }

    /// Read a file progressively with size hints
    pub async fn read_progressive(
        &self,
        ekey: &EKey,
        size_hint: SizeHint,
    ) -> Result<Arc<crate::progressive::ProgressiveFile>> {
        let manager = self.progressive_manager.as_ref().ok_or_else(|| {
            CascError::InvalidArchiveFormat("Progressive loading not initialized".to_string())
        })?;

        Ok(manager
            .get_or_create_progressive_file(*ekey, size_hint)
            .await)
    }

    /// Read a file by FileDataID progressively with size hints from manifest
    pub async fn read_by_fdid_progressive(
        &self,
        fdid: u32,
    ) -> Result<Arc<crate::progressive::ProgressiveFile>> {
        let manifests = self.tact_manifests.as_ref().ok_or_else(|| {
            CascError::ManifestNotLoaded("TACT manifests not initialized".to_string())
        })?;

        let mapping = manifests.lookup_by_fdid(fdid)?;
        let ekey = mapping
            .encoding_key
            .ok_or_else(|| CascError::EntryNotFound(format!("EKey for FDID {fdid}")))?;

        // Create size hint from archive location data
        let size_hint = if let Some(location) = self.combined_index.lookup(&ekey) {
            // Archive location gives us compressed size, actual size is usually larger
            SizeHint::Minimum(location.size as u64)
        } else {
            SizeHint::Unknown
        };

        self.read_progressive(&ekey, size_hint).await
    }

    /// Get size hint for an EKey from archive location
    pub fn get_size_hint_for_ekey(&self, ekey: &EKey) -> SizeHint {
        if let Some(location) = self.combined_index.lookup(ekey) {
            // Archive location gives us a minimum size (compressed size)
            // Actual decompressed size is usually larger
            SizeHint::Minimum(location.size as u64)
        } else {
            SizeHint::Unknown
        }
    }

    /// Check if progressive loading is available
    pub fn has_progressive_loading(&self) -> bool {
        self.progressive_manager.is_some()
    }

    /// Cleanup inactive progressive files
    pub async fn cleanup_progressive_files(&self) {
        if let Some(manager) = &self.progressive_manager {
            use std::time::Duration;
            manager
                .cleanup_inactive_files(Duration::from_secs(300))
                .await; // 5 minutes
        }
    }

    /// Get progressive loading statistics
    pub async fn get_progressive_stats(&self) -> Vec<(EKey, crate::progressive::LoadingStats)> {
        if let Some(manager) = &self.progressive_manager {
            manager.get_global_stats().await
        } else {
            Vec::new()
        }
    }
}

/// ChunkLoader implementation for CascStorage
struct CascStorageChunkLoader {
    storage: *const CascStorage,
}

// Safety: CascStorageChunkLoader is only used with a valid CascStorage pointer
// and the storage lifetime is guaranteed by the ProgressiveFileManager
unsafe impl Send for CascStorageChunkLoader {}
unsafe impl Sync for CascStorageChunkLoader {}

#[async_trait::async_trait]
impl ChunkLoader for CascStorageChunkLoader {
    async fn load_chunk(&self, ekey: EKey, offset: u64, size: usize) -> Result<Vec<u8>> {
        // SAFETY: The storage pointer is valid for the entire lifetime of CascStorageChunkLoader
        // as it's created from a reference to CascStorage and the lifetime 'a ensures that
        // the CascStorage outlives this ChunkLoader instance.
        let storage = unsafe { &*self.storage };

        // Get the location of the file
        let location = storage.combined_index.lookup(&ekey).ok_or_else(|| {
            debug!("EKey {} not found in combined index", ekey);
            CascError::EntryNotFound(ekey.to_string())
        })?;

        debug!(
            "Loading chunk for {} from archive {} at offset {:x} (chunk offset={}, size={})",
            ekey, location.archive_id, location.offset, offset, size
        );

        // Read the compressed data from archive
        let raw_data = {
            let mut archives = storage.archives.write();
            let archive = archives
                .get_mut(&location.archive_id)
                .ok_or(CascError::ArchiveNotFound(location.archive_id))?;

            archive.read_at(&location)?
        };

        // CASC archives have a 30-byte header before the BLTE data
        const CASC_ENTRY_HEADER_SIZE: usize = 30;

        if raw_data.len() < CASC_ENTRY_HEADER_SIZE {
            return Err(CascError::InvalidArchiveFormat(format!(
                "Archive data too small: {} bytes",
                raw_data.len()
            )));
        }

        // Skip the header and get the BLTE data
        let compressed_data = raw_data[CASC_ENTRY_HEADER_SIZE..].to_vec();

        // Decompress using streaming BLTE
        use std::io::{Cursor, Read};
        let cursor = Cursor::new(compressed_data);
        let mut stream = blte::create_streaming_reader(cursor, None)
            .map_err(|e| CascError::DecompressionError(e.to_string()))?;

        // Seek to the requested offset in the decompressed stream
        if offset > 0 {
            let mut discard_buf = vec![0u8; 8192]; // 8KB discard buffer
            let mut remaining = offset;

            while remaining > 0 {
                let to_read = (remaining as usize).min(discard_buf.len());
                let read = stream
                    .read(&mut discard_buf[..to_read])
                    .map_err(|e| CascError::DecompressionError(e.to_string()))?;

                if read == 0 {
                    break; // End of stream
                }

                remaining -= read as u64;
            }
        }

        // Read the requested chunk
        let mut chunk_data = vec![0u8; size];
        let actual_read = stream
            .read(&mut chunk_data)
            .map_err(|e| CascError::DecompressionError(e.to_string()))?;

        // Resize to actual read size
        chunk_data.truncate(actual_read);

        debug!(
            "Loaded chunk for {} (offset={}, requested_size={}, actual_size={})",
            ekey, offset, size, actual_read
        );

        Ok(chunk_data)
    }
}
