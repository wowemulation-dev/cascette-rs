//! # TACT Parser
//!
//! Parser for various TACT (Torrent-Assisted Content Transfer) file formats
//! used by Blizzard Entertainment's NGDP (Next Generation Distribution Pipeline).
//!
//! This crate provides parsers for the file formats used to distribute game data
//! through Blizzard's CDN. While some community tooling refers to these as "CASC files",
//! CASC specifically refers to the virtual filesystem used by locally-installed games.
//!
//! ## Features
//!
//! - **WoW Root Parsing**: Read World of Warcraft root files to find file IDs and MD5 hashes
//! - **Jenkins3 Hashing**: Implementation of the Jenkins3 hash algorithm used by TACT
//! - **Efficient I/O**: Buffered I/O operations for parsing large game data files
//! - **Format Support**: Both modern (8.2+) and legacy pre-8.2 root file formats
//!
//! ## Quick Start
//!
//! Parse a WoW root file to find game data files:
//!
//! ```no_run
//! use tact_parser::wow_root::{WowRootHeader, LocaleFlags, ContentFlags};
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse root file header
//! let mut file = BufReader::new(File::open("path/to/root")?);
//! let header = WowRootHeader::parse(&mut file)?;
//!
//! println!("Root file version: {}", header.version);
//! println!("Total files: {}", header.total_file_count);
//! # Ok(())
//! # }
//! ```
//!
//! ## Roadmap
//!
//! Current implementation status:
//!
//! - ✅ WoW Root file parsing
//! - ✅ Jenkins3 hash implementation
//! - ⏳ Encoding table parsing (planned)
//! - ⏳ BLTE file decoding (planned)
//! - ⏳ Patch file support (planned)
//!
//! ## See Also
//!
//! - [`ngdp-client`](https://docs.rs/ngdp-client) - CLI tool for NGDP operations
//! - [`tact-client`](https://docs.rs/tact-client) - TACT protocol client
//! - [TACT Format Documentation](https://wowdev.wiki/TACT)

mod error;
mod ioutils;
pub mod jenkins3;
pub mod utils;
pub mod wow_root;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
