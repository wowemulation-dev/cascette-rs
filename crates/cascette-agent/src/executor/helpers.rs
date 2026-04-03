//! Shared executor utilities for protocol queries, progress bridging, and
//! configuration construction.
//!
//! These helpers are used across install, update, repair, and verify executors
//! to avoid duplicating Ribbit query logic and progress mapping.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Timeout for Ribbit metadata resolution queries.
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

use cascette_crypto::{TactKey, TactKeyProvider, TactKeyStore};
use cascette_formats::config::KeyringConfig;
use cascette_import::ImportProvider;
use cascette_installation::ProgressEvent;
use cascette_installation::config::InstallConfig;
use cascette_protocol::{CdnClient, CdnEndpoint, RibbitTactClient};

use crate::error::{AgentError, AgentResult};
use crate::models::progress::Progress;
use crate::server::router::AppState;

/// Metadata resolved from Ribbit for a product in a given region.
pub struct ProductMetadata {
    /// Build config hash (hex).
    pub build_config: String,
    /// CDN config hash (hex).
    pub cdn_config: String,
    /// CDN path (e.g., "tpr/wow").
    pub cdn_path: String,
    /// Resolved CDN endpoints in priority order.
    pub endpoints: Vec<CdnEndpoint>,
    /// Human-readable version string (e.g., "1.15.7.63696").
    pub version_name: String,
    /// Build ID.
    pub build_id: String,
    /// Keyring config hash from Ribbit (optional, not all products have one).
    pub keyring_hash: Option<String>,
}

/// Query Ribbit for a product's version and CDN information.
///
/// Resolves build_config/cdn_config hashes, CDN endpoints, and version name
/// for the given product and region. If `cdn_overrides` is provided, those
/// endpoints are used instead of the Ribbit-advertised hosts.
///
/// Times out after 30 seconds if Ribbit is unreachable.
pub async fn resolve_product_metadata(
    ribbit: &RibbitTactClient,
    cdn_client_ref: &CdnClient,
    product: &str,
    region: &str,
    cdn_overrides: Option<&[CdnEndpoint]>,
) -> AgentResult<ProductMetadata> {
    tokio::time::timeout(
        METADATA_TIMEOUT,
        resolve_product_metadata_inner(ribbit, cdn_client_ref, product, region, cdn_overrides),
    )
    .await
    .map_err(|_| {
        AgentError::Timeout(format!(
            "metadata resolution for {product} timed out after {METADATA_TIMEOUT:?}"
        ))
    })?
}

