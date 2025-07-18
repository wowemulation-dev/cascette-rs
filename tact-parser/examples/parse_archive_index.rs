use clap::Parser;
use std::{
    fs::OpenOptions,
    io::{BufReader, Cursor},
    path::PathBuf,
};
use tact_parser::{Md5, archive::ArchiveIndexParser, blte::BlteExtractor};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_archive_index")]
struct Cli {
    /// Archive index to parse.
    #[clap(long)]
    pub archive_index: PathBuf,

    #[clap(long)]
    pub archive: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    // Get the hash part of the filename
    let hash: Md5 = hex::decode(args.archive_index.file_stem().unwrap().as_encoded_bytes())?
        .as_slice()
        .try_into()?;

    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.archive_index)?);
    let mut ra = if let Some(archive) = args.archive {
        Some(BufReader::new(OpenOptions::new().read(true).open(archive)?))
    } else {
        None
    };

    info!("Reading archive index...");
    let mut parser = ArchiveIndexParser::new(&mut rf, &hash)?;

    info!("Index footer: {:#?}", parser.footer());

    info!("TOC contains {} entries:", parser.toc().last_ekey.len());
    for (block, last_ekey) in parser.toc().last_ekey.iter().enumerate() {
        println!("  - Block {block} last EKey: {}", hex::encode(last_ekey));
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

        // HACK: testing that we can extract data
        // This will not be the actual API :)
        if let Some(ra) = ra.as_mut() {
            let mut archive =
                BlteExtractor::new(ra, entry.archive_offset, entry.blte_encoded_size)?;
            info!(
                "Archive has {} compressed and {} decompressed bytes",
                entry.blte_encoded_size,
                archive.header().total_decompressed_size(),
            );

            for block in 0..archive.header().block_count() {
                let block_data_offset = archive.header().block_data_offset(block).unwrap();
                let abs_block_offset = entry.archive_offset + block_data_offset;
                let info = archive.read_block_header(block)?;
                println!("    * Block {block}: @{abs_block_offset:#x}: {info:?}");
            }

            // 0xc51e02d: single plain text blob, OGG file
            // 0x3e136f7: single zlib blob
            // 0x2c3a0b2: multiple zlib blobs
            if entry.archive_offset != 0x2c3a0b2 {
                continue;
            }

            // read it
            let o = Vec::with_capacity(archive.header().total_decompressed_size() as usize);
            let v = Cursor::new(o);
            let v = archive.write_to_file(v)?.into_inner();

            info!("read {} bytes:", v.len());
            // println!("{}", hex::encode(v));

            panic!("found first");
        }
    }

    Ok(())
}
