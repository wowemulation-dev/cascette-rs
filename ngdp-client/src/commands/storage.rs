use crate::{OutputFormat, StorageCommands};

pub async fn handle(
    cmd: StorageCommands,
    _format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        StorageCommands::Init { path, product } => {
            println!("Storage init not yet implemented");
            println!("Path: {path:?}");
            if let Some(product) = product {
                println!("Product: {product}");
            }
        }
        StorageCommands::Info { path } => {
            println!("Storage info not yet implemented");
            println!("Path: {path:?}");
        }
        StorageCommands::Verify { path, fix } => {
            println!("Storage verify not yet implemented");
            println!("Path: {path:?}");
            println!("Fix: {fix}");
        }
        StorageCommands::Clean { path, dry_run } => {
            println!("Storage clean not yet implemented");
            println!("Path: {path:?}");
            println!("Dry run: {dry_run}");
        }
    }
    Ok(())
}
