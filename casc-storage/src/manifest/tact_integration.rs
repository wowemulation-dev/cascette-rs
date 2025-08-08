//! TACT manifest integration implementation

use crate::error::{CascError, Result};
use crate::types::EKey;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;
use tact_parser::{
    encoding::EncodingFile,
    wow_root::{ContentFlags, LocaleFlags, WowRoot},
};
use tracing::{debug, info};

/// Configuration for manifest loading
#[derive(Debug, Clone)]
pub struct ManifestConfig {
    /// Locale to use for filtering files
    pub locale: LocaleFlags,
    /// Content flags to require (e.g., Windows, x86_64)
    pub content_flags: Option<ContentFlags>,
    /// Whether to cache manifests in memory
    pub cache_manifests: bool,
    /// Enable lazy loading for large manifest sections
    pub lazy_loading: bool,
    /// Maximum number of entries to cache when lazy loading
    pub lazy_cache_limit: usize,
}

impl Default for ManifestConfig {
    fn default() -> Self {
        Self {
            locale: LocaleFlags::any_locale(),
            content_flags: None,
            cache_manifests: true,
            lazy_loading: true,
            lazy_cache_limit: 10_000,
        }
    }
}

/// Represents a file mapping from FileDataID to EKey
#[derive(Debug, Clone)]
pub struct FileMapping {
    /// FileDataID (game's internal file identifier)
    pub file_data_id: u32,
    /// Content key (MD5 hash from root manifest)
    pub content_key: [u8; 16],
    /// Encoding key (from encoding manifest)
    pub encoding_key: Option<EKey>,
    /// Content flags for this file
    pub flags: Option<ContentFlags>,
}

/// Lazy-loaded root manifest for memory efficiency
#[allow(dead_code)] // Infrastructure for future full lazy loading implementation
struct LazyRootManifest {
    /// Raw decompressed data  
    data: Vec<u8>,
    /// Partial cache of FileDataID mappings
    fdid_cache: HashMap<
        u32,
        std::collections::BTreeMap<tact_parser::wow_root::LocaleContentFlags, [u8; 16]>,
    >,
    /// Filename hash cache
    hash_cache: HashMap<u64, u32>,
    /// Configuration
    config: ManifestConfig,
    /// Approximate file count for memory management
    approx_file_count: u32,
}

/// Lazy-loaded encoding manifest for memory efficiency
#[allow(dead_code)] // Infrastructure for future full lazy loading implementation
struct LazyEncodingManifest {
    /// Raw decompressed data
    data: Vec<u8>,
    /// Partial cache of CKey mappings
    ckey_cache: HashMap<Vec<u8>, tact_parser::encoding::EncodingEntry>,
    /// EKey to CKey reverse mapping cache
    ekey_cache: HashMap<Vec<u8>, Vec<u8>>,
    /// Maximum cache size
    cache_limit: usize,
}

/// Manages TACT manifests and their integration with CASC storage
pub struct TactManifests {
    /// Configuration
    config: ManifestConfig,

    /// Root manifest (FileDataID -> CKey)
    root: Arc<RwLock<Option<WowRoot>>>,

    /// Lazy root manifest for memory efficiency
    lazy_root: Arc<RwLock<Option<LazyRootManifest>>>,

    /// Encoding manifest (CKey -> EKey)
    encoding: Arc<RwLock<Option<EncodingFile>>>,

    /// Lazy encoding manifest for memory efficiency
    lazy_encoding: Arc<RwLock<Option<LazyEncodingManifest>>>,

    /// Cached FileDataID -> EKey mappings
    fdid_cache: Arc<RwLock<HashMap<u32, FileMapping>>>,

    /// Cached filename -> FileDataID mappings (from listfile)
    filename_cache: Arc<RwLock<HashMap<String, u32>>>,
}

