//! Progressive file loading with size hints for better memory management

use crate::error::{CascError, Result};
use crate::types::EKey;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, info, trace, warn};

/// Default chunk size for progressive loading (256KB)
const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;

/// Default maximum number of chunks to prefetch ahead
const DEFAULT_MAX_PREFETCH_CHUNKS: usize = 4;

/// Default timeout for chunk loading
const DEFAULT_CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// Configuration for progressive file loading
#[derive(Debug, Clone)]
pub struct ProgressiveConfig {
    /// Size of each chunk to load progressively
    pub chunk_size: usize,
    /// Maximum number of chunks to prefetch ahead of current position
    pub max_prefetch_chunks: usize,
    /// Timeout for loading individual chunks
    pub chunk_timeout: Duration,
    /// Whether to use aggressive prefetching based on access patterns
    pub use_predictive_prefetch: bool,
    /// Minimum file size to enable progressive loading (smaller files loaded entirely)
    pub min_progressive_size: usize,
}

impl Default for ProgressiveConfig {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            max_prefetch_chunks: DEFAULT_MAX_PREFETCH_CHUNKS,
            chunk_timeout: DEFAULT_CHUNK_TIMEOUT,
            use_predictive_prefetch: true,
            min_progressive_size: 1024 * 1024, // 1MB minimum
        }
    }
}

/// Size hint information for progressive loading
#[derive(Debug, Clone, Copy)]
pub enum SizeHint {
    /// Exact size is known
    Exact(u64),
    /// Estimated size with confidence level (0.0-1.0)
    Estimated { size: u64, confidence: f32 },
    /// Minimum known size, actual could be larger
    Minimum(u64),
    /// No size information available
    Unknown,
}

impl SizeHint {
    /// Get the suggested initial allocation size
    pub fn suggested_initial_size(&self) -> Option<usize> {
        match self {
            SizeHint::Exact(size) => Some(*size as usize),
            SizeHint::Estimated { size, confidence } if *confidence > 0.7 => Some(*size as usize),
            SizeHint::Minimum(size) => Some(*size as usize),
            _ => None,
        }
    }

    /// Check if progressive loading is recommended
    pub fn should_use_progressive(&self, config: &ProgressiveConfig) -> bool {
        match self {
            SizeHint::Exact(size) | SizeHint::Minimum(size) => {
                *size as usize > config.min_progressive_size
            }
            SizeHint::Estimated { size, confidence } => {
                *size as usize > config.min_progressive_size && *confidence > 0.5
            }
            SizeHint::Unknown => false,
        }
    }
}

/// Access pattern tracking for predictive prefetching
#[derive(Debug, Default)]
struct AccessPattern {
    /// Sequential read history (chunk indices)
    sequential_reads: VecDeque<usize>,
    /// Last access time
    last_access: Option<Instant>,
    /// Average chunk access interval
    avg_interval: Option<Duration>,
}

impl AccessPattern {
    /// Record a chunk access
    fn record_access(&mut self, chunk_index: usize) {
        let now = Instant::now();

        // Update interval tracking
        if let Some(last) = self.last_access {
            let interval = now.duration_since(last);
            self.avg_interval = Some(match self.avg_interval {
                Some(avg) => Duration::from_nanos(
                    ((avg.as_nanos() + interval.as_nanos()) / 2).min(u64::MAX as u128) as u64,
                ),
                None => interval,
            });
        }

        self.last_access = Some(now);
        self.sequential_reads.push_back(chunk_index);

        // Keep only recent history
        while self.sequential_reads.len() > 10 {
            self.sequential_reads.pop_front();
        }
    }

