//! Strongly-typed response definitions for all Ribbit endpoints
//!
//! This module provides type-safe representations of responses from various
//! Ribbit endpoints, automatically parsing BPSV data into structured Rust types.

use crate::{Error, Response, error::Result};
use ngdp_bpsv::BpsvDocument;

/// Trait for typed responses that can be parsed from BPSV documents
pub trait TypedResponse: Sized {
    /// Parse the response from a BPSV document
    ///
    /// # Errors
    /// Returns an error if parsing the BPSV document fails.
    fn from_bpsv(doc: &BpsvDocument) -> Result<Self>;

    /// Parse from a raw response
    ///
    /// # Errors
    /// Returns an error if the response has no data or parsing fails.
    fn from_response(response: &Response) -> Result<Self> {
        match &response.data {
            Some(data) => {
                // Parse directly - BPSV parser now correctly handles HEX:N as N bytes
                let doc = BpsvDocument::parse(data)
                    .map_err(|e| Error::ParseError(format!("BPSV parse error: {e}")))?;
                Self::from_bpsv(&doc)
            }
            None => Err(Error::ParseError("No data in response".to_string())),
        }
    }
}

/// Product versions response containing build information for all regions
#[derive(Debug, Clone, PartialEq)]
pub struct ProductVersionsResponse {
    /// Sequence number from the BPSV document
    pub sequence_number: Option<u32>,
    /// Version entries for each region
    pub entries: Vec<VersionEntry>,
}

/// Single version entry for a specific region
#[derive(Debug, Clone, PartialEq)]
pub struct VersionEntry {
    /// Region code (us, eu, cn, kr, tw, sg)
    pub region: String,
    /// Build configuration hash (16 bytes hex)
    pub build_config: String,
    /// CDN configuration hash (16 bytes hex)
    pub cdn_config: String,
    /// Optional keyring hash (16 bytes hex)
    pub key_ring: Option<String>,
    /// Build ID number
    pub build_id: u32,
    /// Human-readable version name
    pub versions_name: String,
    /// Product configuration hash (16 bytes hex)
    pub product_config: String,
}

/// CDN server information response
#[derive(Debug, Clone, PartialEq)]
pub struct ProductCdnsResponse {
    /// Sequence number from the BPSV document
    pub sequence_number: Option<u32>,
    /// CDN entries for each region/configuration
    pub entries: Vec<CdnEntry>,
}

/// CDN configuration entry
#[derive(Debug, Clone, PartialEq)]
pub struct CdnEntry {
    /// CDN name/identifier
    pub name: String,
    /// CDN path prefix
    pub path: String,
    /// List of CDN hostnames
    pub hosts: Vec<String>,
    /// List of CDN server URLs
    pub servers: Vec<String>,
    /// Configuration path on the CDN
    pub config_path: String,
}

/// Background download configuration response
#[derive(Debug, Clone, PartialEq)]
pub struct ProductBgdlResponse {
    /// Sequence number from the BPSV document
    pub sequence_number: Option<u32>,
    /// Background download entries per region
    pub entries: Vec<BgdlEntry>,
}

/// Background download configuration entry
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

/// Summary of all available products
#[derive(Debug, Clone, PartialEq)]
pub struct SummaryResponse {
    /// Sequence number from the BPSV document
    pub sequence_number: Option<u32>,
    /// List of available products
    pub products: Vec<ProductSummary>,
}

/// Summary information for a single product
#[derive(Debug, Clone, PartialEq)]
pub struct ProductSummary {
    /// Product code (e.g., "wow", "d3", "hero")
    pub product: String,
    /// Product-specific sequence number
    pub seqn: u32,
    /// Optional flags
    pub flags: Option<String>,
}

/// Helper struct for accessing BPSV row data by field name
struct FieldAccessor<'a> {
    row: &'a ngdp_bpsv::document::BpsvRow<'a>,
    schema: &'a ngdp_bpsv::BpsvSchema,
}

