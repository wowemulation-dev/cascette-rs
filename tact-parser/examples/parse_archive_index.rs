use clap::Parser;
use std::{fs::OpenOptions, io::BufReader, path::PathBuf};
use tact_parser::archive::{ArchiveIndexFooter, ArchiveIndexToc};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_archive_index")]
struct Cli {
    #[clap(long)]
    pub archive_index: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    // Get the hash part of the filename
    let hash = hex::decode(args.archive_index.file_stem().unwrap().as_encoded_bytes()).unwrap();

    let mut rf = BufReader::new(
        OpenOptions::new()
            .read(true)
            .open(args.archive_index)
            .unwrap(),
    );

    info!("Reading archive index...");
    // TODO: make a proper wrapper
    let footer = ArchiveIndexFooter::parse(&mut rf, hash.as_slice().try_into().unwrap()).unwrap();
    info!("Index footer: {footer:#?}");
    let toc = ArchiveIndexToc::parse(&mut rf, &footer).unwrap();

    info!("TOC contains {} entries:", toc.last_ekey.len());
    for (last_ekey, partial_md5) in toc.last_ekey.iter().zip(toc.block_partial_md5.iter()) {
        println!(
            "  - EKey {}, MD5 {}",
            hex::encode(last_ekey),
            hex::encode(partial_md5)
        );
    }
}
