//! Server configuration management.
//!
//! This module handles loading and validating server configuration from CLI arguments,
//! environment variables, and validates consistency (e.g., TLS cert/key pairing).
//!
//! # Configuration Sources
//!
//! Configuration can be provided via:
//! - CLI arguments (`--http-bind`, `--tcp-bind`, etc.)
//! - Environment variables (`CASCETTE_RIBBIT_HTTP_BIND`, etc.)
//! - Default values
//!
//! # Example
//!
//! ```no_run
//! use cascette_ribbit::ServerConfig;
//!
//! // Load from CLI args and environment
//! let config = ServerConfig::from_args();
//!
//! // Validate configuration
//! config.validate().expect("Invalid configuration");
//!
//! println!("HTTP server will bind to: {}", config.http_bind);
//! println!("TCP server will bind to: {}", config.tcp_bind);
//! println!("TLS enabled: {}", config.has_tls());
//! ```

use crate::database::BuildRecord;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Server configuration loaded from CLI args and environment variables.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "cascette-ribbit",
    about = "Complete Ribbit server for NGDP/CASC installations",
    version
)]
pub struct ServerConfig {
    /// HTTP/HTTPS bind address
    #[arg(
        long,
        env = "CASCETTE_RIBBIT_HTTP_BIND",
        default_value = "0.0.0.0:8080"
    )]
    pub http_bind: SocketAddr,

    /// TCP bind address (Ribbit v1/v2)
    #[arg(long, env = "CASCETTE_RIBBIT_TCP_BIND", default_value = "0.0.0.0:1119")]
    pub tcp_bind: SocketAddr,

    /// Path to builds JSON database
    #[arg(long, env = "CASCETTE_RIBBIT_BUILDS", default_value = "./builds.json")]
    pub builds: PathBuf,

    /// Default CDN hostname(s), space-separated
    #[arg(
        long,
        env = "CASCETTE_RIBBIT_CDN_HOSTS",
        default_value = "cdn.arctium.tools"
    )]
    pub cdn_hosts: String,

    /// Default CDN path prefix
    #[arg(long, env = "CASCETTE_RIBBIT_CDN_PATH", default_value = "tpr/wow")]
    pub cdn_path: String,

    /// TLS certificate file path (optional, enables HTTPS)
    #[arg(long, env = "CASCETTE_RIBBIT_TLS_CERT")]
    pub tls_cert: Option<PathBuf>,

    /// TLS private key file path (required if `tls_cert` is set)
    #[arg(long, env = "CASCETTE_RIBBIT_TLS_KEY")]
    pub tls_key: Option<PathBuf>,
}

impl ServerConfig {
    /// Parse configuration from command-line arguments.
    #[must_use]
    pub fn from_args() -> Self {
        Self::parse()
    }

    /// Get default CDN configuration.
    #[must_use]
    pub fn default_cdn_config(&self) -> CdnConfig {
        CdnConfig {
            hosts: self.cdn_hosts.clone(),
            path: self.cdn_path.clone(),
            servers: format!(
                "https://{}/?fallbackProtocol=http",
                self.cdn_hosts
                    .split_whitespace()
                    .next()
                    .unwrap_or("cdn.arctium.tools")
            ),
            config_path: self.cdn_path.clone(),
        }
    }

    /// Check if TLS is configured.
    #[must_use]
    pub const fn has_tls(&self) -> bool {
        self.tls_cert.is_some() && self.tls_key.is_some()
    }

    /// Validate configuration.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if:
    /// - Builds file doesn't exist
    /// - TLS cert is provided without key (or vice versa)
    /// - TLS cert/key files don't exist
    pub fn validate(&self) -> Result<(), crate::error::ConfigError> {
        use crate::error::ConfigError;

        // Validate builds file exists
        if !self.builds.exists() {
            return Err(ConfigError::MissingRequired(format!(
                "builds file not found: {}",
                self.builds.display()
            )));
        }

        // Validate TLS configuration
        match (&self.tls_cert, &self.tls_key) {
            (Some(_cert), None) => {
                return Err(ConfigError::TlsConfig(
                    "TLS certificate provided without private key".to_string(),
                ));
            }
            (None, Some(_key)) => {
                return Err(ConfigError::TlsConfig(
                    "TLS private key provided without certificate".to_string(),
                ));
            }
            (Some(cert), Some(key)) => {
                if !cert.exists() {
                    return Err(ConfigError::TlsConfig(format!(
                        "TLS certificate file not found: {}",
                        cert.display()
                    )));
                }
                if !key.exists() {
                    return Err(ConfigError::TlsConfig(format!(
                        "TLS private key file not found: {}",
                        key.display()
                    )));
                }
            }
            (None, None) => {
                // No TLS configured, which is valid
            }
        }

        Ok(())
    }
}

