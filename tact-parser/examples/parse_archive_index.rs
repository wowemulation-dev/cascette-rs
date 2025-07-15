use clap::Parser;
use std::{fs::OpenOptions, io::BufReader, path::PathBuf};
use tact_parser::{Md5, archive::ArchiveIndexParser};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_archive_index")]
struct Cli {
    #[clap(long)]
    pub archive_index: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    // Get the hash part of the filename
    let hash: Md5 = hex::decode(args.archive_index.file_stem().unwrap().as_encoded_bytes())?
        .as_slice()
        .try_into()?;

    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.archive_index)?);

    info!("Reading archive index...");
    let mut parser = ArchiveIndexParser::new(&mut rf, &hash)?;

    info!("Index footer: {:#?}", parser.footer());

    info!("TOC contains {} entries:", parser.toc().last_ekey.len());
    for (last_ekey, partial_md5) in parser
        .toc()
        .last_ekey
        .iter()
        .zip(parser.toc().block_partial_md5.iter())
    {
        println!(
            "  - EKey {}, MD5 {}",
            hex::encode(last_ekey),
            hex::encode(partial_md5)
        );
    }
    println!();
    info!("Reading block 0...");
    for entry in parser.read_block(0)? {
        println!(
            "  - {}, @{:#x}, {:#x} bytes",
            hex::encode(entry.ekey),
            entry.archive_offset,
            entry.blte_encoded_size
        );
    }

    Ok(())
}
