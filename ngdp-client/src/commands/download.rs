use crate::{DownloadCommands, OutputFormat};

pub async fn handle(
    cmd: DownloadCommands,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        DownloadCommands::Build {
            product,
            build,
            output,
            region,
        } => {
            println!("Download build not yet implemented");
            println!("Product: {}", product);
            println!("Build: {}", build);
            println!("Output: {:?}", output);
            println!("Region: {}", region);
        }
        DownloadCommands::Files {
            product,
            patterns,
            output,
            build,
        } => {
            println!("Download files not yet implemented");
            println!("Product: {}", product);
            println!("Patterns: {:?}", patterns);
            println!("Output: {:?}", output);
            if let Some(build) = build {
                println!("Build: {}", build);
            }
        }
        DownloadCommands::Resume { session } => {
            println!("Resume download not yet implemented");
            println!("Session: {}", session);
        }
    }
    Ok(())
}