    /// Predict next likely chunks to access
    fn predict_next_chunks(&self, current_chunk: usize, max_predictions: usize) -> Vec<usize> {
        if self.sequential_reads.len() < 2 {
            // Default to sequential prediction
            return (1..=max_predictions).map(|i| current_chunk + i).collect();
        }

        // Analyze pattern
        let is_sequential = self
            .sequential_reads
            .iter()
            .collect::<Vec<_>>()
            .windows(2)
            .all(|w| w[1] == &(w[0] + 1));

        if is_sequential {
            // Sequential access pattern
            (1..=max_predictions).map(|i| current_chunk + i).collect()
        } else {
            // More complex pattern analysis could be added here
            // For now, default to sequential
            (1..=max_predictions).map(|i| current_chunk + i).collect()
        }
    }
}

/// A chunk of progressively loaded data
#[derive(Debug, Clone)]
pub struct ProgressiveChunk {
    /// Chunk index within the file
    pub index: usize,
    /// Data for this chunk
    pub data: Vec<u8>,
    /// Actual size of this chunk (may be less than chunk_size for last chunk)
    pub size: usize,
    /// Whether this chunk is the final chunk in the file
    pub is_final: bool,
}

/// State of a progressively loaded file
#[derive(Debug)]
pub struct ProgressiveFile {
    /// Unique identifier for this file (EKey)
    ekey: EKey,
    /// Size hint for the file
    size_hint: SizeHint,
    /// Configuration used for loading
    config: ProgressiveConfig,
    /// Loaded chunks (indexed by chunk number)
    chunks: Arc<RwLock<std::collections::HashMap<usize, ProgressiveChunk>>>,
    /// Current read position
    position: Arc<RwLock<u64>>,
    /// Access pattern tracking
    access_pattern: Arc<RwLock<AccessPattern>>,
    /// Channel for requesting chunks
    chunk_request_tx: mpsc::UnboundedSender<ChunkRequest>,
    /// Loading statistics
    stats: Arc<RwLock<LoadingStats>>,
}

/// Statistics for progressive loading
#[derive(Debug, Default, Clone)]
pub struct LoadingStats {
    /// Total chunks loaded
    pub chunks_loaded: usize,
    /// Total bytes loaded
    pub bytes_loaded: u64,
    /// Cache hits (chunks already loaded)
    pub cache_hits: usize,
    /// Cache misses (chunks needed to be loaded)
    pub cache_misses: usize,
    /// Total loading time
    pub total_load_time: Duration,
    /// Average chunk load time
    pub avg_chunk_load_time: Duration,
    /// Number of prefetched chunks that were used
    pub prefetch_hits: usize,
    /// Number of prefetched chunks that were wasted
    pub prefetch_misses: usize,
}

/// Request for loading a chunk
#[derive(Debug)]
struct ChunkRequest {
    /// Chunk index to load
    chunk_index: usize,
    /// Priority of the request (higher = more urgent)
    priority: ChunkPriority,
    /// Response channel
    response_tx: oneshot::Sender<Result<()>>,
}

/// Priority levels for chunk loading
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ChunkPriority {
    /// Background prefetch
    Prefetch = 1,
    /// Normal read request
    Normal = 2,
    /// Urgent read request (blocking current operation)
    Urgent = 3,
}

impl ProgressiveFile {
    /// Create a new progressive file loader
    pub fn new(
        ekey: EKey,
        size_hint: SizeHint,
        config: ProgressiveConfig,
        loader: Weak<dyn ChunkLoader + Send + Sync>,
    ) -> Self {
        let (chunk_request_tx, chunk_request_rx) = mpsc::unbounded_channel();
        let chunks = Arc::new(RwLock::new(std::collections::HashMap::new()));
        let stats = Arc::new(RwLock::new(LoadingStats::default()));

        // Spawn background chunk loader
        let loader_chunks = Arc::clone(&chunks);
        let loader_stats = Arc::clone(&stats);
        let loader_config = config.clone();

        tokio::spawn(async move {
            Self::chunk_loader_task(
                ekey,
                loader,
                chunk_request_rx,
                loader_chunks,
                loader_stats,
                loader_config,
            )
            .await;
        });

        Self {
            ekey,
            size_hint,
            config,
            chunks,
            position: Arc::new(RwLock::new(0)),
            access_pattern: Arc::new(RwLock::new(AccessPattern::default())),
            chunk_request_tx,
            stats,
        }
    }

