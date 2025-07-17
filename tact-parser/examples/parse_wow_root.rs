//! The root contains a mapping of file IDs (and sometimes file name hashes) to
//! MD5 checksums of the file content.
use clap::Parser;
use std::{fs::OpenOptions, io::BufReader, path::PathBuf};
use tact_parser::wow_root::{LocaleFlags, WowRoot};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_wow_root")]
struct Cli {
    #[clap(long)]
    pub root: PathBuf,

    /// Filename to find in the root TACT file.
    ///
    /// In Retail, less than 10% of file names are available. The wowdev
    /// community listfile provides a community-maintained list of filename ->
    /// FID mapping.
    #[clap(long)]
    pub filename: Option<String>,

    /// File ID to find in the root TACT file.
    #[clap(long)]
    pub fid: Option<u32>,

    /// Consider any locale, not just English.
    #[clap(long)]
    pub any_locale: bool,
}

fn main() {
    tracing_subscriber::fmt::init();
    let mut args = Cli::parse();

    if args.fid.is_some() && args.filename.is_some() {
        panic!("can't search by both filename and FID at the same time");
    }

    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.root).unwrap());
    let locale = if args.any_locale {
        LocaleFlags::any_locale()
    } else {
        LocaleFlags::new().with_en_us(true).with_en_gb(true)
    };

    info!("Reading WoW TACT root...");
    let root = WowRoot::parse(&mut rf, locale).unwrap();
    info!(
        "Root contains {} File IDs, {} ({:.1}%) file names",
        root.fid_md5.len(),
        root.name_hash_fid.len(),
        (root.name_hash_fid.len() as f64 / root.fid_md5.len() as f64) * 100.,
    );

    if let Some(filename) = args.filename {
        if let Some(fid) = root.get_fid(&filename) {
            println!("File {filename:?} is File ID {fid}");
            println!();
            args.fid = Some(fid);
        } else {
            println!("File {filename:?} not found!");
        }
    }

    if let Some(fid) = args.fid {
        if let Some(md5s) = root.fid_md5.get(&fid) {
            println!("Found {} version(s) of file {fid}:", md5s.len());
            println!();
            for (context, md5) in md5s.iter() {
                println!("MD5: {} => {context:#?}", hex::encode(md5));
            }
        } else {
            println!("FID {fid} not found!");
        }
    }
}
