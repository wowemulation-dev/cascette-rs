//! BPSV (Blizzard Pipe-Separated Values) response generation.
//!
//! BPSV is a text format used by Blizzard's NGDP system for metadata exchange.
//! Format: Header line with column definitions, followed by data rows, optional seqn footer.

use crate::config::CdnConfig;
use crate::database::BuildRecord;
use std::fmt;

/// Supported BPSV response types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpsvResponseType {
    /// Version information (`BuildConfig`, `CDNConfig`, `BuildId`, `VersionsName`, `ProductConfig`)
    Versions,
    /// CDN configuration (Name, Path, Hosts, `Servers`, `ConfigPath`)
    Cdns,
    /// Background download information (similar to versions)
    Bgdl,
    /// Product summary (Product, Seqn) - TCP v1 only
    Summary,
}

/// BPSV response builder.
pub struct BpsvResponse {
    /// Response type determines schema
    response_type: BpsvResponseType,
    /// Lines of BPSV content
    lines: Vec<String>,
}

impl BpsvResponse {
    /// Create versions response for a build across all regions.
    #[must_use]
    pub fn versions(build: &BuildRecord, seqn: u64) -> Self {
        let mut lines = Vec::new();

        // Header line - must match Blizzard's exact format including KeyRing field
        lines.push(
            "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!HEX:16"
                .to_string(),
        );

        // Data rows for each region (same build across all regions)
        // Real Blizzard API uses 7 regions: us, eu, cn, kr, tw, sg, xx
        let product_config = build.product_config.as_deref().unwrap_or("");
        let keyring = build.keyring.as_deref().unwrap_or("");
        for region in ["us", "eu", "cn", "kr", "tw", "sg", "xx"] {
            lines.push(format!(
                "{region}|{}|{}|{keyring}|{}|{}|{product_config}",
                build.build_config, build.cdn_config, build.build, build.version
            ));
        }

        // Sequence number footer
        lines.push(format!("## seqn = {seqn}"));

        Self {
            response_type: BpsvResponseType::Versions,
            lines,
        }
    }

    /// Create CDN configuration response for all regions.
    #[must_use]
    pub fn cdns(cdn_config: &CdnConfig, seqn: u64) -> Self {
        let mut lines = Vec::new();

        // Header line
        lines.push(
            "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0"
                .to_string(),
        );

        // Data rows for each region (same CDN config across all regions)
        for region in ["us", "eu", "kr", "tw", "cn"] {
            lines.push(format!(
                "{region}|{}|{}|{}|{}",
                cdn_config.path, cdn_config.hosts, cdn_config.servers, cdn_config.config_path
            ));
        }

        // Sequence number footer
        lines.push(format!("## seqn = {seqn}"));

        Self {
            response_type: BpsvResponseType::Cdns,
            lines,
        }
    }

    /// Create background download response (same format as versions).
    #[must_use]
    pub fn bgdl(build: &BuildRecord, seqn: u64) -> Self {
        // BGDL uses same format as versions
        let mut response = Self::versions(build, seqn);
        response.response_type = BpsvResponseType::Bgdl;
        response
    }

    /// Create summary response listing all products.
    #[must_use]
    pub fn summary(products: &[&str], seqn: u64) -> Self {
        let mut lines = vec!["Product!STRING:0|Seqn!DEC:4".to_string()];

        // Data rows for each product
        for product in products {
            lines.push(format!("{product}|{seqn}"));
        }

        // Sequence number footer
        lines.push(format!("## seqn = {seqn}"));

        Self {
            response_type: BpsvResponseType::Summary,
            lines,
        }
    }

    /// Get the response type.
    #[must_use]
    pub const fn response_type(&self) -> BpsvResponseType {
        self.response_type
    }

    /// Get BPSV content as bytes.
    #[must_use]
    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl fmt::Display for BpsvResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_build() -> BuildRecord {
        BuildRecord {
            id: 1,
            product: "wow_classic".to_string(),
            version: "1.13.2.32600".to_string(),
            build: "32600".to_string(),
            build_config: "596c212114208f0f849c6b6e596e6680".to_string(),
            cdn_config: "bf4672a701f0795b21ad63bf6b98ae0a".to_string(),
            keyring: None,
            product_config: None,
            build_time: "2019-11-21T18:33:35+00:00".to_string(),
            encoding_ekey: "aaaabbbbccccddddeeeeffffaaaaffff".to_string(),
            root_ekey: "bbbbccccddddeeeeffffaaaabbbbcccc".to_string(),
            install_ekey: "ccccddddeeeeffffaaaabbbbccccdddd".to_string(),
            download_ekey: "ddddeeeeffffaaaabbbbccccddddeeee".to_string(),
            cdn_path: None,
        }
    }

    #[test]
    fn test_versions_response() {
        let build = create_test_build();
        let response = BpsvResponse::versions(&build, 1_730_534_400);

        let text = response.to_string();
        assert!(text.contains("Region!STRING:0|BuildConfig!HEX:16"));
        assert!(text.contains("KeyRing!HEX:16"));
        assert!(text.contains("us|596c212114208f0f849c6b6e596e6680"));
        assert!(text.contains("## seqn = 1730534400"));

        // Should have 7 regions + header + footer = 9 lines
        assert_eq!(text.lines().count(), 9);
    }

    #[test]
    fn test_cdns_response() {
        let cdn_config = CdnConfig {
            hosts: "cdn.arctium.tools".to_string(),
            path: "tpr/wow".to_string(),
            servers: "https://cdn.arctium.tools/?fallbackProtocol=http".to_string(),
            config_path: "tpr/wow".to_string(),
        };

        let response = BpsvResponse::cdns(&cdn_config, 1_730_534_400);

        let text = response.to_string();
        assert!(text.contains("Name!STRING:0|Path!STRING:0|Hosts!STRING:0"));
        assert!(text.contains("us|tpr/wow|cdn.arctium.tools"));
        assert!(text.contains("## seqn = 1730534400"));
    }

    #[test]
    fn test_summary_response() {
        let products = vec!["wow", "wow_classic", "wow_classic_era"];
        let response = BpsvResponse::summary(&products, 1_730_534_400);

        let text = response.to_string();
        assert!(text.contains("Product!STRING:0|Seqn!DEC:4"));
        assert!(text.contains("wow|1730534400"));
        assert!(text.contains("wow_classic|1730534400"));
        assert!(text.contains("## seqn = 1730534400"));
    }

    #[test]
    fn test_bgdl_response() {
        let build = create_test_build();
        let response = BpsvResponse::bgdl(&build, 1_730_534_400);

        // BGDL uses same format as versions
        assert_eq!(response.response_type(), BpsvResponseType::Bgdl);
        let text = response.to_string();
        assert!(text.contains("Region!STRING:0|BuildConfig!HEX:16"));
    }

    #[test]
    fn test_response_with_product_config() {
        let mut build = create_test_build();
        build.product_config = Some("abcdef1234567890abcdef1234567890".to_string());

        let response = BpsvResponse::versions(&build, 1_730_534_400);
        let text = response.to_string();
        assert!(text.contains("abcdef1234567890abcdef1234567890"));
    }
}