async fn resolve_product_metadata_inner(
    ribbit: &RibbitTactClient,
    cdn_client_ref: &CdnClient,
    product: &str,
    region: &str,
    cdn_overrides: Option<&[CdnEndpoint]>,
) -> AgentResult<ProductMetadata> {
    // Query versions
    let versions_endpoint = format!("v1/products/{product}/versions");
    let versions = ribbit.query(&versions_endpoint).await?;

    // Find the row matching the requested region.
    // Use get_raw_by_name() throughout because the v2 BPSV response declares
    // typed fields (e.g. BuildConfig!HEX:16) which parse into BpsvValue::Hex,
    // not BpsvValue::String. The raw accessor returns the unparsed string for
    // any field type.
    let version_row = versions
        .rows()
        .iter()
        .find(|row| {
            row.get_raw_by_name("Region", versions.schema())
                .is_some_and(|r| r.eq_ignore_ascii_case(region))
        })
        .or_else(|| versions.rows().first())
        .ok_or_else(|| {
            AgentError::InvalidConfig(format!("no version data for product {product}"))
        })?;

    let build_config = version_row
        .get_raw_by_name("BuildConfig", versions.schema())
        .ok_or_else(|| AgentError::InvalidConfig("missing BuildConfig field".to_string()))?
        .to_string();

    let cdn_config = version_row
        .get_raw_by_name("CDNConfig", versions.schema())
        .ok_or_else(|| AgentError::InvalidConfig("missing CDNConfig field".to_string()))?
        .to_string();

    let version_name = version_row
        .get_raw_by_name("VersionsName", versions.schema())
        .unwrap_or("unknown")
        .to_string();

    let build_id = version_row
        .get_raw_by_name("BuildId", versions.schema())
        .unwrap_or("0")
        .to_string();

    let keyring_hash = version_row
        .get_raw_by_name("KeyRing", versions.schema())
        .map(ToString::to_string);

    // Resolve CDN endpoints from Ribbit, then prepend any operator overrides.
    let cdns_endpoint = format!("v1/products/{product}/cdns");
    let cdns = ribbit.query(&cdns_endpoint).await?;

    let mut resolved_endpoints = Vec::new();
    let mut cdn_path = String::new();

    for row in cdns.rows() {
        // Filter to matching region if possible
        let row_region = row.get_raw_by_name("Name", cdns.schema()).unwrap_or("");

        if !row_region.eq_ignore_ascii_case(region) && !resolved_endpoints.is_empty() {
            continue;
        }

        if let Ok(ep) = CdnClient::endpoint_from_bpsv_row(row, cdns.schema()) {
            if cdn_path.is_empty() {
                cdn_path.clone_from(&ep.path);
            }
            resolved_endpoints.push(ep);
        }
    }

    if resolved_endpoints.is_empty() {
        return Err(AgentError::InvalidConfig(format!(
            "no CDN endpoints found for product {product}"
        )));
    }

    // Prepend operator-configured overrides so they are tried first,
    // with Ribbit-advertised endpoints as fallback.
    let (endpoints, cdn_path) = if let Some(overrides) = cdn_overrides {
        let all = overrides
            .iter()
            .cloned()
            .chain(resolved_endpoints)
            .collect::<Vec<_>>();
        // Use the override path if provided, otherwise keep the Ribbit path.
        let path = overrides.first().map_or(cdn_path, |e| e.path.clone());
        (all, path)
    } else {
        (resolved_endpoints, cdn_path)
    };

    // Suppress unused variable warning -- cdn_client_ref reserved for future
    // content-type resolution if needed.
    let _ = cdn_client_ref;

    debug!(
        product,
        version = %version_name,
        build_config = %build_config,
        cdn_path = %cdn_path,
        endpoints = endpoints.len(),
        "resolved product metadata"
    );

    Ok(ProductMetadata {
        build_config,
        cdn_config,
        cdn_path,
        endpoints,
        version_name,
        build_id,
        keyring_hash,
    })
}

/// Fetch and parse a keyring config from CDN.
///
/// The keyring is a config-type blob addressed by the hash from the Ribbit
/// `KeyRing` column. Returns `None` on fetch or parse failure (non-fatal).
pub async fn fetch_keyring(
    cdn_client: &CdnClient,
    endpoints: &[CdnEndpoint],
    keyring_hash: &str,
) -> Option<KeyringConfig> {
    use cascette_protocol::ContentType;

    let key = match hex::decode(keyring_hash) {
        Ok(k) => k,
        Err(e) => {
            warn!(hash = keyring_hash, error = %e, "invalid keyring hash");
            return None;
        }
    };

    let data = match cdn_client
        .download_from_endpoints(endpoints, ContentType::Config, &key)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            warn!(hash = keyring_hash, error = %e, "failed to fetch keyring config");
            return None;
        }
    };

    match KeyringConfig::parse(std::io::Cursor::new(&data)) {
        Ok(config) => {
            debug!(keys = config.entries().len(), "keyring config loaded");
            Some(config)
        }
        Err(e) => {
            warn!(error = %e, "failed to parse keyring config");
            None
        }
    }
}

/// Build a key provider from hardcoded keys and an optional keyring config.
///
/// Merges hardcoded WoW keys (from `TactKeyStore::new()`) with any keys
/// found in the keyring config blob fetched from CDN.
pub fn build_key_provider(
    keyring: Option<&KeyringConfig>,
) -> Arc<dyn TactKeyProvider + Send + Sync> {
    let mut store = TactKeyStore::new();

    if let Some(kr) = keyring {
        for entry in kr.entries() {
            let Ok(id) = u64::from_str_radix(&entry.key_id, 16) else {
                continue;
            };
            let Ok(value_bytes) = hex::decode(&entry.key_value) else {
                continue;
            };
            let Ok(key): Result<[u8; 16], _> = value_bytes.try_into() else {
                continue;
            };
            store.add(TactKey::new(id, key));
        }
    }

    Arc::new(store)
}

