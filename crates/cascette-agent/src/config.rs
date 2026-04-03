//! Configuration and CLI argument parsing for the agent service.
//!
//! CLI flags match the real Blizzard Agent.exe (version 3.13.3) where applicable,
//! with additional cascette-specific options.
//!
//! # Example
//!
//! ```no_run
//! use cascette_agent::config::AgentConfig;
//!
//! let config = AgentConfig::from_args();
//! println!("Listening on port {}", config.port());
//! ```

use std::path::PathBuf;

use clap::Parser;

/// Default HTTP listen port (matches real agent).
pub const DEFAULT_PORT: u16 = 1120;

/// Fallback ports when the default is unavailable.
pub const FALLBACK_PORTS: &[u16] = &[6881, 6882, 6883];

/// Default database filename.
pub const DEFAULT_DB_NAME: &str = "agent.db";

/// Agent service configuration loaded from CLI arguments and environment variables.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "cascette-agent",
    about = "Local HTTP agent service compatible with Blizzard Agent.exe",
    version
)]
pub struct AgentConfig {
    /// HTTP listen port (default 1120, fallback 6881-6883).
    #[arg(long, env = "CASCETTE_AGENT_PORT")]
    pub port: Option<u16>,

    /// Path to the SQLite database file.
    #[arg(long, env = "CASCETTE_AGENT_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Default locale for installations (e.g., "enUS").
    #[arg(long, env = "CASCETTE_AGENT_LOCALE", default_value = "enUS")]
    pub locale: String,

    /// Show the agent window (Windows-only, ignored on other platforms).
    #[arg(long)]
    pub show: bool,

    /// Allow command execution from HTTP API.
    #[arg(long)]
    pub allowcommands: bool,

    /// Skip self-update check on startup.
    #[arg(long)]
    pub skipupdate: bool,

    /// Log level override (trace, debug, info, warn, error).
    #[arg(long, env = "CASCETTE_AGENT_LOG_LEVEL")]
    pub loglevel: Option<String>,

    /// Session identifier for launcher communication.
    #[arg(long)]
    pub session: Option<String>,

    /// Patch check frequency in seconds.
    #[arg(long, env = "CASCETTE_AGENT_PATCH_FREQ", default_value = "300")]
    pub patchfreq: u32,

    /// Version server URL override.
    #[arg(long, env = "CASCETTE_AGENT_VERSION_SERVER_URL")]
    pub version_server_url: Option<String>,

    /// Bind address (defaults to 127.0.0.1).
    #[arg(long, env = "CASCETTE_AGENT_BIND_ADDR", default_value = "127.0.0.1")]
    pub bind_addr: String,

    /// Maximum concurrent operations.
    #[arg(long, env = "CASCETTE_AGENT_MAX_CONCURRENT", default_value = "1")]
    pub max_concurrent_operations: usize,

    /// Request timeout in seconds.
    #[arg(long, env = "CASCETTE_AGENT_REQUEST_TIMEOUT", default_value = "30")]
    pub request_timeout_secs: u64,

    /// CDN host override (e.g., "cdn.arctium.tools" or comma-separated list).
    /// When set, these hosts are used instead of the hosts from Ribbit CDN queries.
    #[arg(long, env = "CASCETTE_AGENT_CDN_HOSTS")]
    pub cdn_hosts: Option<String>,

    /// CDN path override (e.g., "tpr/wow").
    /// When set, this path is used for all CDN requests instead of the path
    /// from Ribbit CDN queries.
    #[arg(long, env = "CASCETTE_AGENT_CDN_PATH")]
    pub cdn_path_override: Option<String>,

    // Windows-only service flags
    /// Install as Windows service.
    #[cfg(windows)]
    #[arg(long)]
    pub install_service: bool,

    /// Remove Windows service.
    #[cfg(windows)]
    #[arg(long)]
    pub remove_service: bool,

    /// Run as Windows service.
    #[cfg(windows)]
    #[arg(long)]
    pub service: bool,
}

impl AgentConfig {
    /// Parse configuration from command-line arguments.
    #[must_use]
    pub fn from_args() -> Self {
        Self::parse()
    }

