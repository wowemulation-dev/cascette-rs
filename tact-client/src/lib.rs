//! TACT protocol client for Cascette

pub mod error;
pub mod http;
pub mod region;
pub mod response_types;

pub use error::{Error, Result};
pub use http::{HttpClient, ProtocolVersion};
pub use region::Region;
pub use response_types::{parse_cdns, parse_versions, CdnEntry, VersionEntry};
