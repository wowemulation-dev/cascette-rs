//! TACT configuration file parsers.
mod build;
mod cdn;
mod parser;

pub use self::{build::BuildConfig, cdn::CdnConfig};
