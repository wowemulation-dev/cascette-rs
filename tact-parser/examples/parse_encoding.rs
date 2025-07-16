use clap::Parser;
use std::{fs::OpenOptions, io::BufReader, path::PathBuf};
use tact_parser::{Md5, encoding::EncodingTable};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_encoding")]
struct Cli {
    #[clap(long)]
    pub encoding: PathBuf,

    /// CKey to find in the encoding file, in base16
    #[clap(long)]
    pub ckey: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let ckey: Md5 = hex::decode(args.ckey)?
        .try_into()
        .map_err(|_| "incorrect length")?;
    info!("Searching for {}", hex::encode(ckey));
    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.encoding)?);

    info!("Reading encoding table...");
    let encoding = EncodingTable::parse(&mut rf)?;
    info!("Table has {} entries", encoding.md5_map.len());

    // Find the ckey
    if let Some((len, ekeys)) = encoding.md5_map.get(&ckey) {
        println!("CKey length: {len}");
        for ekey in ekeys {
            println!("  - ekey: {}", hex::encode(ekey));
        }
    } else {
        println!("CKey not found!");
    }

    Ok(())
}
