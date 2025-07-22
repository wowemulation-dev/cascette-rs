//! Asynchronously parses an encoding file.
//!
//! Encoding files contain a mapping of CKey (MD5 of the uncompressed/decrypted
//! game data file) -> file length and EKey(s) (MD5 of the compressed/encrypted
//! game data file).
use clap::Parser;
use std::path::PathBuf;
use tact_parser::{Md5, encoding::EncodingTable};
use tokio::{fs::OpenOptions, io::BufReader};
use tracing::*;

#[derive(Parser)]
#[command(name = "parse_encoding_async")]
struct Cli {
    /// Encoding file to parse.
    ///
    /// This file distributed as a BLTE stream, and must be extracted before
    /// use with this tool.
    #[clap(long)]
    pub encoding: PathBuf,

    /// CKey to find in the encoding file, as hex.
    ///
    /// Example: `c95a3144de253ef2444954c6f00b19d4` (English version of
    /// `interface/cinematics/logo_1024.avi`).
    #[clap(long)]
    pub ckey: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let ckey: Md5 = hex::decode(args.ckey)?
        .try_into()
        .map_err(|_| "incorrect length")?;
    info!("Searching for {}", hex::encode(ckey));
    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.encoding).await?);

    info!("Reading encoding table...");
    let encoding = EncodingTable::aparse(&mut rf).await?;
    info!(
        "Table has {} / {} entries",
        encoding.md5_map.len(),
        encoding.md5_map.capacity(),
    );

    // Find the ckey
    if let Some((len, ekeys)) = encoding.md5_map.get(&ckey) {
        info!("CKey length: {len}");
        for ekey in ekeys {
            println!("  - ekey: {}", hex::encode(ekey));
        }
    } else {
        error!("CKey not found!");
    }

    Ok(())
}
