use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::Level;

use ngdp_client::commands::keys::KeysCommands;
use ngdp_client::{
    CertsCommands, ConfigCommands, DownloadCommands, InspectCommands, OutputFormat,
    ProductsCommands, StorageCommands, cached_client, commands,
};

#[derive(Parser)]
#[command(
    name = "ngdp",
    about = "NGDP client for interacting with Blizzard's content distribution system",
    version,
    author,
    long_about = "A command-line tool for accessing NGDP (Next Generation Distribution Pipeline) services, including Ribbit for product information and TACT for content delivery."
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

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Disable request caching
    #[arg(long, global = true)]
    no_cache: bool,

    /// Clear all cached data before running command
    #[arg(long, global = true)]
    clear_cache: bool,

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

    /// Manage encryption keys
    #[command(subcommand)]
    Keys(KeysCommands),

    /// Certificate operations
    #[command(subcommand)]
    Certs(CertsCommands),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::from(cli.log_level))
        .with_target(false)
        .init();

    // Set global color override if requested
    if cli.no_color {
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
    }

    // Handle cache flags
    if cli.no_cache {
        cached_client::set_caching_enabled(false);
        tracing::debug!("Caching disabled via --no-cache flag");
    }

    if cli.clear_cache {
        // Clear cache for all regions
        tracing::info!("Clearing all cached data...");
        for region in ["us", "eu", "kr", "tw", "cn", "sg"] {
            if let Ok(r) = ribbit_client::Region::from_str(region) {
                if let Ok(client) = cached_client::create_client(r).await {
                    let _ = client.clear_cache().await;
                }
            }
        }
        tracing::info!("Cache cleared successfully");
    }

    // Handle commands
    let result = match cli.command {
        Commands::Products(cmd) => commands::products::handle(cmd, cli.format).await,
        Commands::Storage(cmd) => commands::storage::handle(cmd, cli.format).await,
        Commands::Download(cmd) => commands::download::handle(cmd, cli.format).await,
        Commands::Inspect(cmd) => commands::inspect::handle(cmd, cli.format).await,
        Commands::Config(cmd) => commands::config::handle(cmd, cli.format).await,
        Commands::Certs(cmd) => commands::certs::handle(cmd, cli.format).await,
        Commands::Keys(cmd) => commands::keys::handle_keys_command(cmd).await,
    };

    // Handle errors with more user-friendly messages
    if let Err(e) = result {
        // Check if it's a ribbit connection timeout error
        if let Some(ribbit_error) = e.downcast_ref::<ribbit_client::Error>() {
            match ribbit_error {
                ribbit_client::Error::ConnectionTimeout {
                    host,
                    port,
                    timeout_secs,
                } => {
                    eprintln!("Error: Connection timed out after {timeout_secs} seconds");
                    eprintln!("Failed to connect to {host}:{port}");
                    eprintln!("\nPossible causes:");
                    eprintln!("  - The server may be unreachable from your location");
                    eprintln!("  - Network restrictions may be blocking the connection");
                    eprintln!("  - The service may be temporarily unavailable");

                    if host.contains("cn.version.battle.net") {
                        eprintln!(
                            "\nNote: The CN (China) region servers are typically only accessible from within China."
                        );
                        eprintln!(
                            "Consider using a different region (e.g., --region us, --region eu)"
                        );
                    }
                    std::process::exit(1);
                }
                ribbit_client::Error::ConnectionFailed { host, port } => {
                    eprintln!("Error: Failed to connect to {host}:{port}");
                    eprintln!("\nPlease check your internet connection and try again.");
                    std::process::exit(1);
                }
                _ => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
