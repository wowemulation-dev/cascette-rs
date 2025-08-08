//! Parses a file with a single complete BLTE stream.
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
//!
//! **TODO:** Support encrypted streams.
use blte::{BLTEFile, ChunkEncodingHeader};
use clap::Parser;
use std::{
    fs::OpenOptions,
    io::{BufReader, Seek, SeekFrom, Write},
    path::PathBuf,
};
use tracing::info;

#[derive(Parser)]
#[command(name = "extract")]
struct Cli {
    /// BLTE stream to read from.
    #[clap(long)]
    pub archive: PathBuf,

    /// Verify the BLTE stream's internal checksums (if available).
    #[clap(long)]
    pub verify_checksum: bool,

    /// Show information about each chunk of the file.
    #[clap(long)]
    pub chunk_info: bool,

    /// File to write a decompressed BLTE stream to.
    #[clap(long)]
    pub output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    // TODO: REMOVE THIS HACK
    if args.archive.is_dir() {
        for p in args.archive.read_dir()? {
            let e = p.unwrap();
            if !e.file_type().is_ok_and(|e| e.is_file()) {
                continue;
            }

            info!("file: {e:?}");

            let mut f = OpenOptions::new().read(true).open(e.path())?;
            let size = f.seek(SeekFrom::End(0))?;
            f.rewind()?;

            let ra = BufReader::new(f);

            info!("Reading BLTE stream ({size} bytes)...");
            let mut stream = BLTEFile::new(ra, 0, size)?;

            info!("Header: {:#x?}", stream.header());

            for chunk in 0..stream.chunk_count() {
                let h = stream.read_chunk_header(chunk)?;
                info!("Chunk {chunk}: {h:#x?}");

                if matches!(h.encoding, ChunkEncodingHeader::Encrypted(_)) {
                    panic!("found encrypted in {e:?}");
                }
            }
        }

        panic!();
    }
    // END HACK

    let mut f = OpenOptions::new().read(true).open(args.archive)?;
    let size = f.seek(SeekFrom::End(0))?;
    f.rewind()?;

    let ra = BufReader::new(f);

    info!("Reading BLTE stream ({size} bytes)...");
    let mut stream = BLTEFile::new(ra, 0, size)?;

    info!("Header: {:#x?}", stream.header());

    info!(
        "Has block-level checksums: {:?}",
        stream.has_chunk_level_checksums(),
    );

    if args.verify_checksum {
        let _ = stream.verify_compressed_checksum()?;
        info!("Checksum OK");
    }

    if args.chunk_info {
        for chunk in 0..stream.chunk_count() {
            let h = stream.read_chunk_header(chunk)?;
            info!("Chunk {chunk}: {h:#x?}");
        }
    }

    if let Some(output) = args.output {
        let o = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output)?;
        let mut o = stream.write_to_file(o)?;
        o.flush()?;
    }

    Ok(())
}
