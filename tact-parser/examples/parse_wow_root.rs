use clap::Parser;
use std::{fs::OpenOptions, path::PathBuf};
use tact_parser::wow_root::{LocaleFlags, WowRoot};
use tracing::info;

#[derive(Parser)]
#[command(name = "parse_wow_root")]
struct Cli {
    #[clap(long)]
    pub root: PathBuf,

    /// Filename to find in the root TACT file.
    #[clap(long)]
    pub filename: String,

    /// Consider any locale, not just English.
    #[clap(long)]
    pub any_locale: bool,
}

fn main() {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let mut rf = OpenOptions::new().read(true).open(args.root).unwrap();
    let locale = if args.any_locale {
        LocaleFlags::any_locale()
    } else {
        LocaleFlags::new().with_en_us(true).with_en_gb(true)
    };

    info!("Reading WoW TACT root...");
    let root = WowRoot::parse(&mut rf, locale).unwrap();
    info!("Root contains {} files", root.fid_md5.len());

    // Find the file
    // interface/cinematics/logo_1024.avi = 21
    if let Some(fid) = root.get_fid(&args.filename) {
        println!("File {:?} is File ID {fid}", args.filename);

        // This should not fail
        if let Some(md5s) = root.fid_md5.get(&fid) {
            println!();
            println!("Found {} version(s) of the file:", md5s.len());
            println!();
            for (context, md5) in md5s.iter() {
                println!("MD5: {} => {context:#?}", hex::encode(md5));
            }
        }
    } else {
        println!("File {:?} not found!", args.filename);
    }
}