    /// Get the configured or default port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// Get the database path, using a default if not specified.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| Self::default_data_dir().join(DEFAULT_DB_NAME))
    }

    /// Get the default data directory for agent state.
    #[must_use]
    pub fn default_data_dir() -> PathBuf {
        // Use platform-appropriate data directory
        #[cfg(target_os = "windows")]
        {
            std::env::var("PROGRAMDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"))
                .join("Battle.net")
                .join("Agent")
        }

        #[cfg(target_os = "macos")]
        {
            dirs_next::data_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("Battle.net")
                .join("Agent")
        }

        #[cfg(target_os = "linux")]
        {
            let base = match std::env::var("XDG_DATA_HOME") {
                Ok(path) => PathBuf::from(path),
                Err(_) => match std::env::var("HOME") {
                    Ok(home) => PathBuf::from(home).join(".local").join("share"),
                    Err(_) => PathBuf::from("/tmp"),
                },
            };
            base.join("cascette").join("agent")
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            PathBuf::from(".").join("cascette-agent-data")
        }
    }

    /// Get the log level filter string for tracing.
    #[must_use]
    pub fn log_filter(&self) -> String {
        self.loglevel.clone().unwrap_or_else(|| "info".to_string())
    }

    /// Build CDN endpoints from the `--cdn-hosts` override.
    ///
    /// Returns `None` if no override is set, meaning endpoints should come from
    /// Ribbit CDN queries. When set, returns endpoints with the specified hosts
    /// and the default or overridden CDN path.
    #[must_use]
    pub fn cdn_endpoint_overrides(&self) -> Option<Vec<cascette_protocol::CdnEndpoint>> {
        let hosts_str = self.cdn_hosts.as_ref()?;
        let cdn_path = self
            .cdn_path_override
            .as_deref()
            .unwrap_or("tpr/wow")
            .to_string();

        let endpoints = hosts_str
            .split(',')
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .map(|host| cascette_protocol::CdnEndpoint {
                host: host.to_string(),
                path: cdn_path.clone(),
                product_path: None,
                scheme: None,
                is_fallback: false,
                strict: false,
                max_hosts: None,
            })
            .collect::<Vec<_>>();

        if endpoints.is_empty() {
            None
        } else {
            Some(endpoints)
        }
    }

    /// Get the list of ports to try binding to, in order.
    #[must_use]
    pub fn port_candidates(&self) -> Vec<u16> {
        if let Some(port) = self.port {
            // Explicit port: only try that one
            vec![port]
        } else {
            // Default + fallbacks
            let mut ports = vec![DEFAULT_PORT];
            ports.extend_from_slice(FALLBACK_PORTS);
            ports
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        let config = AgentConfig {
            port: None,
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: None,
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        assert_eq!(config.port(), DEFAULT_PORT);
    }

    #[test]
    fn test_explicit_port() {
        let config = AgentConfig {
            port: Some(8080),
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: None,
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn test_port_candidates_default() {
        let config = AgentConfig {
            port: None,
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: None,
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        let candidates = config.port_candidates();
        assert_eq!(candidates, vec![1120, 6881, 6882, 6883]);
    }

    #[test]
    fn test_port_candidates_explicit() {
        let config = AgentConfig {
            port: Some(9999),
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: None,
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        let candidates = config.port_candidates();
        assert_eq!(candidates, vec![9999]);
    }

    #[test]
    fn test_log_filter_default() {
        let config = AgentConfig {
            port: None,
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: None,
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        assert_eq!(config.log_filter(), "info");
    }

    #[test]
    fn test_log_filter_override() {
        let config = AgentConfig {
            port: None,
            db_path: None,
            locale: "enUS".to_string(),
            show: false,
            allowcommands: false,
            skipupdate: false,
            loglevel: Some("debug".to_string()),
            session: None,
            patchfreq: 300,
            version_server_url: None,
            bind_addr: "127.0.0.1".to_string(),
            max_concurrent_operations: 1,
            request_timeout_secs: 30,
            cdn_hosts: None,
            cdn_path_override: None,
            #[cfg(windows)]
            install_service: false,
            #[cfg(windows)]
            remove_service: false,
            #[cfg(windows)]
            service: false,
        };
        assert_eq!(config.log_filter(), "debug");
    }
}
