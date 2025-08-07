//! TACT protocol client for Cascette

pub mod error;
pub mod http;
pub mod region;
pub mod response_types;
pub mod resumable;

pub use error::{Error, Result};
pub use http::{HttpClient, ProtocolVersion};
pub use region::Region;
pub use response_types::{CdnEntry, VersionEntry, parse_cdns, parse_versions};
pub use resumable::{DownloadProgress, ResumableDownload};
