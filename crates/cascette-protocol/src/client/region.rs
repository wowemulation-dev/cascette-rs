//! Region definitions for Blizzard game servers.
//!
//! Blizzard uses different hostnames for different protocol versions:
//! - TACT v2 (HTTPS, port 443): `{region}.version.battle.net`
//! - TACT v1 (HTTP, port 1119): `{region}.patch.battle.net:1119`
//! - Ribbit TCP (port 1119): `{region}.version.battle.net:1119`
//!
//! China uses `.com.cn` domains operated separately from the global
//! `.battle.net` infrastructure.

/// Game server region.
///
/// Each region maps to specific TACT and Ribbit endpoints.
/// China uses `.com.cn` domains; all other regions use `.battle.net`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// United States
    US,
    /// Europe
    EU,
    /// Korea
    KR,
    /// Taiwan
    TW,
    /// China (uses `.com.cn` domains)
    CN,
    /// Singapore
    SG,
}

impl Region {
    /// TACT v2 HTTPS URL for this region (port 443).
    ///
    /// Used by [`TactClient::for_region()`](super::TactClient::for_region).
    pub fn tact_https_url(&self) -> &'static str {
        match self {
            Self::US => "https://us.version.battle.net",
            Self::EU => "https://eu.version.battle.net",
            Self::KR => "https://kr.version.battle.net",
            Self::TW => "https://tw.version.battle.net",
            Self::CN => "https://cn.version.battlenet.com.cn",
            Self::SG => "https://sg.version.battle.net",
        }
    }

    /// TACT v1 HTTP URL for this region (port 1119).
    pub fn tact_http_url(&self) -> &'static str {
        match self {
            Self::US => "http://us.patch.battle.net:1119",
            Self::EU => "http://eu.patch.battle.net:1119",
            Self::KR => "http://kr.patch.battle.net:1119",
            Self::TW => "http://tw.patch.battle.net:1119",
            Self::CN => "http://cn.patch.battlenet.com.cn:1119",
            Self::SG => "http://sg.patch.battle.net:1119",
        }
    }

    /// Ribbit TCP address (`host:port`) for this region (port 1119).
    ///
    /// Used by [`RibbitClient::for_region()`](super::RibbitClient::for_region).
    pub fn ribbit_address(&self) -> &'static str {
        match self {
            Self::US => "us.version.battle.net:1119",
            Self::EU => "eu.version.battle.net:1119",
            Self::KR => "kr.version.battle.net:1119",
            Self::TW => "tw.version.battle.net:1119",
            Self::CN => "cn.version.battlenet.com.cn:1119",
            Self::SG => "sg.version.battle.net:1119",
        }
    }
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::US => write!(f, "us"),
            Self::EU => write!(f, "eu"),
            Self::KR => write!(f, "kr"),
            Self::TW => write!(f, "tw"),
            Self::CN => write!(f, "cn"),
            Self::SG => write!(f, "sg"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_us_region_urls() {
        assert_eq!(
            Region::US.tact_https_url(),
            "https://us.version.battle.net"
        );
        assert_eq!(
            Region::US.tact_http_url(),
            "http://us.patch.battle.net:1119"
        );
        assert_eq!(Region::US.ribbit_address(), "us.version.battle.net:1119");
    }

    #[test]
    fn test_eu_region_urls() {
        assert_eq!(
            Region::EU.tact_https_url(),
            "https://eu.version.battle.net"
        );
        assert_eq!(
            Region::EU.tact_http_url(),
            "http://eu.patch.battle.net:1119"
        );
        assert_eq!(Region::EU.ribbit_address(), "eu.version.battle.net:1119");
    }

    #[test]
    fn test_kr_region_urls() {
        assert_eq!(
            Region::KR.tact_https_url(),
            "https://kr.version.battle.net"
        );
        assert_eq!(
            Region::KR.tact_http_url(),
            "http://kr.patch.battle.net:1119"
        );
        assert_eq!(Region::KR.ribbit_address(), "kr.version.battle.net:1119");
    }

    #[test]
    fn test_tw_region_urls() {
        assert_eq!(
            Region::TW.tact_https_url(),
            "https://tw.version.battle.net"
        );
        assert_eq!(
            Region::TW.tact_http_url(),
            "http://tw.patch.battle.net:1119"
        );
        assert_eq!(Region::TW.ribbit_address(), "tw.version.battle.net:1119");
    }

    #[test]
    fn test_cn_region_urls() {
        assert_eq!(
            Region::CN.tact_https_url(),
            "https://cn.version.battlenet.com.cn"
        );
        assert_eq!(
            Region::CN.tact_http_url(),
            "http://cn.patch.battlenet.com.cn:1119"
        );
        assert_eq!(
            Region::CN.ribbit_address(),
            "cn.version.battlenet.com.cn:1119"
        );
    }

    #[test]
    fn test_sg_region_urls() {
        assert_eq!(
            Region::SG.tact_https_url(),
            "https://sg.version.battle.net"
        );
        assert_eq!(
            Region::SG.tact_http_url(),
            "http://sg.patch.battle.net:1119"
        );
        assert_eq!(Region::SG.ribbit_address(), "sg.version.battle.net:1119");
    }

    #[test]
    fn test_region_display() {
        assert_eq!(Region::US.to_string(), "us");
        assert_eq!(Region::CN.to_string(), "cn");
        assert_eq!(Region::SG.to_string(), "sg");
    }
}