    /// Read data from the progressive file
    pub async fn read(&self, offset: u64, length: usize) -> Result<Vec<u8>> {
        let start_time = Instant::now();
        trace!("Progressive read: offset={}, length={}", offset, length);

        // Update position tracking
        {
            let mut pos = self.position.write().await;
            *pos = offset;
        }

        let chunk_size = self.config.chunk_size as u64;
        let start_chunk = (offset / chunk_size) as usize;
        let end_chunk = ((offset + length as u64 - 1) / chunk_size) as usize;

        let mut result = Vec::with_capacity(length);

        for chunk_index in start_chunk..=end_chunk {
            // Record access for pattern analysis
            {
                let mut pattern = self.access_pattern.write().await;
                pattern.record_access(chunk_index);
            }

            // Check if chunk is already loaded
            let chunk_data = {
                let chunks = self.chunks.read().await;
                chunks.get(&chunk_index).map(|chunk| chunk.data.clone())
            };

            let chunk_data = if let Some(data) = chunk_data {
                // Cache hit
                {
                    let mut stats = self.stats.write().await;
                    stats.cache_hits += 1;
                }
                trace!("Cache hit for chunk {}", chunk_index);
                data
            } else {
                // Cache miss - need to load chunk
                {
                    let mut stats = self.stats.write().await;
                    stats.cache_misses += 1;
                }

                trace!("Cache miss for chunk {}, loading...", chunk_index);
                self.load_chunk(chunk_index, ChunkPriority::Urgent).await?;

                let chunks = self.chunks.read().await;
                chunks
                    .get(&chunk_index)
                    .ok_or_else(|| {
                        CascError::InvalidArchiveFormat("Chunk failed to load".to_string())
                    })?
                    .data
                    .clone()
            };

            // Extract the portion of the chunk we need
            let chunk_start_offset = chunk_index as u64 * chunk_size;
            let chunk_end_offset = chunk_start_offset + chunk_data.len() as u64;

            let read_start = offset.max(chunk_start_offset);
            let read_end = (offset + length as u64).min(chunk_end_offset);

            if read_start < read_end {
                let chunk_read_start = (read_start - chunk_start_offset) as usize;
                let chunk_read_end = (read_end - chunk_start_offset) as usize;

                result.extend_from_slice(&chunk_data[chunk_read_start..chunk_read_end]);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_load_time += start_time.elapsed();
        }

        // Trigger prefetching if enabled
        if self.config.use_predictive_prefetch {
            self.trigger_predictive_prefetch(end_chunk).await;
        }

        debug!(
            "Progressive read completed: offset={}, length={}, chunks={}..={}",
            offset, length, start_chunk, end_chunk
        );

        Ok(result)
    }

    /// Load a specific chunk with given priority
    async fn load_chunk(&self, chunk_index: usize, priority: ChunkPriority) -> Result<()> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = ChunkRequest {
            chunk_index,
            priority,
            response_tx,
        };

        self.chunk_request_tx
            .send(request)
            .map_err(|_| CascError::InvalidArchiveFormat("Chunk loader unavailable".to_string()))?;

        response_rx
            .await
            .map_err(|_| CascError::InvalidArchiveFormat("Chunk load failed".to_string()))?
    }

    /// Trigger predictive prefetching based on access patterns
    async fn trigger_predictive_prefetch(&self, last_accessed_chunk: usize) {
        let predictions = {
            let pattern = self.access_pattern.read().await;
            pattern.predict_next_chunks(last_accessed_chunk, self.config.max_prefetch_chunks)
        };

        trace!("Predictive prefetch suggestions: {:?}", predictions);

        for chunk_index in predictions {
            // Only prefetch if not already loaded
            let already_loaded = {
                let chunks = self.chunks.read().await;
                chunks.contains_key(&chunk_index)
            };

            if !already_loaded {
                let _ = self.load_chunk(chunk_index, ChunkPriority::Prefetch).await;
            }
        }
    }

