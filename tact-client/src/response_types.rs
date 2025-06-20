//! Response types for TACT HTTP endpoints using ngdp-bpsv

use crate::{Error, Result};
use ngdp_bpsv::{BpsvDocument, BpsvSchema};

/// Version configuration entry (from /versions endpoint)
#[derive(Debug, Clone, PartialEq)]
pub struct VersionEntry {
    /// Region code
    pub region: String,
    /// Build configuration hash
    pub build_config: String,
    /// CDN configuration hash
    pub cdn_config: String,
    /// Optional key ring hash
    pub key_ring: Option<String>,
    /// Build ID number
    pub build_id: u32,
    /// Human-readable version name
    pub versions_name: String,
    /// Product configuration hash
    pub product_config: String,
}

/// CDN configuration entry (from /cdns endpoint)
#[derive(Debug, Clone, PartialEq)]
pub struct CdnEntry {
    /// CDN name/identifier (e.g., "us", "eu")
    pub name: String,
    /// CDN path prefix (e.g., "tpr/wow")
    pub path: String,
    /// List of CDN hostnames (space-separated in manifest)
    pub hosts: Vec<String>,
    /// List of CDN server URLs (space-separated in manifest)
    pub servers: Vec<String>,
    /// Configuration path on the CDN
    pub config_path: String,
}

/// Background download configuration entry (from /bgdl endpoint)
#[derive(Debug, Clone, PartialEq)]
pub struct BgdlEntry {
    /// Region code
    pub region: String,
    /// Build configuration hash
    pub build_config: String,
    /// CDN configuration hash
    pub cdn_config: String,
    /// Optional install background download config
    pub install_bgdl_config: Option<String>,
    /// Optional game background download config
    pub game_bgdl_config: Option<String>,
}

/// Helper struct for accessing BPSV row data by field name
struct FieldAccessor<'a> {
    row: &'a ngdp_bpsv::document::BpsvRow,
    schema: &'a BpsvSchema,
}

impl<'a> FieldAccessor<'a> {
    fn new(row: &'a ngdp_bpsv::document::BpsvRow, schema: &'a BpsvSchema) -> Self {
        Self { row, schema }
    }

    fn get_string(&self, field: &str) -> Result<String> {
        self.row
            .get_raw_by_name(field, self.schema)
            .map(std::string::ToString::to_string)
            .ok_or_else(|| Error::InvalidManifest {
                line: 0,
                reason: format!("Missing field: {field}"),
            })
    }

    fn get_string_optional(&self, field: &str) -> Option<String> {
        self.row.get_raw_by_name(field, self.schema).and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
    }

    fn get_u32(&self, field: &str) -> Result<u32> {
        let value = self.get_string(field)?;
        value.parse().map_err(|_| Error::InvalidManifest {
            line: 0,
            reason: format!("Invalid u32 for {field}: {value}"),
        })
    }

    fn get_string_list(&self, field: &str, separator: char) -> Result<Vec<String>> {
        let value = self.get_string(field)?;
        if value.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(value
                .split(separator)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect())
        }
    }
}

/// Parse versions manifest into typed entries
pub fn parse_versions(content: &str) -> Result<Vec<VersionEntry>> {
    let doc = BpsvDocument::parse(content)?;
    let schema = doc.schema();
    let mut entries = Vec::new();

    for row in doc.rows() {
        let accessor = FieldAccessor::new(row, schema);

        entries.push(VersionEntry {
            region: accessor.get_string("Region")?,
            build_config: accessor.get_string("BuildConfig")?,
            cdn_config: accessor.get_string("CDNConfig")?,
            key_ring: accessor.get_string_optional("KeyRing"),
            build_id: accessor.get_u32("BuildId")?,
            versions_name: accessor.get_string("VersionsName")?,
            product_config: accessor.get_string("ProductConfig")?,
        });
    }

    Ok(entries)
}

