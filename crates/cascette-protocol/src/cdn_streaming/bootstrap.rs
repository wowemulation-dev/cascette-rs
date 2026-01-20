//! CDN bootstrap module for handling Ribbit response data
//!
//! Provides functionality to parse CDN server information from Ribbit responses
//! and configure streaming CDN clients with dynamically discovered endpoints.

#[cfg(feature = "streaming")]
use super::{
    error::{StreamingError, StreamingResult},
    http::CdnServer,
};
#[cfg(feature = "streaming")]
use crate::bpsv::{BpsvRow, BpsvSchema};
#[cfg(feature = "streaming")]
use std::collections::HashMap;

/// CDN bootstrap configuration from Ribbit responses
///
/// Contains parsed CDN server information and paths extracted from
/// Ribbit `/cdns` endpoint responses in BPSV format.
#[cfg(feature = "streaming")]
#[derive(Debug, Clone)]
pub struct CdnBootstrap {
    /// CDN servers discovered from Ribbit
    pub servers: Vec<CdnServer>,
    /// Product-specific CDN paths (product -> path)
    pub paths: HashMap<String, String>,
    /// Preferred CDN hosts in priority order
    pub preferred_hosts: Vec<String>,
    /// Whether configuration comes from official Ribbit (vs fallback)
    pub is_official: bool,
}

/// CDN entry from Ribbit BPSV response
///
/// Represents a single CDN configuration entry with all fields
/// that may appear in `/cdns` endpoint responses.
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq)]
pub struct CdnEntry {
    /// CDN name/identifier
    pub name: String,
    /// CDN path for this configuration
    pub path: String,
    /// Space-separated list of CDN hosts
    pub hosts: String,
    /// Configuration servers (optional)
    pub config_path: Option<String>,
}