/// Tracks per-file download start times and computes an exponential moving
/// average of download speed across completed files.
///
/// Uses a simple EMA with α = 0.2: recent samples have more weight than old
/// ones, giving a smooth speed that reacts to network changes without being
/// too jittery.
struct SpeedTracker {
    /// (start_instant, file_size_bytes) keyed by file path.
    /// Populated on `FileDownloading`, consumed on `FileComplete`.
    in_flight: HashMap<String, (Instant, u64)>,
    /// Current exponential moving average speed in bytes/sec.
    ema_bps: Option<f64>,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            ema_bps: None,
        }
    }

    /// Record the start of a download for `path` with expected `size` bytes.
    fn start(&mut self, path: String, size: u64) {
        self.in_flight.insert(path, (Instant::now(), size));
    }

    /// Record completion of `path`, using the size stored at `start()`.
    ///
    /// Returns the updated EMA speed in bytes/sec, or `None` if timing
    /// data is unavailable or elapsed time is too short.
    fn complete_timed(&mut self, path: &str) -> Option<u64> {
        let (start, bytes) = self.in_flight.remove(path)?;
        self.record_sample(bytes, start.elapsed())
    }

    /// Record completion of `path` with an externally-supplied `bytes` count.
    ///
    /// Used for `FileLeeched` events where size is provided directly.
    fn complete(&mut self, path: &str, bytes: u64) -> Option<u64> {
        let (start, _) = self.in_flight.remove(path)?;
        self.record_sample(bytes, start.elapsed())
    }

    fn record_sample(&mut self, bytes: u64, elapsed: std::time::Duration) -> Option<u64> {
        // Ignore sub-10ms completions — they produce unreliable rate samples.
        if elapsed.as_millis() < 10 {
            return self.ema_bps.map(|v| v as u64);
        }

        #[allow(clippy::cast_precision_loss)]
        let sample_bps = bytes as f64 / elapsed.as_secs_f64();

        #[allow(clippy::items_after_statements)]
        const ALPHA: f64 = 0.2;
        let new_ema = match self.ema_bps {
            None => sample_bps,
            Some(prev) => ALPHA.mul_add(sample_bps, (1.0 - ALPHA) * prev),
        };
        self.ema_bps = Some(new_ema);

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        Some(new_ema as u64)
    }
}

/// A progress bridge that maps `cascette_installation::ProgressEvent` values
/// to the agent's `Progress` model and periodically flushes to SQLite.
pub struct ProgressBridge {
    progress: Arc<Mutex<Progress>>,
    speed: Arc<std::sync::Mutex<SpeedTracker>>,
}

impl ProgressBridge {
    /// Create a new progress bridge for an operation.
    ///
    /// Returns the bridge and a flush handle that should be awaited on
    /// completion (or aborted on cancellation) to stop the background
    /// flush task.
    pub fn new(
        operation_id: uuid::Uuid,
        state: &Arc<AppState>,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let progress = Arc::new(Mutex::new(Progress::new("initializing".to_string(), 0, 0)));
        let speed = Arc::new(std::sync::Mutex::new(SpeedTracker::new()));

        let flush_progress = Arc::clone(&progress);
        let flush_state = Arc::clone(state);
        let flush_handle = tokio::spawn(async move {
            let state = flush_state;
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                let snapshot = flush_progress.lock().await.clone();
                if let Ok(mut op) = state.queue.get(&operation_id.to_string()).await {
                    op.update_progress(snapshot);
                    let _ = state.queue.update(&op).await;
                }
            }
        });