    /// Get current loading statistics
    pub async fn get_stats(&self) -> LoadingStats {
        self.stats.read().await.clone()
    }

    /// Get the current size estimate
    pub fn get_size_hint(&self) -> SizeHint {
        self.size_hint
    }

    /// Check if the file is fully loaded
    pub async fn is_fully_loaded(&self) -> bool {
        if let SizeHint::Exact(size) = self.size_hint {
            let chunks = self.chunks.read().await;
            let chunk_size = self.config.chunk_size as u64;
            let expected_chunks = ((size + chunk_size - 1) / chunk_size) as usize;

            chunks.len() == expected_chunks && chunks.values().any(|chunk| chunk.is_final)
        } else {
            false
        }
    }

    /// Background task for loading chunks
    async fn chunk_loader_task(
        ekey: EKey,
        loader: Weak<dyn ChunkLoader + Send + Sync>,
        mut request_rx: mpsc::UnboundedReceiver<ChunkRequest>,
        chunks: Arc<RwLock<std::collections::HashMap<usize, ProgressiveChunk>>>,
        stats: Arc<RwLock<LoadingStats>>,
        config: ProgressiveConfig,
    ) {
        debug!("Started chunk loader task for {}", ekey);

        // Priority queue for chunk requests
        let mut pending_requests: Vec<ChunkRequest> = Vec::new();

        while let Some(request) = request_rx.recv().await {
            pending_requests.push(request);

            // Sort by priority (highest first)
            pending_requests.sort_by(|a, b| b.priority.cmp(&a.priority));

            // Process highest priority request
            if let Some(request) = pending_requests.pop() {
                let load_result = if let Some(loader_arc) = loader.upgrade() {
                    Self::load_single_chunk(
                        loader_arc,
                        ekey,
                        request.chunk_index,
                        &chunks,
                        &stats,
                        &config,
                    )
                    .await
                } else {
                    warn!("Chunk loader has been dropped, stopping chunk loading");
                    break;
                };

                let _ = request.response_tx.send(load_result);
            }
        }

        debug!("Chunk loader task completed for {}", ekey);
    }

    /// Load a single chunk
    async fn load_single_chunk(
        loader: Arc<dyn ChunkLoader + Send + Sync>,
        ekey: EKey,
        chunk_index: usize,
        chunks: &Arc<RwLock<std::collections::HashMap<usize, ProgressiveChunk>>>,
        stats: &Arc<RwLock<LoadingStats>>,
        config: &ProgressiveConfig,
    ) -> Result<()> {
        let start_time = Instant::now();
        trace!("Loading chunk {} for {}", chunk_index, ekey);

        // Check if already loaded (race condition protection)
        {
            let chunks_guard = chunks.read().await;
            if chunks_guard.contains_key(&chunk_index) {
                trace!("Chunk {} already loaded", chunk_index);
                return Ok(());
            }
        }

        let chunk_offset = chunk_index as u64 * config.chunk_size as u64;

        match loader
            .load_chunk(ekey, chunk_offset, config.chunk_size)
            .await
        {
            Ok(chunk_data) => {
                let is_final = chunk_data.len() < config.chunk_size;
                let chunk_size = chunk_data.len();
                let chunk = ProgressiveChunk {
                    index: chunk_index,
                    size: chunk_size,
                    is_final,
                    data: chunk_data,
                };

                // Store the chunk
                {
                    let mut chunks_guard = chunks.write().await;
                    chunks_guard.insert(chunk_index, chunk.clone());
                }

                // Update stats
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.chunks_loaded += 1;
                    stats_guard.bytes_loaded += chunk_size as u64;
                    let load_time = start_time.elapsed();
                    stats_guard.total_load_time += load_time;
                    stats_guard.avg_chunk_load_time =
                        stats_guard.total_load_time / stats_guard.chunks_loaded as u32;
                }

                trace!(
                    "Loaded chunk {} ({} bytes) for {} in {:?}",
                    chunk_index,
                    chunk.size,
                    ekey,
                    start_time.elapsed()
                );

                Ok(())
            }
            Err(e) => {
                warn!("Failed to load chunk {} for {}: {}", chunk_index, ekey, e);
                Err(e)
            }
        }
    }
}

