//! CDN endpoint selection and community mirror configuration.
//!
//! Official CDN hosts come from the CDN config response. Community mirrors
//! serve as fallbacks for historical builds that may no longer be on official CDN.

use cascette_protocol::CdnEndpoint;

/// Community CDN mirrors that host historical CASC content.
pub const COMMUNITY_MIRRORS: &[&str] =
    &["cdn.arctium.tools", "casc.wago.tools", "archive.wow.tools"];

/// Mirror selection configuration.
#[derive(Debug, Clone)]
pub struct MirrorConfig {
    /// Official CDN endpoints from Ribbit/TACT.
    pub official: Vec<CdnEndpoint>,

    /// Whether to include community mirrors as fallback.
    pub use_community_mirrors: bool,

    /// Whether this is a historical build (community mirrors get priority).
    pub is_historic: bool,
}

impl MirrorConfig {
    /// Build the ordered endpoint list.
    ///
    /// For current builds: official endpoints first, then community mirrors.
    /// For historical builds: community mirrors first, then official endpoints.
    #[must_use]
    pub fn build_endpoint_list(&self, cdn_path: &str) -> Vec<CdnEndpoint> {
        let community: Vec<CdnEndpoint> = if self.use_community_mirrors {
            COMMUNITY_MIRRORS
                .iter()
                .map(|host| CdnEndpoint {
                    host: (*host).to_string(),
                    path: cdn_path.to_string(),
                    product_path: None,
                    scheme: Some("https".to_string()),
                    is_fallback: true,
                    strict: false,
                    max_hosts: None,
                })
                .collect()
        } else {
            Vec::new()
        };

        if self.is_historic {
            let mut endpoints = community;
            endpoints.extend(self.official.clone());
            endpoints
        } else {
            let mut endpoints = self.official.clone();
            endpoints.extend(community);
            endpoints
        }
    }
}

/// Backoff weight multipliers from agent.exe reverse engineering.
///
/// These control how much a host's weight increases after specific HTTP errors.
pub mod backoff {
    /// 401 Unauthorized or 416 Range Not Satisfiable: moderate penalty.
    pub const AUTH_RANGE_MULTIPLIER: f64 = 2.5;

    /// 500, 502, 503, 504 server errors: heavy penalty.
    pub const SERVER_ERROR_MULTIPLIER: f64 = 5.0;
}
