//! Example: Texture streaming using progressive file loading
//!
//! This example demonstrates how progressive loading can be used for
//! streaming game textures (BLP files) on-demand, reducing memory usage
//! and improving load times.

use casc_storage::types::CascConfig;
use casc_storage::{CascStorage, EKey, ProgressiveConfig, SizeHint};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio;
use tracing::{debug, info, warn};
use tracing_subscriber;

/// Simulated texture manager that uses progressive loading
struct TextureManager {
    storage: Arc<tokio::sync::Mutex<CascStorage>>,
    /// Active texture streams
    textures: HashMap<EKey, Arc<casc_storage::progressive::ProgressiveFile>>,
    /// Configuration for progressive loading
    config: ProgressiveConfig,
}

impl TextureManager {
    fn new(storage: CascStorage) -> Self {
        let config = ProgressiveConfig {
            chunk_size: 128 * 1024, // 128KB chunks for textures
            max_prefetch_chunks: 2, // Conservative prefetch for many textures
            chunk_timeout: std::time::Duration::from_secs(10),
            use_predictive_prefetch: true,
            min_progressive_size: 512 * 1024, // Use progressive for textures > 512KB
        };

        let mut storage = storage;
        storage.init_progressive_loading(config.clone());

        Self {
            storage: Arc::new(tokio::sync::Mutex::new(storage)),
            textures: HashMap::new(),
            config,
        }
    }

    /// Load texture header (first few bytes) to get dimensions and format
    async fn load_texture_header(
        &mut self,
        ekey: &EKey,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        info!("Loading texture header for {}", ekey);

        // Check if we already have this texture stream
        if let Some(progressive_file) = self.textures.get(ekey) {
            debug!("Reusing existing texture stream");
            // BLP header is typically in first 148 bytes
            return Ok(progressive_file.read(0, 148).await?);
        }

        // Get file info for size hint
        let storage = self.storage.lock().await;
        // For demo purposes, use Unknown size hint
        let size_hint = SizeHint::Unknown;

        // Check if we should use progressive loading
        if !size_hint.should_use_progressive(&self.config) {
            // Small texture, just read it all
            debug!("Texture is small, using regular read");
            let data = storage.read(ekey)?;
            return Ok(data[..148.min(data.len())].to_vec());
        }

        // Create progressive file handle
        let progressive_file = storage.read_progressive(ekey, size_hint).await?;

        // Read header
        let header = progressive_file.read(0, 148).await?;

        // Store for future use
        drop(storage); // Release lock before modifying self
        self.textures.insert(*ekey, progressive_file);

        Ok(header)
    }

    /// Load a specific mipmap level of a texture
    async fn load_texture_mipmap(
        &mut self,
        ekey: &EKey,
        mipmap_offset: u64,
        mipmap_size: usize,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        info!(
            "Loading mipmap for {} (offset: {}, size: {})",
            ekey, mipmap_offset, mipmap_size
        );

        // Ensure we have a progressive file handle
        if !self.textures.contains_key(ekey) {
            let storage = self.storage.lock().await;
            let size_hint = SizeHint::Unknown;

            let progressive_file = storage.read_progressive(ekey, size_hint).await?;
            drop(storage);
            self.textures.insert(*ekey, progressive_file);
        }

        // Load the specific mipmap data
        let progressive_file = self.textures.get(ekey).unwrap();
        Ok(progressive_file.read(mipmap_offset, mipmap_size).await?)
    }

    /// Unload a texture from memory
    async fn unload_texture(&mut self, ekey: &EKey) {
        if self.textures.remove(ekey).is_some() {
            info!("Unloaded texture {}", ekey);
        }
    }

    /// Get streaming statistics for all loaded textures
    async fn get_statistics(&self) -> HashMap<EKey, casc_storage::progressive::LoadingStats> {
        let mut stats = HashMap::new();
        for (ekey, file) in &self.textures {
            stats.insert(*ekey, file.get_stats().await);
        }
        stats
    }
}

/// Simulate a texture loading scenario
async fn simulate_texture_loading() -> Result<(), Box<dyn std::error::Error>> {
    let data_path =
        Path::new("/home/danielsreichenbach/Downloads/wow/1.13.2.31650.windows-win64/Data");

    if !data_path.exists() {
        warn!("Data path does not exist: {:?}", data_path);
        return Ok(());
    }

    info!("Initializing texture manager");
    let config = CascConfig {
        data_path: data_path.to_path_buf(),
        cache_size_mb: 100,
        max_archive_size: 1024 * 1024 * 1024,
        use_memory_mapping: true,
        read_only: false,
    };
    let storage = CascStorage::new(config)?;
    let mut texture_manager = TextureManager::new(storage);

    // Get some files to simulate as textures
    let storage = texture_manager.storage.lock().await;
    let files: Vec<EKey> = storage.get_all_ekeys().into_iter().take(5).collect();
    drop(storage);

    if files.is_empty() {
        warn!("No files found to simulate texture loading");
        return Ok(());
    }

    info!("Simulating texture streaming for {} files", files.len());

    // Simulate loading texture headers (e.g., when browsing a model viewer)
    for ekey in &files {
        match texture_manager.load_texture_header(ekey).await {
            Ok(header) => {
                info!("Loaded texture header: {} bytes", header.len());
                // In a real scenario, we'd parse BLP header here
            }
            Err(e) => {
                warn!("Failed to load texture header: {}", e);
            }
        }
    }

    // Simulate loading specific mipmaps (e.g., when zooming in)
    if let Some(ekey) = files.first() {
        info!("\nSimulating mipmap loading for texture {}", ekey);

        // Load different mipmap levels
        for level in 0..3 {
            let offset = 148 + (level * 1024); // Simulated offsets
            let size = 1024; // Simulated mipmap size

            match texture_manager
                .load_texture_mipmap(ekey, offset, size)
                .await
            {
                Ok(data) => {
                    info!("Loaded mipmap level {}: {} bytes", level, data.len());
                }
                Err(e) => {
                    warn!("Failed to load mipmap level {}: {}", level, e);
                }
            }
        }
    }

    // Show statistics
    info!("\n--- Texture Streaming Statistics ---");
    let stats = texture_manager.get_statistics().await;
    let total_memory: u64 = stats.values().map(|s| s.bytes_loaded).sum();
    let total_chunks: usize = stats.values().map(|s| s.chunks_loaded).sum();

    info!("Total textures loaded: {}", stats.len());
    info!("Total memory used: {} MB", total_memory / (1024 * 1024));
    info!("Total chunks loaded: {}", total_chunks);

    for (ekey, stat) in stats {
        let cache_ratio =
            stat.cache_hits as f64 / (stat.cache_hits + stat.cache_misses).max(1) as f64;
        info!(
            "  Texture {}: {} KB loaded, {:.1}% cache hit rate",
            ekey,
            stat.bytes_loaded / 1024,
            cache_ratio * 100.0
        );
    }

    // Simulate unloading textures (e.g., when changing zones)
    info!("\nUnloading textures...");
    for ekey in &files {
        texture_manager.unload_texture(ekey).await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    simulate_texture_loading().await?;

    info!("Texture streaming demo completed");
    Ok(())
}
