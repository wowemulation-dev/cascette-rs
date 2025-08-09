//! NGDP client library
//!
//! This library provides the core functionality for the ngdp CLI tool.

pub mod cached_client;
pub mod cdn_config;
pub mod commands;
pub mod config_manager;
pub mod fallback_client;
pub mod output;
pub mod pattern_extraction;
pub mod wago_api;

/// Common test constants
pub mod test_constants {
    /// Example certificate hash used throughout tests and examples
    /// SKI: 5168ff90af0207753cccd9656462a212b859723b
    pub const EXAMPLE_CERT_HASH: &str = "5168ff90af0207753cccd9656462a212b859723b";
}

// Re-export command handlers
pub use crate::commands::{
    certs::handle as handle_certs, config::handle as handle_config,
    download::handle as handle_download, inspect::handle as handle_inspect,
    install::handle as handle_install, listfile::handle as handle_listfile,
    products::handle as handle_products, storage::handle as handle_storage,
};

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ProductsCommands {
    /// List all available products
    List {
        /// Filter by product name pattern
        #[arg(short, long)]
        filter: Option<String>,

        /// Region to query
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Show versions for a specific product
    Versions {
        /// Product name (e.g., wow, d3, agent)
        product: String,

        /// Region to query
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Show all regions
        #[arg(short, long)]
        all_regions: bool,

        /// Parse and show build configuration details
        #[arg(long)]
        parse_config: bool,
    },

    /// Show CDN configuration for a product
    Cdns {
        /// Product name
        product: String,

        /// Region to query
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Get detailed information about a product
    Info {
        /// Product name
        product: String,

        /// Region to query (omit to show all regions)
        #[arg(short, long)]
        region: Option<String>,
    },

    /// Show all historical builds for a product
    Builds {
        /// Product name (e.g., wow, wowt, wowxptr)
        product: String,

        /// Filter by version pattern
        #[arg(short, long)]
        filter: Option<String>,

        /// Show only builds from the last N days
        #[arg(long)]
        days: Option<u32>,

        /// Limit number of results (default: show all)
        #[arg(long)]
        limit: Option<usize>,

        /// Show only background download builds
        #[arg(long)]
        bgdl_only: bool,
    },
}

#[derive(Subcommand)]
pub enum StorageCommands {
    /// Initialize a new CASC storage
    Init {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Product to initialize for
        #[arg(short, long)]
        product: Option<String>,
    },

    /// Show storage information
    Info {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Show NGDP configuration information from WoW installation
    Config {
        /// Path to WoW installation directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Show detailed storage statistics
    Stats {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Verify storage integrity
    Verify {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Fix corrupted files
        #[arg(short, long)]
        fix: bool,
    },

    /// Read a file by EKey
    Read {
        /// Path to storage directory
        path: PathBuf,

        /// Encoding key (hex)
        ekey: String,

        /// Output file (defaults to stdout)
        #[arg(short = 'O', long)]
        output: Option<PathBuf>,
    },

    /// Write a file to storage
    Write {
        /// Path to storage directory
        path: PathBuf,

        /// Encoding key (hex)
        ekey: String,

        /// Input file (defaults to stdin)
        #[arg(short = 'I', long)]
        input: Option<PathBuf>,
    },

    /// List all files in storage
    List {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,

        /// Limit number of results
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },

    /// Rebuild storage indices
    Rebuild {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Force rebuild even if indices seem valid
        #[arg(short, long)]
        force: bool,
    },

    /// Optimize storage for performance
    Optimize {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Repair corrupted storage
    Repair {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Dry run (don't actually repair)
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Clean up unused data
    Clean {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Dry run (don't actually delete)
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Extract a file by EKey with optional filename resolution
    Extract {
        /// Encoding key (hex)
        ekey: String,

        /// Path to storage directory
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Output file path (optional)
        #[arg(short = 'O', long)]
        output: Option<PathBuf>,

        /// Path to community listfile for filename resolution
        #[arg(long)]
        listfile: Option<PathBuf>,

        /// Resolve filename using listfile and TACT manifests
        #[arg(long)]
        resolve_filename: bool,
    },

    /// Extract a file by FileDataID (requires TACT manifests)
    ExtractById {
        /// FileDataID to extract
        fdid: u32,

        /// Path to storage directory
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Output file path (optional)
        #[arg(short = 'O', long)]
        output: Option<PathBuf>,

        /// Path to root manifest file
        #[arg(long)]
        root_manifest: Option<PathBuf>,

        /// Path to encoding manifest file
        #[arg(long)]
        encoding_manifest: Option<PathBuf>,
    },

    /// Extract a file by filename (requires TACT manifests and listfile)
    ExtractByName {
        /// Filename to extract
        filename: String,

        /// Path to storage directory
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Output file path (optional)
        #[arg(short = 'O', long)]
        output: Option<PathBuf>,

        /// Path to root manifest file
        #[arg(long)]
        root_manifest: Option<PathBuf>,

        /// Path to encoding manifest file
        #[arg(long)]
        encoding_manifest: Option<PathBuf>,

        /// Path to community listfile for filename resolution
        #[arg(long)]
        listfile: Option<PathBuf>,
    },

    /// Load TACT manifests for enhanced operations
    LoadManifests {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Path to root manifest file
        #[arg(long)]
        root_manifest: Option<PathBuf>,

        /// Path to encoding manifest file
        #[arg(long)]
        encoding_manifest: Option<PathBuf>,

        /// Path to community listfile for filename resolution
        #[arg(long)]
        listfile: Option<PathBuf>,

        /// Locale to use for filtering (default: all)
        #[arg(long, default_value = "all")]
        locale: String,

        /// Only show info, don't persist
        #[arg(long)]
        info_only: bool,
    },
}

#[derive(Subcommand)]
pub enum ListfileCommands {
    /// Download the latest community listfile
    Download {
        /// Output directory for listfile
        #[arg(long, default_value = ".")]
        output: PathBuf,

        /// Force download even if file exists
        #[arg(short, long)]
        force: bool,
    },

    /// Show listfile information
    Info {
        /// Path to listfile
        #[arg(default_value = "community-listfile.csv")]
        path: PathBuf,
    },

    /// Search for files in listfile
    Search {
        /// Search pattern (regex)
        pattern: String,

        /// Path to listfile
        #[arg(default_value = "community-listfile.csv")]
        path: PathBuf,

        /// Case-insensitive search
        #[arg(short, long)]
        ignore_case: bool,

        /// Limit results
        #[arg(short, long, default_value = "50")]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub enum DownloadCommands {
    /// Download a specific build
    Build {
        /// Product name
        product: String,

        /// Build ID or version
        build: String,

        /// Output directory
        #[arg(long, default_value = ".")]
        output: PathBuf,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Dry run - show what would be downloaded without actually downloading
        #[arg(long)]
        dry_run: bool,

        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Download specific files
    Files {
        /// Product name
        product: String,

        /// File patterns to download
        patterns: Vec<String>,

        /// Output directory
        #[arg(long, default_value = ".")]
        output: PathBuf,

        /// Build ID or version
        #[arg(short, long)]
        build: Option<String>,

        /// Dry run - show what would be downloaded without actually downloading
        #[arg(long)]
        dry_run: bool,

        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Limit number of files to download
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Resume an interrupted download
    Resume {
        /// Session ID or path
        session: String,
    },

    /// Test resumable download with a known file (for testing)
    TestResume {
        /// File hash to download (32 hex chars)
        hash: String,

        /// CDN host
        #[arg(short = 'H', long, default_value = "blzddist1-a.akamaihd.net")]
        host: String,

        /// Output file path
        #[arg(long, default_value = "test_download.bin")]
        output: PathBuf,

        /// Enable resumable mode
        #[arg(short, long)]
        resumable: bool,
    },
}

#[derive(Subcommand)]
pub enum InstallCommands {
    /// Install a game or product
    Game {
        /// Product name (e.g., wow, wow_classic)
        product: String,

        /// Installation directory
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Specific build to install (defaults to latest)
        #[arg(short, long)]
        build: Option<String>,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Installation type
        #[arg(short = 't', long, value_enum, default_value = "minimal")]
        install_type: InstallType,

        /// Resume existing installation (detects .build.info and missing files)
        #[arg(long)]
        resume: bool,

        /// Verify installation after completion
        #[arg(short = 'v', long)]
        verify: bool,

        /// Dry run - show what would be installed without downloading
        #[arg(long)]
        dry_run: bool,

        /// Maximum concurrent downloads
        #[arg(long, default_value = "5")]
        max_concurrent: usize,

        /// Filter by tags (comma-separated, e.g., "Windows,enUS")
        #[arg(long)]
        tags: Option<String>,
    },

    /// Repair an existing installation by verifying and re-downloading corrupted files
    Repair {
        /// Installation directory containing .build.info
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Verify checksums of existing files
        #[arg(short = 'v', long)]
        verify_checksums: bool,

        /// Dry run - show what would be repaired without downloading
        #[arg(long)]
        dry_run: bool,

        /// Maximum concurrent downloads
        #[arg(long, default_value = "5")]
        max_concurrent: usize,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum InstallType {
    /// Only required files for basic functionality
    Minimal,
    /// All available content
    Full,
    /// Custom selection based on tags
    Custom,
    /// Only create .build.info and Data/config structure (no downloads)
    MetadataOnly,
}

#[derive(Subcommand)]
pub enum InspectCommands {
    /// Parse and display BPSV data
    Bpsv {
        /// Input file or URL
        input: String,

        /// Show raw data
        #[arg(short, long)]
        raw: bool,
    },

    /// Inspect build configuration
    BuildConfig {
        /// Product name
        product: String,

        /// Build ID
        build: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Inspect CDN configuration
    CdnConfig {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Inspect encoding file
    Encoding {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,

        /// Search for specific key (hex string)
        #[arg(long)]
        search: Option<String>,

        /// Limit number of entries shown
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Inspect install manifest
    Install {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Show all entries (not just summary)
        #[arg(long)]
        all: bool,
    },

    /// Inspect download manifest
    DownloadManifest {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Show priority files
        #[arg(long, default_value = "10")]
        priority_limit: usize,

        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Inspect size file
    Size {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Show largest files
        #[arg(long, default_value = "10")]
        largest: usize,

        /// Calculate size for tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Reset configuration to defaults
    Reset {
        /// Confirm reset
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum CertsCommands {
    /// Download a certificate by its SKI/hash
    Download {
        /// Subject Key Identifier or certificate hash
        ski: String,

        /// Output file (defaults to stdout)
        #[arg(long)]
        output: Option<PathBuf>,

        /// Region to query
        #[arg(short, long, default_value = "us")]
        region: String,

        /// Certificate format (pem or der)
        #[arg(short = 'F', long = "cert-format", value_enum, default_value = "pem")]
        cert_format: CertFormat,

        /// Show certificate details
        #[arg(short, long)]
        details: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum CertFormat {
    /// PEM format (text)
    Pem,
    /// DER format (binary)
    Der,
}

/// Output format options for the CLI
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum OutputFormat {
    /// Plain text output
    Text,
    /// JSON output
    Json,
    /// Pretty-printed JSON
    JsonPretty,
    /// Raw BPSV format
    Bpsv,
}

/// Context for command execution
#[derive(Clone, Debug)]
pub struct CommandContext {
    /// Output format
    pub format: OutputFormat,
    /// Whether to disable colors
    pub no_color: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_debug() {
        assert_eq!(format!("{:?}", OutputFormat::Text), "Text");
        assert_eq!(format!("{:?}", OutputFormat::Json), "Json");
        assert_eq!(format!("{:?}", OutputFormat::JsonPretty), "JsonPretty");
        assert_eq!(format!("{:?}", OutputFormat::Bpsv), "Bpsv");
    }
}