impl<'a> FieldAccessor<'a> {
    fn new(row: &'a ngdp_bpsv::document::BpsvRow, schema: &'a ngdp_bpsv::BpsvSchema) -> Self {
        Self { row, schema }
    }

    fn get_string(&self, field: &str) -> Result<String> {
        self.row
            .get_raw_by_name(field, self.schema)
            .map(std::string::ToString::to_string)
            .ok_or_else(|| Error::ParseError(format!("Missing field: {field}")))
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
        value
            .parse()
            .map_err(|_| Error::ParseError(format!("Invalid u32 for {field}: {value}")))
    }

    fn get_string_list(&self, field: &str, separator: char) -> Result<Vec<String>> {
        let value = self.get_string(field)?;
        if value.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(value
                .split(separator)
                .map(|s| s.trim().to_string())
                .collect())
        }
    }
}

// Implementations for each response type

impl TypedResponse for ProductVersionsResponse {
    fn from_bpsv(doc: &BpsvDocument) -> Result<Self> {
        let mut entries = Vec::new();
        let schema = doc.schema();

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

        Ok(Self {
            sequence_number: doc.sequence_number(),
            entries,
        })
    }
}

impl TypedResponse for ProductCdnsResponse {
    fn from_bpsv(doc: &BpsvDocument) -> Result<Self> {
        let mut entries = Vec::new();
        let schema = doc.schema();

        for row in doc.rows() {
            let accessor = FieldAccessor::new(row, schema);

            entries.push(CdnEntry {
                name: accessor.get_string("Name")?,
                path: accessor.get_string("Path")?,
                hosts: accessor.get_string_list("Hosts", ' ')?,
                servers: accessor
                    .get_string_optional("Servers")
                    .map(|s| {
                        s.split_whitespace()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                config_path: accessor.get_string("ConfigPath")?,
            });
        }

        Ok(Self {
            sequence_number: doc.sequence_number(),
            entries,
        })
    }
}

impl TypedResponse for ProductBgdlResponse {
    fn from_bpsv(doc: &BpsvDocument) -> Result<Self> {
        let mut entries = Vec::new();
        let schema = doc.schema();

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

        Ok(Self {
            sequence_number: doc.sequence_number(),
            entries,
        })
    }
}

impl TypedResponse for SummaryResponse {
    fn from_bpsv(doc: &BpsvDocument) -> Result<Self> {
        let mut products = Vec::new();
        let schema = doc.schema();

        for row in doc.rows() {
            let accessor = FieldAccessor::new(row, schema);

            products.push(ProductSummary {
                product: accessor.get_string("Product")?,
                seqn: accessor.get_u32("Seqn")?,
                flags: accessor.get_string_optional("Flags"),
            });
        }

        Ok(Self {
            sequence_number: doc.sequence_number(),
            products,
        })
    }
}

/// Convenience methods for response types
impl ProductVersionsResponse {
    /// Get version entry for a specific region
    #[must_use]
    pub fn get_region(&self, region: &str) -> Option<&VersionEntry> {
        self.entries.iter().find(|e| e.region == region)
    }

    /// Get all unique build IDs
    #[must_use]
    pub fn build_ids(&self) -> Vec<u32> {
        let mut ids: Vec<_> = self.entries.iter().map(|e| e.build_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

impl ProductCdnsResponse {
    /// Get CDN entry by name
    #[must_use]
    pub fn get_cdn(&self, name: &str) -> Option<&CdnEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Get all unique CDN hosts
    #[must_use]
    pub fn all_hosts(&self) -> Vec<String> {
        let mut hosts = Vec::new();
        for entry in &self.entries {
            hosts.extend(entry.hosts.clone());
        }
        hosts.sort();
        hosts.dedup();
        hosts
    }
}

impl SummaryResponse {
    /// Get summary for a specific product
    #[must_use]
    pub fn get_product(&self, product: &str) -> Option<&ProductSummary> {
        self.products.iter().find(|p| p.product == product)
    }

    /// Get all product codes
    #[must_use]
    pub fn product_codes(&self) -> Vec<&str> {
        self.products.iter().map(|p| p.product.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_product_versions() {
        // Use HEX:16 which expects 32 hex characters (16 bytes)
        let bpsv_data = concat!(
            "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!HEX:16\n",
            "## seqn = 12345\n",
            "us|abcdef1234567890abcdef1234567890|fedcba0987654321fedcba0987654321|123456|10.2.5.53040|1234567890abcdef1234567890abcdef\n",
            "eu|abcdef1234567890abcdef1234567890|fedcba0987654321fedcba0987654321|123456|10.2.5.53040|1234567890abcdef1234567890abcdef\n"
        );

        let doc = BpsvDocument::parse(bpsv_data).unwrap();
        let response = ProductVersionsResponse::from_bpsv(&doc).unwrap();

        assert_eq!(response.sequence_number, Some(12345));
        assert_eq!(response.entries.len(), 2);
        assert_eq!(response.entries[0].region, "us");
        assert_eq!(response.entries[0].build_id, 123_456);
        assert_eq!(response.entries[0].versions_name, "10.2.5.53040");
    }

    #[test]
    fn test_parse_product_cdns() {
        let bpsv_data = concat!(
            "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|ConfigPath!STRING:0\n",
            "## seqn = 54321\n",
            "us|tpr/wow|level3.blizzard.com edgecast.blizzard.com|tpr/configs/data\n",
            "eu|tpr/wow|level3.blizzard.com|tpr/configs/data\n"
        );

        let doc = BpsvDocument::parse(bpsv_data).unwrap();
        let response = ProductCdnsResponse::from_bpsv(&doc).unwrap();

        assert_eq!(response.sequence_number, Some(54321));
        assert_eq!(response.entries.len(), 2);
        assert_eq!(response.entries[0].name, "us");
        assert_eq!(response.entries[0].hosts.len(), 2);
        assert_eq!(response.entries[0].hosts[0], "level3.blizzard.com");
    }

    #[test]
    fn test_parse_summary() {
        let bpsv_data = concat!(
            "Product!STRING:0|Seqn!DEC:4|Flags!STRING:0\n",
            "## seqn = 99999\n",
            "wow|12345|installed\n",
            "d3|54321|\n",
            "hero|11111|beta\n"
        );

        let doc = BpsvDocument::parse(bpsv_data).unwrap();
        let response = SummaryResponse::from_bpsv(&doc).unwrap();

        assert_eq!(response.sequence_number, Some(99999));
        assert_eq!(response.products.len(), 3);
        assert_eq!(response.products[0].product, "wow");
        assert_eq!(response.products[0].seqn, 12345);
        assert_eq!(response.products[0].flags, Some("installed".to_string()));
        assert_eq!(response.products[1].flags, None);
    }

    #[test]
    fn test_from_response_with_hex_adjustment() {
        // Test that from_response properly adjusts HEX field lengths
        let data = concat!(
            "Region!STRING:0|BuildConfig!HEX:16\n",
            "## seqn = 12345\n",
            "us|e359107662e72559b4e1ab721b157cb0\n"
        );

        let response = Response {
            raw: data.as_bytes().to_vec(),
            data: Some(data.to_string()),
            mime_parts: None,
        };

        // This would fail without HEX adjustment because the data has 32 chars but header says HEX:16
        // With adjustment, it should work
        let result = ProductVersionsResponse::from_response(&response);

        // The test expects this to fail because ProductVersionsResponse needs more fields
        // But it should fail with a missing field error, not a HEX validation error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing field") || err_msg.contains("Parse error"));
        assert!(!err_msg.contains("Invalid value for field 'BuildConfig'"));
    }
}