impl TactManifests {
    /// Create a new TACT manifest manager
    pub fn new(config: ManifestConfig) -> Self {
        Self {
            config,
            root: Arc::new(RwLock::new(None)),
            lazy_root: Arc::new(RwLock::new(None)),
            encoding: Arc::new(RwLock::new(None)),
            lazy_encoding: Arc::new(RwLock::new(None)),
            fdid_cache: Arc::new(RwLock::new(HashMap::new())),
            filename_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load root manifest from raw data
    pub fn load_root_from_data(&self, data: Vec<u8>) -> Result<()> {
        info!("Loading root manifest from data ({} bytes)", data.len());

        if self.config.lazy_loading {
            return self.load_root_lazy(data);
        }

        // Check if data is BLTE compressed
        let decompressed = if data.starts_with(b"BLTE") {
            debug!("Root manifest is BLTE compressed, decompressing with streaming");
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(data);
            let mut stream = blte::create_streaming_reader(cursor, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            let mut result = Vec::new();
            stream
                .read_to_end(&mut result)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;
            result
        } else {
            data
        };

        // Parse root manifest
        let mut cursor = Cursor::new(decompressed);
        let root = WowRoot::parse(&mut cursor, self.config.locale)
            .map_err(|e| CascError::InvalidFormat(format!("Failed to parse root: {e}")))?;

        info!(
            "Loaded root manifest: {} FileDataIDs, {} name hashes",
            root.fid_md5.len(),
            root.name_hash_fid.len()
        );

        // Store in cache
        *self.root.write() = Some(root);

        // Clear FileDataID cache as it's now outdated
        self.fdid_cache.write().clear();

        Ok(())
    }

    /// Load root manifest with lazy loading (memory efficient)
    fn load_root_lazy(&self, data: Vec<u8>) -> Result<()> {
        info!(
            "Loading root manifest with lazy loading ({} bytes)",
            data.len()
        );

        // Decompress if needed
        let decompressed = if data.starts_with(b"BLTE") {
            debug!("Root manifest is BLTE compressed, decompressing");
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(&data);
            let mut stream = blte::create_streaming_reader(cursor, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            let mut result = Vec::new();
            stream
                .read_to_end(&mut result)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;
            result
        } else {
            data
        };

        // Parse just the header to get an estimate
        let mut cursor = Cursor::new(&decompressed);
        let header = tact_parser::wow_root::WowRootHeader::parse(&mut cursor)
            .map_err(|e| CascError::InvalidFormat(format!("Failed to parse root header: {e}")))?;

        info!(
            "Parsed root header for lazy loading: {} total files, {} named files",
            header.total_file_count, header.named_file_count
        );

        // Create lazy manifest
        let lazy_manifest = LazyRootManifest {
            data: decompressed,
            fdid_cache: HashMap::new(),
            hash_cache: HashMap::new(),
            config: self.config.clone(),
            approx_file_count: header.total_file_count,
        };

        // Store lazy manifest
        *self.lazy_root.write() = Some(lazy_manifest);

        // Clear caches
        self.fdid_cache.write().clear();
        *self.root.write() = None; // Clear fully loaded version if any

        Ok(())
    }

    /// Load encoding manifest from raw data
    pub fn load_encoding_from_data(&self, data: Vec<u8>) -> Result<()> {
        info!("Loading encoding manifest from data ({} bytes)", data.len());

        if self.config.lazy_loading {
            return self.load_encoding_lazy(data);
        }

        // Check if data is BLTE compressed
        let decompressed = if data.starts_with(b"BLTE") {
            debug!("Encoding manifest is BLTE compressed, decompressing with streaming");
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(data);
            let mut stream = blte::create_streaming_reader(cursor, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            let mut result = Vec::new();
            stream
                .read_to_end(&mut result)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;
            result
        } else {
            data
        };

        // Parse encoding manifest
        let encoding = EncodingFile::parse(&decompressed)
            .map_err(|e| CascError::InvalidFormat(format!("Failed to parse encoding: {e}")))?;

        info!(
            "Loaded encoding manifest: {} CKey entries",
            encoding.ckey_count()
        );

        // Store in cache
        *self.encoding.write() = Some(encoding);

        // Clear FileDataID cache as it's now outdated
        self.fdid_cache.write().clear();

        Ok(())
    }

    /// Load encoding manifest with lazy loading (memory efficient)
    fn load_encoding_lazy(&self, data: Vec<u8>) -> Result<()> {
        info!(
            "Loading encoding manifest with lazy loading ({} bytes)",
            data.len()
        );

        // Decompress if needed
        let decompressed = if data.starts_with(b"BLTE") {
            debug!("Encoding manifest is BLTE compressed, decompressing");
            use std::io::{Cursor, Read};
            let cursor = Cursor::new(&data);
            let mut stream = blte::create_streaming_reader(cursor, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            let mut result = Vec::new();
            stream
                .read_to_end(&mut result)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;
            result
        } else {
            data
        };

        info!(
            "Stored encoding manifest data for lazy loading ({} bytes)",
            decompressed.len()
        );

        // Create lazy manifest
        let lazy_manifest = LazyEncodingManifest {
            data: decompressed,
            ckey_cache: HashMap::new(),
            ekey_cache: HashMap::new(),
            cache_limit: self.config.lazy_cache_limit,
        };

        // Store lazy manifest
        *self.lazy_encoding.write() = Some(lazy_manifest);

        // Clear caches
        self.fdid_cache.write().clear();
        *self.encoding.write() = None; // Clear fully loaded version if any

        Ok(())
    }

    /// Load root manifest from streaming reader (memory-efficient)
    pub fn load_root_from_reader<R: std::io::Read + std::io::Seek>(
        &self,
        mut reader: R,
    ) -> Result<()> {
        info!("Loading root manifest from streaming reader");

        // Check if data is BLTE compressed by reading magic
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        let root = if &magic == b"BLTE" {
            debug!("Root manifest is BLTE compressed, decompressing with streaming");
            // Seek back to beginning for BLTE parser
            reader.seek(std::io::SeekFrom::Start(0))?;

            // Create streaming BLTE reader
            let mut blte_stream = blte::create_streaming_reader(reader, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            // Root parser needs Read+Seek, so we need to decompress to memory first
            // This is still better than loading the compressed file entirely into memory
            let mut decompressed = Vec::new();
            blte_stream
                .read_to_end(&mut decompressed)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            // Parse from decompressed data
            let mut cursor = Cursor::new(decompressed);
            WowRoot::parse(&mut cursor, self.config.locale)
                .map_err(|e| CascError::InvalidFormat(format!("Failed to parse root: {e}")))?
        } else {
            debug!("Root manifest is uncompressed, parsing directly");
            // Seek back to beginning for direct parsing
            reader.seek(std::io::SeekFrom::Start(0))?;

            // Parse directly from reader
            WowRoot::parse(&mut reader, self.config.locale)
                .map_err(|e| CascError::InvalidFormat(format!("Failed to parse root: {e}")))?
        };

        info!(
            "Loaded root manifest: {} FileDataIDs, {} name hashes",
            root.fid_md5.len(),
            root.name_hash_fid.len()
        );

        // Store in cache
        *self.root.write() = Some(root);

        // Clear FileDataID cache as it's now outdated
        self.fdid_cache.write().clear();

        Ok(())
    }

    /// Load encoding manifest from streaming reader (memory-efficient)
    pub fn load_encoding_from_reader<R: std::io::Read + std::io::Seek>(
        &self,
        mut reader: R,
    ) -> Result<()> {
        info!("Loading encoding manifest from streaming reader");

        // Check if data is BLTE compressed by reading magic
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        let encoding = if &magic == b"BLTE" {
            debug!("Encoding manifest is BLTE compressed, decompressing with streaming");
            // Seek back to beginning for BLTE parser
            reader.seek(std::io::SeekFrom::Start(0))?;

            // Create streaming BLTE reader
            let mut blte_stream = blte::create_streaming_reader(reader, None)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            // For encoding files, we need to read all data since the parser expects &[u8]
            // TODO: This could be further optimized if EncodingFile gets streaming support
            let mut decompressed = Vec::new();
            blte_stream
                .read_to_end(&mut decompressed)
                .map_err(|e| CascError::DecompressionError(e.to_string()))?;

            EncodingFile::parse(&decompressed)
                .map_err(|e| CascError::InvalidFormat(format!("Failed to parse encoding: {e}")))?
        } else {
            debug!("Encoding manifest is uncompressed");
            // Read all data for parsing (EncodingFile needs &[u8])
            reader.seek(std::io::SeekFrom::Start(0))?;
            let mut data = Vec::new();
            reader.read_to_end(&mut data)?;

            EncodingFile::parse(&data)
                .map_err(|e| CascError::InvalidFormat(format!("Failed to parse encoding: {e}")))?
        };

        info!(
            "Loaded encoding manifest: {} CKey entries",
            encoding.ckey_count()
        );

        // Store in cache
        *self.encoding.write() = Some(encoding);

        // Clear FileDataID cache as it's now outdated
        self.fdid_cache.write().clear();

        Ok(())
    }

    /// Load root manifest from file
    pub fn load_root_from_file(&self, path: &Path) -> Result<()> {
        info!("Loading root manifest from file: {:?}", path);
        let file = std::fs::File::open(path)?;
        self.load_root_from_reader(file)
    }

    /// Load encoding manifest from file
    pub fn load_encoding_from_file(&self, path: &Path) -> Result<()> {
        info!("Loading encoding manifest from file: {:?}", path);
        let file = std::fs::File::open(path)?;
        self.load_encoding_from_reader(file)
    }

    /// Load a listfile for filename -> FileDataID mappings
    pub fn load_listfile(&self, path: &Path) -> Result<usize> {
        info!("Loading listfile from: {:?}", path);
        let file = std::fs::File::open(path)?;
        self.load_listfile_from_reader(file)
    }

    /// Load listfile from streaming reader (memory-efficient for large listfiles)
    pub fn load_listfile_from_reader<R: std::io::Read>(&self, reader: R) -> Result<usize> {
        info!("Loading listfile from streaming reader");

        let mut cache = self.filename_cache.write();
        cache.clear();

        // Use BufReader for efficient line-by-line reading
        use std::io::{BufRead, BufReader};
        let buf_reader = BufReader::new(reader);

        let mut count = 0;
        for line_result in buf_reader.lines() {
            let line = line_result?;

            // Parse CSV format: "FileDataID;Filename"
            if let Some(sep_pos) = line.find(';') {
                if let Ok(fdid) = line[..sep_pos].parse::<u32>() {
                    let filename = line[sep_pos + 1..].to_string();
                    cache.insert(filename, fdid);
                    count += 1;
                }
            }
        }

        info!("Loaded {} filename mappings from listfile", count);
        Ok(count)
    }

    /// Lookup a file by FileDataID
    pub fn lookup_by_fdid(&self, fdid: u32) -> Result<FileMapping> {
        // Check cache first
        {
            let cache = self.fdid_cache.read();
            if let Some(mapping) = cache.get(&fdid) {
                return Ok(mapping.clone());
            }
        }

        // Try lazy loading first if enabled
        if self.config.lazy_loading {
            if let Some(result) = self.lookup_fdid_lazy(fdid)? {
                return Ok(result);
            }
        }

        // Fallback to fully loaded manifests
        let root = self.root.read();
        let encoding = self.encoding.read();

        let root = root
            .as_ref()
            .ok_or_else(|| CascError::ManifestNotLoaded("root".to_string()))?;
        let encoding = encoding
            .as_ref()
            .ok_or_else(|| CascError::ManifestNotLoaded("encoding".to_string()))?;

        // Get content key from root manifest
        let content_entries = root
            .fid_md5
            .get(&fdid)
            .ok_or_else(|| CascError::EntryNotFound(format!("FileDataID {fdid}")))?;

        // Find the best matching content entry based on locale/content flags
        let (flags, content_key) = self.select_best_content(content_entries)?;

        // Get encoding key from encoding manifest
        let encoding_entry = encoding.lookup_by_ckey(content_key).ok_or_else(|| {
            CascError::EntryNotFound(format!("CKey {} in encoding", hex::encode(content_key)))
        })?;

        // Get the first EKey (usually there's only one)
        let ekey = encoding_entry
            .encoding_keys
            .first()
            .ok_or_else(|| CascError::EntryNotFound("EKey in encoding entry".to_string()))?;

        let mapping = FileMapping {
            file_data_id: fdid,
            content_key: *content_key,
            encoding_key: Some(EKey::from_slice(ekey).unwrap()),
            flags: Some(*flags),
        };

        // Cache the result
        if self.config.cache_manifests {
            self.fdid_cache.write().insert(fdid, mapping.clone());
        }

        Ok(mapping)
    }

    /// Lookup a file by filename
    pub fn lookup_by_filename(&self, filename: &str) -> Result<FileMapping> {
        // First try the filename cache
        let fdid = {
            let cache = self.filename_cache.read();
            cache.get(filename).copied()
        };

        if let Some(fdid) = fdid {
            return self.lookup_by_fdid(fdid);
        }

        // Try using jenkins hash from root manifest
        let root = self.root.read();
        let root = root
            .as_ref()
            .ok_or_else(|| CascError::ManifestNotLoaded("root".to_string()))?;

        let fdid = root
            .get_fid(filename)
            .ok_or_else(|| CascError::EntryNotFound(format!("Filename: {filename}")))?;

        self.lookup_by_fdid(fdid)
    }

    /// Get all FileDataIDs
    pub fn get_all_fdids(&self) -> Result<Vec<u32>> {
        let root = self.root.read();
        let root = root
            .as_ref()
            .ok_or_else(|| CascError::ManifestNotLoaded("root".to_string()))?;

        Ok(root.fid_md5.keys().copied().collect())
    }

    /// Get FileDataID for a filename (if known)
    pub fn get_fdid_for_filename(&self, filename: &str) -> Option<u32> {
        // Check filename cache first
        {
            let cache = self.filename_cache.read();
            if let Some(&fdid) = cache.get(filename) {
                return Some(fdid);
            }
        }

        // Try root manifest's jenkins hash lookup
        let root = self.root.read();
        root.as_ref()?.get_fid(filename)
    }

    /// Get EKey for a FileDataID (if manifests are loaded)
    pub fn get_ekey_for_fdid(&self, fdid: u32) -> Result<EKey> {
        let mapping = self.lookup_by_fdid(fdid)?;
        mapping
            .encoding_key
            .ok_or_else(|| CascError::EntryNotFound(format!("EKey for FDID {fdid}")))
    }

    /// Check if manifests are loaded
    pub fn is_loaded(&self) -> bool {
        let has_root = self.root.read().is_some() || self.lazy_root.read().is_some();
        let has_encoding = self.encoding.read().is_some() || self.lazy_encoding.read().is_some();
        has_root && has_encoding
    }

    /// Clear all cached data
    pub fn clear_cache(&self) {
        self.fdid_cache.write().clear();

        // Clear lazy manifest caches
        if let Some(lazy_root) = self.lazy_root.write().as_mut() {
            lazy_root.fdid_cache.clear();
            lazy_root.hash_cache.clear();
        }

        if let Some(lazy_encoding) = self.lazy_encoding.write().as_mut() {
            lazy_encoding.ckey_cache.clear();
            lazy_encoding.ekey_cache.clear();
        }

        debug!("Cleared FileDataID and lazy manifest caches");
    }

    /// Lazy lookup implementation for FileDataID
    fn lookup_fdid_lazy(&self, _fdid: u32) -> Result<Option<FileMapping>> {
        let lazy_root = self.lazy_root.read();
        let lazy_encoding = self.lazy_encoding.read();

        if lazy_root.is_none() || lazy_encoding.is_none() {
            return Ok(None); // Fallback to full loading
        }

        // For now, fallback to full loading until we implement on-demand parsing
        // This provides the infrastructure for lazy loading while maintaining compatibility
        debug!("Lazy lookup infrastructure ready, falling back to full loading for now");
        Ok(None)
    }

    /// Select the best content entry based on locale and content flags
    fn select_best_content<'a>(
        &self,
        entries: &'a std::collections::BTreeMap<
            tact_parser::wow_root::LocaleContentFlags,
            [u8; 16],
        >,
    ) -> Result<(&'a ContentFlags, &'a [u8; 16])> {
        // If only one entry, use it
        if entries.len() == 1 {
            let (flags, key) = entries.iter().next().unwrap();
            return Ok((&flags.content, key));
        }

        // Filter by locale first
        let locale_matches: Vec<_> = entries
            .iter()
            .filter(|(flags, _)| (flags.locale & self.config.locale).any() || flags.locale.all())
            .collect();

        if locale_matches.is_empty() {
            // No locale match, use first available
            let (flags, key) = entries.iter().next().unwrap();
            return Ok((&flags.content, key));
        }

        // If content flags are specified, try to match them
        if let Some(required_flags) = self.config.content_flags {
            for (flags, key) in &locale_matches {
                // Check if the entry matches required flags
                if self.content_flags_match(&flags.content, &required_flags) {
                    return Ok((&flags.content, key));
                }
            }
        }

        // Use first locale match
        let (flags, key) = locale_matches[0];
        Ok((&flags.content, key))
    }

    /// Check if content flags match requirements
    fn content_flags_match(&self, flags: &ContentFlags, required: &ContentFlags) -> bool {
        // Check platform requirements
        if required.windows() && !flags.windows() {
            return false;
        }
        if required.macos() && !flags.macos() {
            return false;
        }

        // Check architecture
        if required.x86_64() && !flags.x86_64() {
            return false;
        }
        if required.x86_32() && !flags.x86_32() {
            return false;
        }
        if required.aarch64() && !flags.aarch64() {
            return false;
        }

        true
    }
}
