//! Parses a community listfile.
use clap::Parser;
use std::{fs::OpenOptions, io::BufReader, path::PathBuf};
use tact_parser::listfile::{ListfileParser, listfile_normalise};

#[derive(Parser)]
#[command(name = "parse_listfile")]
struct Cli {
    /// Listfile to parse.
    #[clap(long)]
    pub listfile: PathBuf,

    /// Full file path to find in the listfile.
    ///
    /// Must be in lowercase.
    #[clap(long)]
    pub path: Option<String>,

    /// File ID to find in the listfile.
    #[clap(long)]
    pub fid: Option<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    if args.path.is_some() && args.fid.is_some() {
        panic!("Cannot search by file ID and path at the same time")
    }

    let mut rf = BufReader::new(OpenOptions::new().read(true).open(args.listfile)?);

    // When doing one-shot reads, it's more efficient to just scan the file than
    // load everything into a BTreeMap.
    let mut listfile = ListfileParser::new(&mut rf);

    if let Some(path) = args.path {
        let path = listfile_normalise(&path);
        while let Some((f, p)) = listfile.next()? {
            if path == p {
                println!("{path:?} is File ID {f}");
                return Ok(());
            }
        }

        println!("{path:?} not found!");
        return Ok(());
    }

    if let Some(fid) = args.fid {
        while let Some((f, p)) = listfile.next()? {
            if fid == f {
                println!("File ID {fid} is {p:?}");
                return Ok(());
            }
        }

        println!("File ID {fid} not found!");
        return Ok(());
    }

    Ok(())
}
