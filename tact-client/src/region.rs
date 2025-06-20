//! Region support for TACT protocol

use std::fmt;

/// Supported regions for TACT protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// United States
    US,
    /// Europe
    EU,
    /// Korea
    KR,
    /// China
    CN,
    /// Taiwan
    TW,
}

impl Region {
    /// Get all available regions
    pub fn all() -> &'static [Region] {
        &[Region::US, Region::EU, Region::KR, Region::CN, Region::TW]
    }

    /// Convert region to lowercase string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Region::US => "us",
            Region::EU => "eu",
            Region::KR => "kr",
            Region::CN => "cn",
            Region::TW => "tw",
        }
    }

    /// Parse region from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "us" => Some(Region::US),
            "eu" => Some(Region::EU),
            "kr" => Some(Region::KR),
            "cn" => Some(Region::CN),
            "tw" => Some(Region::TW),
            _ => None,
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Region {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Region::parse(s).ok_or_else(|| crate::Error::InvalidRegion(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_parse() {
        assert_eq!(Region::parse("us"), Some(Region::US));
        assert_eq!(Region::parse("US"), Some(Region::US));
        assert_eq!(Region::parse("eu"), Some(Region::EU));
        assert_eq!(Region::parse("invalid"), None);
    }

    #[test]
    fn test_region_from_str() {
        use std::str::FromStr;

        assert_eq!(Region::from_str("us").unwrap(), Region::US);
        assert_eq!(Region::from_str("EU").unwrap(), Region::EU);
        assert!(Region::from_str("invalid").is_err());
    }

    #[test]
    fn test_region_display() {
        assert_eq!(Region::US.to_string(), "us");
        assert_eq!(Region::EU.to_string(), "eu");
    }
}
