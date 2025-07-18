//! NGDP client library
//!
//! This library provides the core functionality for the ngdp CLI tool.

pub mod cached_client;
pub mod cdn_config;
pub mod commands;
pub mod fallback_client;
pub mod output;
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

    /// Verify storage integrity
    Verify {
        /// Path to storage directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Fix corrupted files
        #[arg(short, long)]
        fix: bool,
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
    },

    /// Resume an interrupted download
    Resume {
        /// Session ID or path
        session: String,
    },
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

        /// Build config hash
        config: Option<String>,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Inspect CDN configuration
    CdnConfig {
        /// Product name
        product: String,

        /// CDN config hash
        config: Option<String>,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,
    },

    /// Show archives information
    Archives {
        /// Product name
        product: String,

        /// Region
        #[arg(short, long, default_value = "us")]
        region: String,        
    },

    /// Show encoding information
    Encoding {
        /// Path to encoding file
        file: PathBuf,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,
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
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
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