/// Trait for loading chunks from storage
#[async_trait]
pub trait ChunkLoader {
    /// Load a chunk of data from storage
    async fn load_chunk(&self, ekey: EKey, offset: u64, size: usize) -> Result<Vec<u8>>;
}

/// Progressive file manager that creates and manages progressive file instances
pub struct ProgressiveFileManager {
    /// Configuration for progressive loading
    config: ProgressiveConfig,
    /// Active progressive files
    active_files: Arc<RwLock<std::collections::HashMap<EKey, Arc<ProgressiveFile>>>>,
    /// Chunk loader implementation
    chunk_loader: Arc<dyn ChunkLoader + Send + Sync>,
}

impl ProgressiveFileManager {
    /// Create a new progressive file manager
    pub fn new(
        config: ProgressiveConfig,
        chunk_loader: Arc<dyn ChunkLoader + Send + Sync>,
    ) -> Self {
        Self {
            config,
            active_files: Arc::new(RwLock::new(std::collections::HashMap::new())),
            chunk_loader,
        }
    }

    /// Create or get existing progressive file
    pub async fn get_or_create_progressive_file(
        &self,
        ekey: EKey,
        size_hint: SizeHint,
    ) -> Arc<ProgressiveFile> {
        // Check if file already exists
        {
            let active_files = self.active_files.read().await;
            if let Some(file) = active_files.get(&ekey) {
                return Arc::clone(file);
            }
        }

        // Create new progressive file
        let progressive_file = Arc::new(ProgressiveFile::new(
            ekey,
            size_hint,
            self.config.clone(),
            Arc::downgrade(&self.chunk_loader),
        ));

        // Register it
        {
            let mut active_files = self.active_files.write().await;
            active_files.insert(ekey, Arc::clone(&progressive_file));
        }

        info!(
            "Created progressive file for {} with hint {:?}",
            ekey, size_hint
        );
        progressive_file
    }

    /// Remove a progressive file from management (cleanup)
    pub async fn remove_progressive_file(&self, ekey: &EKey) {
        let mut active_files = self.active_files.write().await;
        active_files.remove(ekey);
    }

    /// Get statistics for all active progressive files
    pub async fn get_global_stats(&self) -> Vec<(EKey, LoadingStats)> {
        let active_files = self.active_files.read().await;
        let mut stats = Vec::new();

        for (ekey, file) in active_files.iter() {
            let file_stats = file.get_stats().await;
            stats.push((*ekey, file_stats));
        }

        stats
    }