        (Self { progress, speed }, flush_handle)
    }

    /// Create the progress callback closure for passing to pipeline `run()`.
    ///
    /// The returned callback is `Fn(ProgressEvent) + Send + Sync` as required
    /// by the installation pipelines. It updates the shared `Progress` state
    /// which the flush task periodically writes to SQLite.
    ///
    /// Speed is measured per-file: `FileDownloading` records a start timestamp,
    /// `FileComplete` computes elapsed time and bytes/sec, then feeds the result
    /// into an exponential moving average (α = 0.2). The EMA is written into
    /// `Progress::speed_bps`, which triggers `recalculate_eta()`.
    pub fn callback(&self) -> impl Fn(ProgressEvent) + Send + Sync {
        let progress = Arc::clone(&self.progress);
        let speed = Arc::clone(&self.speed);

        move |event: ProgressEvent| {
            // Use try_lock to avoid blocking the pipeline thread.
            // If the lock is held by the flush task, skip this update --
            // the next event will catch up.
            let Ok(mut p) = progress.try_lock() else {
                return;
            };

            match event {
                ProgressEvent::MetadataResolving { .. } => {
                    p.phase = "resolving".to_string();
                }
                ProgressEvent::MetadataResolved {
                    artifacts,
                    total_bytes,
                } => {
                    p.phase = "downloading".to_string();
                    p.bytes_total = total_bytes;
                    p.files_total = artifacts;
                }
                ProgressEvent::ArchiveIndexDownloading { index, total } => {
                    p.phase = "downloading indices".to_string();
                    p.current_file = Some(format!("index {index}/{total}"));
                }
                ProgressEvent::ArchiveIndexComplete { .. } => {}
                ProgressEvent::FileDownloading { ref path, size } => {
                    // Record start time and expected size for speed measurement.
                    if let Ok(mut s) = speed.try_lock() {
                        s.start(path.clone(), size);
                    }
                    p.phase = "downloading".to_string();
                    p.current_file = Some(path.clone());
                    let new_bytes = p.bytes_done.saturating_add(size);
                    p.update_bytes(new_bytes);
                }
                ProgressEvent::FileComplete { ref path } => {
                    // Compute per-file speed from the Instant+size stored at
                    // FileDownloading, feed the sample into the EMA.
                    let ema = speed
                        .try_lock()
                        .ok()
                        .and_then(|mut s| s.complete_timed(path));
                    if let Some(bps) = ema {
                        p.set_speed(bps);
                    }
                    let new_files = p.files_done.saturating_add(1);
                    p.update_files(new_files);
                }
                ProgressEvent::ExtractComplete { .. }
                | ProgressEvent::RepairComplete { .. }
                | ProgressEvent::FileFailed { .. }
                | ProgressEvent::LeechFailed { .. }
                | ProgressEvent::PatchFailed { .. }
                | ProgressEvent::PatchApplied { .. } => {
                    let new_files = p.files_done.saturating_add(1);
                    p.update_files(new_files);
                }
                ProgressEvent::FileLeeched { ref path, bytes } => {
                    // Leeched files also contribute to speed measurement.
                    let ema = speed
                        .try_lock()
                        .ok()
                        .and_then(|mut s| s.complete(path, bytes));
                    if let Some(bps) = ema {
                        p.set_speed(bps);
                    }
                    let new_files = p.files_done.saturating_add(1);
                    p.update_files(new_files);
                }
                ProgressEvent::CheckpointSaved {
                    completed,
                    remaining,
                } => {
                    p.current_file =
                        Some(format!("checkpoint: {completed} done, {remaining} left"));
                }
                ProgressEvent::VerifyResult { ref path, valid } => {
                    p.phase = "verifying".to_string();
                    p.current_file = Some(path.clone());
                    if valid {
                        let new_files = p.files_done.saturating_add(1);
                        p.update_files(new_files);
                    }
                }
                ProgressEvent::ExtractStarted { ref path } => {
                    p.phase = "extracting".to_string();
                    p.current_file = Some(path.clone());
                }
                ProgressEvent::RepairDownloading { ref path } => {
                    p.phase = "repairing".to_string();
                    p.current_file = Some(path.clone());
                }
                ProgressEvent::UpdateClassified {
                    required,
                    total_bytes,
                    ..
                } => {
                    p.phase = "updating".to_string();
                    p.files_total = required;
                    p.bytes_total = total_bytes;
                }
                ProgressEvent::PatchApplying { ref path } => {
                    p.phase = "patching".to_string();
                    p.current_file = Some(path.clone());
                }
            }
        }
    }
}

