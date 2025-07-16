//! Parses a file with a single complete BTLE stream.
//!
//! Normally, archives are ~256 MiB blobs containing multiple BLTE streams,
//! and readers need an index file to jump to the correct position for each
//! file.
//!
//! SimulationCraft's `casc_extract` downloads files from the CDN using the HTTP
//! `Range` header to fetch a single BLTE stream, and then writes these to disk
//! named by their MD5.
//!
//! This is intended for reading such files.
use clap::Parser;
use std::{
    fs::OpenOptions,
    io::{BufReader, Seek, SeekFrom, Write},
    path::PathBuf,
};
use tact_parser::blte::BlteExtractor;
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_solo_blte_stream")]
struct Cli {
    #[clap(long)]
    pub archive: PathBuf,

    #[clap(long)]
    pub verify_checksum: bool,

    #[clap(long)]
    pub output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let mut f = OpenOptions::new().read(true).open(args.archive)?;
    let size = f.seek(SeekFrom::End(0))?;
    f.seek(SeekFrom::Start(0))?;

    let ra = BufReader::new(f);

    info!("Reading BLTE stream ({size} bytes)...");
    let mut stream = BlteExtractor::new(ra, 0, size)?;

    info!("Header: {:#x?}", stream.header());

    info!(
        "Has block-level checksums: {:?}",
        stream.has_block_level_checksums(),
    );

    if args.verify_checksum {
        let _ = stream.verify_compressed_checksum()?;
        info!("Checksum OK");
    }

    if let Some(output) = args.output {
        let o = OpenOptions::new().create_new(true).write(true).open(output)?;
        let mut o = stream.write_to_file(o)?;
        o.flush()?;
    }

    Ok(())
}