    /// Clean up inactive files (those not accessed recently)
    pub async fn cleanup_inactive_files(&self, max_idle_time: Duration) {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        {
            let active_files = self.active_files.read().await;
            for (ekey, file) in active_files.iter() {
                let pattern = file.access_pattern.read().await;
                if let Some(last_access) = pattern.last_access {
                    if now.duration_since(last_access) > max_idle_time {
                        to_remove.push(*ekey);
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            let mut active_files = self.active_files.write().await;
            for ekey in to_remove {
                active_files.remove(&ekey);
                trace!("Cleaned up inactive progressive file: {}", ekey);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EKey;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Mock chunk loader for testing
    struct MockChunkLoader {
        total_size: usize,
        call_count: Arc<AtomicUsize>,
    }

    impl MockChunkLoader {
        fn new(total_size: usize) -> Self {
            Self {
                total_size,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl ChunkLoader for MockChunkLoader {
        async fn load_chunk(&self, _ekey: EKey, offset: u64, size: usize) -> Result<Vec<u8>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            let start = offset as usize;
            let end = (start + size).min(self.total_size);

            if start >= self.total_size {
                return Ok(Vec::new());
            }

            // Generate deterministic data for testing
            let data: Vec<u8> = (start..end).map(|i| (i % 256) as u8).collect();

            // Simulate some loading delay
            tokio::time::sleep(Duration::from_millis(10)).await;

            Ok(data)
        }
    }

    #[tokio::test]
    async fn test_progressive_file_creation() {
        let ekey = EKey::new([1; 16]);
        let size_hint = SizeHint::Exact(1024);
        let config = ProgressiveConfig::default();
        let loader = Arc::new(MockChunkLoader::new(1024));

        let manager = ProgressiveFileManager::new(config, loader);
        let file = manager
            .get_or_create_progressive_file(ekey, size_hint)
            .await;

        assert_eq!(file.get_size_hint().suggested_initial_size(), Some(1024));
    }

    #[tokio::test]
    async fn test_progressive_reading() {
        let ekey = EKey::new([2; 16]);
        let total_size = 2048;
        let size_hint = SizeHint::Exact(total_size);
        let config = ProgressiveConfig {
            chunk_size: 512,
            ..ProgressiveConfig::default()
        };
        let loader = Arc::new(MockChunkLoader::new(total_size as usize));

        let manager = ProgressiveFileManager::new(config, loader);
        let file = manager
            .get_or_create_progressive_file(ekey, size_hint)
            .await;

        // Read from beginning
        let data1 = file.read(0, 256).await.unwrap();
        assert_eq!(data1.len(), 256);
        assert_eq!(data1[0], 0);
        assert_eq!(data1[255], 255);

        // Read across chunk boundary
        let data2 = file.read(400, 300).await.unwrap();
        assert_eq!(data2.len(), 300);

        let stats = file.get_stats().await;
        assert!(stats.chunks_loaded > 0);
        assert!(stats.bytes_loaded > 0);
    }

    #[tokio::test]
    async fn test_size_hint_logic() {
        let config = ProgressiveConfig::default();

        assert!(SizeHint::Exact(2_000_000).should_use_progressive(&config));
        assert!(!SizeHint::Exact(500_000).should_use_progressive(&config));

        assert!(
            SizeHint::Estimated {
                size: 2_000_000,
                confidence: 0.8
            }
            .should_use_progressive(&config)
        );

        assert!(
            !SizeHint::Estimated {
                size: 2_000_000,
                confidence: 0.3
            }
            .should_use_progressive(&config)
        );
    }

    #[tokio::test]
    async fn test_cache_efficiency() {
        let ekey = EKey::new([3; 16]);
        let total_size = 1024;
        let size_hint = SizeHint::Exact(total_size);
        let config = ProgressiveConfig {
            chunk_size: 256,
            ..ProgressiveConfig::default()
        };
        let loader = Arc::new(MockChunkLoader::new(total_size as usize));

        let manager = ProgressiveFileManager::new(config, loader.clone());
        let file = manager
            .get_or_create_progressive_file(ekey, size_hint)
            .await;

        // First read
        let _data1 = file.read(100, 100).await.unwrap();
        let initial_calls = loader.call_count.load(Ordering::SeqCst);

        // Second read from same chunk - should be cached
        let _data2 = file.read(150, 50).await.unwrap();
        let final_calls = loader.call_count.load(Ordering::SeqCst);

        // Should not have made additional calls for cached chunk
        assert_eq!(initial_calls, final_calls);

        let stats = file.get_stats().await;
        assert!(stats.cache_hits > 0);
    }
}