/// CDN configuration for responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnConfig {
    /// CDN hostname(s), space-separated
    pub hosts: String,

    /// CDN path prefix (e.g., "tpr/wow")
    pub path: String,

    /// Full server URLs with parameters, space-separated
    pub servers: String,

    /// Config path (usually same as path)
    pub config_path: String,
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            hosts: "cdn.arctium.tools".to_string(),
            path: "tpr/wow".to_string(),
            servers: "https://cdn.arctium.tools/?fallbackProtocol=http".to_string(),
            config_path: "tpr/wow".to_string(),
        }
    }
}

impl CdnConfig {
    /// Resolve CDN configuration for a specific build.
    ///
    /// Uses product-specific CDN path from `BuildRecord` if available,
    /// otherwise falls back to default configuration.
    #[must_use]
    pub fn resolve_for_build(build: &BuildRecord, default_config: &Self) -> Self {
        Self {
            hosts: default_config.hosts.clone(),
            path: build
                .cdn_path
                .clone()
                .unwrap_or_else(|| default_config.path.clone()),
            servers: default_config.servers.clone(),
            config_path: build
                .cdn_path
                .clone()
                .unwrap_or_else(|| default_config.config_path.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdn_config_default() {
        let config = CdnConfig::default();
        assert_eq!(config.hosts, "cdn.arctium.tools");
        assert_eq!(config.path, "tpr/wow");
    }

    #[test]
    fn test_cdn_config_resolve_with_override() {
        let default_config = CdnConfig::default();
        let build = BuildRecord {
            id: 1,
            product: "wow_classic".to_string(),
            version: "1.0.0".to_string(),
            build: "1".to_string(),
            build_config: "0123456789abcdef0123456789abcdef".to_string(),
            cdn_config: "fedcba9876543210fedcba9876543210".to_string(),
            keyring: None,
            product_config: None,
            build_time: "2024-01-01T00:00:00+00:00".to_string(),
            encoding_ekey: "aaaabbbbccccddddeeeeffffgggghhh1".to_string(),
            root_ekey: "aaaabbbbccccddddeeeeffffgggghhh2".to_string(),
            install_ekey: "aaaabbbbccccddddeeeeffffgggghhh3".to_string(),
            download_ekey: "aaaabbbbccccddddeeeeffffgggghhh4".to_string(),
            cdn_path: Some("tpr/wow_classic".to_string()),
        };

        let resolved = CdnConfig::resolve_for_build(&build, &default_config);
        assert_eq!(resolved.path, "tpr/wow_classic");
        assert_eq!(resolved.hosts, "cdn.arctium.tools");
    }

    #[test]
    fn test_cdn_config_resolve_without_override() {
        let default_config = CdnConfig::default();
        let build = BuildRecord {
            id: 1,
            product: "wow".to_string(),
            version: "1.0.0".to_string(),
            build: "1".to_string(),
            build_config: "0123456789abcdef0123456789abcdef".to_string(),
            cdn_config: "fedcba9876543210fedcba9876543210".to_string(),
            keyring: None,
            product_config: None,
            build_time: "2024-01-01T00:00:00+00:00".to_string(),
            encoding_ekey: "aaaabbbbccccddddeeeeffffgggghhh1".to_string(),
            root_ekey: "aaaabbbbccccddddeeeeffffgggghhh2".to_string(),
            install_ekey: "aaaabbbbccccddddeeeeffffgggghhh3".to_string(),
            download_ekey: "aaaabbbbccccddddeeeeffffgggghhh4".to_string(),
            cdn_path: None,
        };

        let resolved = CdnConfig::resolve_for_build(&build, &default_config);
        assert_eq!(resolved.path, "tpr/wow");
        assert_eq!(resolved.hosts, "cdn.arctium.tools");
    }

    #[test]
    fn test_server_config_has_tls() {
        let mut config = ServerConfig {
            http_bind: "0.0.0.0:8080".parse().unwrap(),
            tcp_bind: "0.0.0.0:1119".parse().unwrap(),
            builds: PathBuf::from("./builds.json"),
            cdn_hosts: "cdn.example.com".to_string(),
            cdn_path: "tpr/test".to_string(),
            tls_cert: Some(PathBuf::from("cert.pem")),
            tls_key: Some(PathBuf::from("key.pem")),
        };

        assert!(config.has_tls());

        config.tls_cert = None;
        assert!(!config.has_tls());
    }
}