/// Parse CDN manifest into typed entries
pub fn parse_cdns(content: &str) -> Result<Vec<CdnEntry>> {
    let doc = BpsvDocument::parse(content)?;
    let schema = doc.schema();
    let mut entries = Vec::new();

    for row in doc.rows() {
        let accessor = FieldAccessor::new(row, schema);

        // Both hosts and servers are space-separated lists
        let hosts = accessor.get_string_list("Hosts", ' ')?;
        let servers = accessor
            .get_string_optional("Servers")
            .map(|s| {
                s.split_whitespace()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        entries.push(CdnEntry {
            name: accessor.get_string("Name")?,
            path: accessor.get_string("Path")?,
            hosts,
            servers,
            config_path: accessor.get_string("ConfigPath")?,
        });
    }

    Ok(entries)
}

/// Parse BGDL manifest into typed entries
pub fn parse_bgdl(content: &str) -> Result<Vec<BgdlEntry>> {
    let doc = BpsvDocument::parse(content)?;
    let schema = doc.schema();
    let mut entries = Vec::new();

    for row in doc.rows() {
        let accessor = FieldAccessor::new(row, schema);

        entries.push(BgdlEntry {
            region: accessor.get_string("Region")?,
            build_config: accessor.get_string("BuildConfig")?,
            cdn_config: accessor.get_string("CDNConfig")?,
            install_bgdl_config: accessor.get_string_optional("InstallBGDLConfig"),
            game_bgdl_config: accessor.get_string_optional("GameBGDLConfig"),
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cdns_with_servers() {
        let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4|tpr/configs/data
eu|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://eu.cdn.blizzard.com/?maxhosts=4 https://blzddist1-a.akamaihd.net/?fallback=1&maxhosts=4|tpr/configs/data"#;

        let entries = parse_cdns(content).unwrap();
        assert_eq!(entries.len(), 2);

        let us_cdn = &entries[0];
        assert_eq!(us_cdn.name, "us");
        assert_eq!(us_cdn.path, "tpr/wow");
        assert_eq!(us_cdn.hosts.len(), 2);
        assert_eq!(us_cdn.hosts[0], "blzddist1-a.akamaihd.net");
        assert_eq!(us_cdn.hosts[1], "level3.blizzard.com");
        assert_eq!(us_cdn.servers.len(), 2);
        assert_eq!(
            us_cdn.servers[0],
            "http://blzddist1-a.akamaihd.net/?maxhosts=4"
        );
        assert_eq!(us_cdn.servers[1], "http://level3.blizzard.com/?maxhosts=4");
    }

    #[test]
    fn test_parse_cdns_without_servers() {
        let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|host1.com host2.com||tpr/configs/data"#;

        let entries = parse_cdns(content).unwrap();
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.hosts, vec!["host1.com", "host2.com"]);
        assert_eq!(entry.servers, Vec::<String>::new());
    }

    #[test]
    fn test_parse_versions() {
        let content = r#"Region!STRING:0|BuildConfig!STRING:0|CDNConfig!STRING:0|KeyRing!STRING:0|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!STRING:0
us|abcd1234|efgh5678||12345|1.0.0.12345|ijkl9012
eu|abcd1234|efgh5678|mnop3456|12345|1.0.0.12345|ijkl9012"#;

        let entries = parse_versions(content).unwrap();
        assert_eq!(entries.len(), 2);

        let us_version = &entries[0];
        assert_eq!(us_version.region, "us");
        assert_eq!(us_version.build_id, 12345);
        assert!(us_version.key_ring.is_none());

        let eu_version = &entries[1];
        assert_eq!(eu_version.key_ring, Some("mnop3456".to_string()));
    }

    #[test]
    fn test_parse_bgdl() {
        let content = r#"Region!STRING:0|BuildConfig!STRING:0|CDNConfig!STRING:0|InstallBGDLConfig!STRING:0|GameBGDLConfig!STRING:0
us|abcd1234|efgh5678|install123|game456
eu|abcd1234|efgh5678||game789"#;

        let entries = parse_bgdl(content).unwrap();
        assert_eq!(entries.len(), 2);

        let us_bgdl = &entries[0];
        assert_eq!(us_bgdl.install_bgdl_config, Some("install123".to_string()));
        assert_eq!(us_bgdl.game_bgdl_config, Some("game456".to_string()));

        let eu_bgdl = &entries[1];
        assert!(eu_bgdl.install_bgdl_config.is_none());
        assert_eq!(eu_bgdl.game_bgdl_config, Some("game789".to_string()));
    }

    #[test]
    fn test_ribbit_vs_http_compatibility() {
        // This test verifies that both Ribbit and HTTP endpoints return
        // CDN data in the same format with hosts and servers fields
        let ribbit_format = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|level3.blizzard.com us.cdn.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4|tpr/configs/data"#;

        let entries = parse_cdns(ribbit_format).unwrap();
        let entry = &entries[0];

        // Verify that both hosts and servers are parsed as lists
        assert_eq!(entry.hosts.len(), 2);
        assert_eq!(entry.servers.len(), 2);

        // Verify the data matches what we see in reference implementations
        assert_eq!(entry.hosts[0], "level3.blizzard.com");
        assert_eq!(entry.hosts[1], "us.cdn.blizzard.com");
        assert_eq!(entry.servers[0], "http://level3.blizzard.com/?maxhosts=4");
        assert_eq!(entry.servers[1], "http://us.cdn.blizzard.com/?maxhosts=4");
    }
}
