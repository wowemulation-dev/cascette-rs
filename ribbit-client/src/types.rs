//! Type definitions for the Ribbit client

use std::fmt;

/// Supported regions for Ribbit endpoints
///
/// # Regional Availability
///
/// Some regions may have network restrictions:
/// - `CN` (China): Typically only accessible from within China
/// - Other regions (`US`, `EU`, `KR`, `TW`, `SG`): Generally accessible globally
///
/// The client will timeout after 10 seconds if a region is unreachable.
///
/// # Example
///
/// ```
/// use ribbit_client::Region;
///
/// let region = Region::US;
/// assert_eq!(region.hostname(), "us.version.battle.net");
/// assert_eq!(region.as_str(), "us");
///
/// // Parse from string
/// let parsed: Region = "eu".parse().unwrap();
/// assert_eq!(parsed, Region::EU);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// United States
    US,
    /// Europe
    EU,
    /// China
    CN,
    /// Korea
    KR,
    /// Taiwan
    TW,
    /// Singapore
    SG,
}

impl Region {
    /// Get the hostname for this region
    #[must_use]
    pub fn hostname(&self) -> &'static str {
        match self {
            Region::US => "us.version.battle.net",
            Region::EU => "eu.version.battle.net",
            Region::CN => "cn.version.battle.net",
            Region::KR => "kr.version.battle.net",
            Region::TW => "tw.version.battle.net",
            Region::SG => "sg.version.battle.net",
        }
    }

    /// Get the region code as a string
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::US => "us",
            Region::EU => "eu",
            Region::CN => "cn",
            Region::KR => "kr",
            Region::TW => "tw",
            Region::SG => "sg",
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Region {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "us" => Ok(Region::US),
            "eu" => Ok(Region::EU),
            "cn" => Ok(Region::CN),
            "kr" => Ok(Region::KR),
            "tw" => Ok(Region::TW),
            "sg" => Ok(Region::SG),
            _ => Err(crate::error::Error::InvalidRegion(s.to_string())),
        }
    }
}

/// Ribbit protocol version
///
/// # Example
///
/// ```
/// use ribbit_client::ProtocolVersion;
///
/// let v1 = ProtocolVersion::V1;
/// assert_eq!(v1.prefix(), "v1");
/// assert_eq!(v1.to_string(), "v1");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    /// Version 1 - MIME format with signatures and checksums
    V1,
    /// Version 2 - Raw PSV data without MIME wrapper
    V2,
}

impl ProtocolVersion {
    /// Get the version prefix for commands
    #[must_use]
    pub fn prefix(&self) -> &'static str {
        match self {
            ProtocolVersion::V1 => "v1",
            ProtocolVersion::V2 => "v2",
        }
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.prefix())
    }
}

/// Common Ribbit endpoints
///
/// # Example
///
/// ```
/// use ribbit_client::Endpoint;
///
/// // Product versions endpoint
/// let versions = Endpoint::ProductVersions("wow".to_string());
/// assert_eq!(versions.as_path(), "products/wow/versions");
///
/// // Certificate endpoint
/// let cert = Endpoint::Cert("abc123".to_string());
/// assert_eq!(cert.as_path(), "certs/abc123");
///
/// // Custom endpoint
/// let custom = Endpoint::Custom("custom/path".to_string());
/// assert_eq!(custom.as_path(), "custom/path");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// Summary of all products
    Summary,
    /// Product versions
    ProductVersions(String),
    /// Product CDNs
    ProductCdns(String),
    /// Product background download info
    ProductBgdl(String),
    /// Certificate by hash
    Cert(String),
    /// OCSP response by hash
    Ocsp(String),
    /// Custom endpoint
    Custom(String),
}

impl Endpoint {
    /// Convert the endpoint to its path representation
    #[must_use]
    pub fn as_path(&self) -> String {
        match self {
            Endpoint::Summary => "summary".to_string(),
            Endpoint::ProductVersions(product) => format!("products/{product}/versions"),
            Endpoint::ProductCdns(product) => format!("products/{product}/cdns"),
            Endpoint::ProductBgdl(product) => format!("products/{product}/bgdl"),
            Endpoint::Cert(hash) => format!("certs/{hash}"),
            Endpoint::Ocsp(hash) => format!("ocsp/{hash}"),
            Endpoint::Custom(path) => path.clone(),
        }
    }
}

/// Ribbit TCP port
pub const RIBBIT_PORT: u16 = 1119;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_hostname() {
        assert_eq!(Region::US.hostname(), "us.version.battle.net");
        assert_eq!(Region::EU.hostname(), "eu.version.battle.net");
        assert_eq!(Region::CN.hostname(), "cn.version.battle.net");
        assert_eq!(Region::KR.hostname(), "kr.version.battle.net");
        assert_eq!(Region::TW.hostname(), "tw.version.battle.net");
        assert_eq!(Region::SG.hostname(), "sg.version.battle.net");
    }

    #[test]
    fn test_region_as_str() {
        assert_eq!(Region::US.as_str(), "us");
        assert_eq!(Region::EU.as_str(), "eu");
        assert_eq!(Region::CN.as_str(), "cn");
        assert_eq!(Region::KR.as_str(), "kr");
        assert_eq!(Region::TW.as_str(), "tw");
        assert_eq!(Region::SG.as_str(), "sg");
    }

    #[test]
    fn test_region_display() {
        assert_eq!(Region::US.to_string(), "us");
        assert_eq!(Region::EU.to_string(), "eu");
    }

    #[test]
    fn test_region_from_str() {
        assert_eq!("us".parse::<Region>().unwrap(), Region::US);
        assert_eq!("US".parse::<Region>().unwrap(), Region::US);
        assert_eq!("eu".parse::<Region>().unwrap(), Region::EU);
        assert_eq!("EU".parse::<Region>().unwrap(), Region::EU);

        assert!("invalid".parse::<Region>().is_err());
        assert!("".parse::<Region>().is_err());
    }

    #[test]
    fn test_protocol_version_prefix() {
        assert_eq!(ProtocolVersion::V1.prefix(), "v1");
        assert_eq!(ProtocolVersion::V2.prefix(), "v2");
    }

    #[test]
    fn test_protocol_version_display() {
        assert_eq!(ProtocolVersion::V1.to_string(), "v1");
        assert_eq!(ProtocolVersion::V2.to_string(), "v2");
    }

    #[test]
    fn test_endpoint_as_path() {
        assert_eq!(Endpoint::Summary.as_path(), "summary");
        assert_eq!(
            Endpoint::ProductVersions("wow".to_string()).as_path(),
            "products/wow/versions"
        );
        assert_eq!(
            Endpoint::ProductCdns("wow_classic".to_string()).as_path(),
            "products/wow_classic/cdns"
        );
        assert_eq!(
            Endpoint::ProductBgdl("wow_beta".to_string()).as_path(),
            "products/wow_beta/bgdl"
        );
        assert_eq!(
            Endpoint::Cert("abc123".to_string()).as_path(),
            "certs/abc123"
        );
        assert_eq!(
            Endpoint::Ocsp("def456".to_string()).as_path(),
            "ocsp/def456"
        );
        assert_eq!(
            Endpoint::Custom("custom/path".to_string()).as_path(),
            "custom/path"
        );
    }

    #[test]
    fn test_ribbit_port() {
        assert_eq!(RIBBIT_PORT, 1119);
    }
}