#[cfg(feature = "streaming")]
impl CdnBootstrap {
    /// Create empty bootstrap configuration
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            paths: HashMap::new(),
            preferred_hosts: Vec::new(),
            is_official: false,
        }
    }

    /// Parse CDN configuration from Ribbit BPSV response
    ///
    /// # Arguments
    /// * `bpsv_data` - Raw BPSV data from Ribbit `/cdns` endpoint
    /// * `product` - Product name to filter CDN entries (optional)
    ///
    /// # Returns
    /// Parsed CDN bootstrap configuration
    ///
    /// # Errors
    /// Returns `StreamingError` if BPSV parsing fails or data is invalid
    pub fn from_ribbit_response(bpsv_data: &[u8], product: Option<&str>) -> StreamingResult<Self> {
        let bpsv_str =
            std::str::from_utf8(bpsv_data).map_err(|e| StreamingError::Configuration {
                reason: format!("Invalid UTF-8 in Ribbit BPSV data: {e}"),
            })?;

        let document = crate::bpsv::parse(bpsv_str).map_err(|e| StreamingError::Configuration {
            reason: format!("Failed to parse Ribbit BPSV data: {e}"),
        })?;

        let mut bootstrap = Self::new();
        bootstrap.is_official = true;

        let schema = document.schema();

        // Parse each BPSV row as CDN entry
        for row in document.rows() {
            let cdn_entry = Self::parse_cdn_entry(row, schema)?;

            // Filter by product if specified
            if let Some(product) = product {
                if !cdn_entry.name.contains(product) {
                    continue;
                }
            }

            // Extract CDN servers from hosts field
            let servers = Self::parse_cdn_hosts(&cdn_entry.hosts)?;
            bootstrap.servers.extend(servers);

            // Cache path for this CDN entry
            if !cdn_entry.path.is_empty() {
                bootstrap
                    .paths
                    .insert(cdn_entry.name.clone(), cdn_entry.path.clone());
            }

            // Add to preferred hosts list
            for host in cdn_entry.hosts.split_whitespace() {
                if !bootstrap.preferred_hosts.contains(&host.to_string()) {
                    bootstrap.preferred_hosts.push(host.to_string());
                }
            }
        }

        // Sort servers by priority (prioritize HTTPS-capable servers)
        bootstrap.servers.sort_by_key(|server| {
            if server.supports_https {
                server.priority
            } else {
                server.priority + 1000 // Deprioritize HTTP-only
            }
        });

        Ok(bootstrap)
    }

    /// Parse a single CDN entry from BPSV row
    fn parse_cdn_entry(row: &BpsvRow, schema: &BpsvSchema) -> StreamingResult<CdnEntry> {
        let name = row
            .get_by_name("Name", schema)
            .or_else(|| row.get_by_name("Region", schema))
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();

        let path = row
            .get_by_name("Path", schema)
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();

        let hosts = row
            .get_by_name("Hosts", schema)
            .or_else(|| row.get_by_name("Server", schema))
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();

        let config_path = row
            .get_by_name("ConfigPath", schema)
            .and_then(|v| v.as_string())
            .map(|s| s.to_string());

        if hosts.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "CDN entry missing hosts field".to_string(),
            });
        }

        Ok(CdnEntry {
            name,
            path,
            hosts,
            config_path,
        })
    }

    /// Parse CDN hosts from space-separated host list
    fn parse_cdn_hosts(hosts_str: &str) -> StreamingResult<Vec<CdnServer>> {
        let mut servers = Vec::new();
        let mut priority = 10; // Start with high priority

        for host in hosts_str.split_whitespace() {
            if host.is_empty() {
                continue;
            }

            // Determine if server supports HTTPS based on host patterns
            let supports_https = host.contains("blizzard.com")
                || host.contains("battle.net")
                || host.contains("wago.tools");

            let server = CdnServer::new(host.to_string(), supports_https, priority);

            servers.push(server);
            priority += 10; // Decrease priority for subsequent servers
        }

        Ok(servers)
    }

    /// Extract path for a specific product
    ///
    /// # Arguments
    /// * `product` - Product name to look up
    ///
    /// # Returns
    /// CDN path if found, None otherwise
    pub fn get_path(&self, product: &str) -> Option<&String> {
        // Try exact match first
        if let Some(path) = self.paths.get(product) {
            return Some(path);
        }

        // Try partial matches for product names
        for (key, path) in &self.paths {
            if key.contains(product) || product.contains(key) {
                return Some(path);
            }
        }

        None
    }

    /// Get primary CDN server (highest priority HTTPS server)
    pub fn primary_server(&self) -> Option<&CdnServer> {
        self.servers
            .iter()
            .filter(|server| server.supports_https)
            .min_by_key(|server| server.priority)
    }

    /// Merge with fallback configuration
    ///
    /// Combines this bootstrap configuration with fallback servers,
    /// preserving official servers with higher priority.
    ///
    /// # Arguments
    /// * `fallback` - Fallback CDN configuration
    ///
    /// # Returns
    /// Merged bootstrap configuration
    pub fn merge_with_fallback(mut self, fallback: CdnBootstrap) -> Self {
        let mut fallback_priority_offset = 1000;

        // Find highest priority in current servers
        if let Some(max_priority) = self.servers.iter().map(|s| s.priority).max() {
            fallback_priority_offset = max_priority + 100;
        }

        // Add fallback servers with lower priority
        for mut server in fallback.servers {
            // Skip if we already have this server
            if self.servers.iter().any(|s| s.host == server.host) {
                continue;
            }

            server.priority += fallback_priority_offset;
            self.servers.push(server);
        }

        // Merge paths (prefer official)
        for (product, path) in fallback.paths {
            if !self.paths.contains_key(&product) {
                self.paths.insert(product, path);
            }
        }

        // Re-sort servers by priority
        self.servers.sort_by_key(|server| server.priority);

        self
    }

    /// Create fallback configuration with community mirrors
    ///
    /// Provides a bootstrap configuration using well-known community
    /// CDN mirrors for when official Ribbit is unavailable.
    /// All mirrors support HTTPS connections.
    pub fn fallback_configuration() -> Self {
        let servers = vec![
            CdnServer::new("cdn.arctium.tools".to_string(), true, 100),
            CdnServer::new("casc.wago.tools".to_string(), true, 110),
            CdnServer::new("cdn.marlam.in".to_string(), true, 120),
        ];

        let mut paths = HashMap::new();
        // Common fallback paths for major products
        paths.insert("wow".to_string(), "tpr/wow".to_string());
        paths.insert("wowt".to_string(), "tpr/wowt".to_string());
        paths.insert("wow_classic".to_string(), "tpr/wow_classic".to_string());
        paths.insert(
            "wow_classic_era".to_string(),
            "tpr/wow_classic_era".to_string(),
        );

        let preferred_hosts = servers.iter().map(|s| s.host.clone()).collect();

        Self {
            servers,
            paths,
            preferred_hosts,
            is_official: false,
        }
    }

    /// Validate bootstrap configuration
    ///
    /// Checks that the configuration has at least one server and
    /// valid paths.
    pub fn validate(&self) -> StreamingResult<()> {
        if self.servers.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "Bootstrap configuration has no CDN servers".to_string(),
            });
        }

        if self.paths.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "Bootstrap configuration has no CDN paths".to_string(),
            });
        }

        // Validate server configurations
        for server in &self.servers {
            if server.host.is_empty() {
                return Err(StreamingError::Configuration {
                    reason: "CDN server has empty host".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get statistics about this configuration
    pub fn stats(&self) -> BootstrapStats {
        let https_servers = self.servers.iter().filter(|s| s.supports_https).count();
        #[allow(clippy::similar_names)]
        let http_servers = self.servers.len() - https_servers;

        BootstrapStats {
            total_servers: self.servers.len(),
            https_servers,
            http_servers,
            total_paths: self.paths.len(),
            is_official: self.is_official,
        }
    }
}

/// Statistics about bootstrap configuration
#[cfg(feature = "streaming")]
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapStats {
    /// Total number of CDN servers configured
    pub total_servers: usize,
    /// Number of HTTPS-capable servers
    pub https_servers: usize,
    /// Number of HTTP-only servers
    pub http_servers: usize,
    /// Total number of cached product paths
    pub total_paths: usize,
    /// Whether configuration comes from official Ribbit
    pub is_official: bool,
}

#[cfg(feature = "streaming")]
impl Default for CdnBootstrap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "streaming"))]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_bootstrap() {
        let bootstrap = CdnBootstrap::new();
        assert!(bootstrap.servers.is_empty());
        assert!(bootstrap.paths.is_empty());
        assert!(!bootstrap.is_official);
        assert!(bootstrap.validate().is_err());
    }

    #[test]
    fn test_fallback_configuration() {
        let bootstrap = CdnBootstrap::fallback_configuration();
        assert!(!bootstrap.servers.is_empty());
        assert!(!bootstrap.paths.is_empty());
        assert!(!bootstrap.is_official);
        assert!(bootstrap.validate().is_ok());

        let stats = bootstrap.stats();
        assert!(stats.total_servers > 0);
        assert!(stats.total_paths > 0);
        assert!(!stats.is_official);
    }

    #[test]
    fn test_cdn_host_parsing() {
        let hosts = "level3.blizzard.com edgecast.blizzard.com";
        let servers = CdnBootstrap::parse_cdn_hosts(hosts).expect("Operation should succeed");

        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].host, "level3.blizzard.com");
        assert_eq!(servers[1].host, "edgecast.blizzard.com");
        assert!(servers[0].supports_https);
        assert!(servers[1].supports_https);
        assert!(servers[0].priority < servers[1].priority);
    }

    #[test]
    fn test_path_lookup() {
        let mut bootstrap = CdnBootstrap::new();
        bootstrap
            .paths
            .insert("wow".to_string(), "tpr/wow".to_string());
        bootstrap
            .paths
            .insert("wow_classic".to_string(), "tpr/wow_classic".to_string());

        assert_eq!(bootstrap.get_path("wow"), Some(&"tpr/wow".to_string()));
        assert_eq!(
            bootstrap.get_path("wow_classic"),
            Some(&"tpr/wow_classic".to_string())
        );
        assert!(bootstrap.get_path("nonexistent").is_none());
    }

    #[test]
    fn test_bootstrap_merge() {
        let mut official = CdnBootstrap::new();
        official.servers.push(CdnServer::new(
            "official.blizzard.com".to_string(),
            true,
            10,
        ));
        official
            .paths
            .insert("wow".to_string(), "tpr/wow".to_string());
        official.is_official = true;

        let mut fallback = CdnBootstrap::new();
        fallback
            .servers
            .push(CdnServer::new("mirror.example.com".to_string(), false, 50));
        fallback
            .paths
            .insert("wow_classic".to_string(), "tpr/wow_classic".to_string());

        let merged = official.merge_with_fallback(fallback);

        assert_eq!(merged.servers.len(), 2);
        assert_eq!(merged.paths.len(), 2);
        assert!(merged.is_official);

        // Official server should have higher priority (lower number)
        assert!(merged.servers[0].host == "official.blizzard.com");
        assert!(merged.servers[1].priority > merged.servers[0].priority);
    }

    #[test]
    fn test_primary_server_selection() {
        let mut bootstrap = CdnBootstrap::new();
        bootstrap
            .servers
            .push(CdnServer::new("http.example.com".to_string(), false, 10));
        bootstrap
            .servers
            .push(CdnServer::new("https.example.com".to_string(), true, 20));

        let primary = bootstrap.primary_server();
        assert!(primary.is_some());
        assert_eq!(
            primary.expect("Operation should succeed").host,
            "https.example.com"
        );
        assert!(primary.expect("Operation should succeed").supports_https);
    }

    #[test]
    fn test_bootstrap_stats() {
        let mut bootstrap = CdnBootstrap::new();
        bootstrap
            .servers
            .push(CdnServer::new("https.example.com".to_string(), true, 10));
        bootstrap
            .servers
            .push(CdnServer::new("http.example.com".to_string(), false, 20));
        bootstrap
            .paths
            .insert("wow".to_string(), "tpr/wow".to_string());
        bootstrap.is_official = true;

        let stats = bootstrap.stats();
        assert_eq!(stats.total_servers, 2);
        assert_eq!(stats.https_servers, 1);
        assert_eq!(stats.http_servers, 1);
        assert_eq!(stats.total_paths, 1);
        assert!(stats.is_official);
    }
}
