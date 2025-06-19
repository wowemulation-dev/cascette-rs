use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::Level;

use ngdp_client::{
    ConfigCommands, DownloadCommands, InspectCommands, OutputFormat, ProductsCommands,
    StorageCommands, commands,
};

#[derive(Parser)]
#[command(
    name = "ngdp",
    about = "NGDP client for interacting with Blizzard's content distribution system",
    version,
    author,
    long_about = "A command-line tool for accessing NGDP (Next Generation Data Pipeline) services, including Ribbit for product information and TACT for content delivery."
)]
struct Cli {
    /// Set the logging level
    #[arg(short, long, value_enum, default_value = "info")]
    log_level: LogLevel,

    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Output format
    #[arg(short = 'o', long, value_enum, global = true, default_value = "text")]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Query product information from Ribbit
    #[command(subcommand)]
    Products(ProductsCommands),

    /// Manage local CASC storage
    #[command(subcommand)]
    Storage(StorageCommands),

    /// Download content using TACT
    #[command(subcommand)]
    Download(DownloadCommands),

    /// Inspect NGDP data structures
    #[command(subcommand)]
    Inspect(InspectCommands),

    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::from(cli.log_level))
        .with_target(false)
        .init();

    // Handle commands
    match cli.command {
        Commands::Products(cmd) => commands::products::handle(cmd, cli.format).await?,
        Commands::Storage(cmd) => commands::storage::handle(cmd, cli.format).await?,
        Commands::Download(cmd) => commands::download::handle(cmd, cli.format).await?,
        Commands::Inspect(cmd) => commands::inspect::handle(cmd, cli.format).await?,
        Commands::Config(cmd) => commands::config::handle(cmd, cli.format).await?,
    }

    Ok(())
}