/// Build an `UpdateConfig` with standard defaults.
///
/// Requires both base (currently installed) and target (desired) config hashes.
#[allow(clippy::too_many_arguments)]
pub fn build_update_config(
    product: &str,
    install_path: &str,
    cdn_path: &str,
    endpoints: Vec<CdnEndpoint>,
    region: &str,
    locale: &str,
    base_build_config: String,
    base_cdn_config: String,
    target_build_config: String,
    target_cdn_config: String,
) -> cascette_installation::config::UpdateConfig {
    let mut config = cascette_installation::config::UpdateConfig::new(
        product.to_string(),
        PathBuf::from(install_path),
        cdn_path.to_string(),
        base_build_config,
        base_cdn_config,
        target_build_config,
        target_cdn_config,
    );
    config.endpoints = endpoints;
    config.region = region.to_string();
    config.locale = locale.to_string();
    config.max_connections_per_host = 3;
    config.max_connections_global = 12;
    config.resume = true;
    config
}

/// Build an `InstallConfig` with standard defaults.
#[allow(clippy::too_many_arguments)]
pub fn build_install_config(
    product: &str,
    install_path: &str,
    cdn_path: &str,
    endpoints: Vec<CdnEndpoint>,
    region: &str,
    locale: &str,
    build_config: Option<String>,
    cdn_config: Option<String>,
    game_subfolder: Option<String>,
) -> InstallConfig {
    let mut config = InstallConfig::new(
        product.to_string(),
        PathBuf::from(install_path),
        cdn_path.to_string(),
    );
    config.endpoints = endpoints;
    config.region = region.to_string();
    config.locale = locale.to_string();
    config.build_config = build_config;
    config.cdn_config = cdn_config;
    config.game_subfolder = game_subfolder;
    config.max_connections_per_host = 3;
    config.max_connections_global = 12;
    config.resume = true;
    config
}

/// Look up a version name from wago.tools given a build_config hash.
///
/// Searches the build database for the product, finds the entry whose
/// `build_config` metadata matches the given hash, and returns the version
/// string (e.g. "1.13.2.31650"). Returns `None` if not found.
pub async fn resolve_version_from_wago(
    wago: &tokio::sync::RwLock<cascette_import::WagoProvider>,
    product: &str,
    build_config: &str,
) -> Option<String> {
    let builds = wago.read().await.get_builds(product).await.ok()?;
    builds.iter().find_map(|b| {
        let bc = b.metadata.get("build_config")?;
        if bc == build_config {
            Some(b.version.clone())
        } else {
            None
        }
    })
}

/// Resolve CDN path and official endpoints from Ribbit for a product/region.
///
/// This queries only the CDN endpoint list (not versions), which is
/// product-level and version-independent. Used when custom build_config/cdn_config
/// are specified and the full `resolve_product_metadata` (which queries versions
/// too) should be skipped.
///
/// Times out after 30 seconds if Ribbit is unreachable.
pub async fn resolve_cdn_info(
    ribbit: &RibbitTactClient,
    product: &str,
    region: &str,
) -> AgentResult<(String, Vec<CdnEndpoint>)> {
    tokio::time::timeout(
        METADATA_TIMEOUT,
        resolve_cdn_info_inner(ribbit, product, region),
    )
    .await
    .map_err(|_| {
        AgentError::Timeout(format!(
            "CDN info resolution for {product} timed out after {METADATA_TIMEOUT:?}"
        ))
    })?
}

async fn resolve_cdn_info_inner(
    ribbit: &RibbitTactClient,
    product: &str,
    region: &str,
) -> AgentResult<(String, Vec<CdnEndpoint>)> {
    let cdns_endpoint = format!("v1/products/{product}/cdns");
    let cdns = ribbit.query(&cdns_endpoint).await?;

    let mut endpoints = Vec::new();
    let mut cdn_path = String::new();

    for row in cdns.rows() {
        let row_region = row.get_raw_by_name("Name", cdns.schema()).unwrap_or("");

        if !row_region.eq_ignore_ascii_case(region) && !endpoints.is_empty() {
            continue;
        }

        if let Ok(ep) = CdnClient::endpoint_from_bpsv_row(row, cdns.schema()) {
            if cdn_path.is_empty() {
                cdn_path.clone_from(&ep.path);
            }
            endpoints.push(ep);
        }
    }

    if endpoints.is_empty() {
        return Err(AgentError::InvalidConfig(format!(
            "no CDN endpoints found for product {product}"
        )));
    }

    Ok((cdn_path, endpoints))
}
