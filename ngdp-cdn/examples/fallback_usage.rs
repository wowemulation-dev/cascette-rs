//! Example demonstrating CDN fallback functionality

use ngdp_cdn::{
    CdnClient, CdnClientBuilder, CdnClientBuilderTrait as _, CdnClientWithFallback,
    CdnClientWithFallbackBuilder, FallbackCdnClientTrait as _, Result,
};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to see fallback behavior
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Example 1: Create client with default backup CDNs
    info!("Creating CDN client with default backup CDNs");
    let client: CdnClientWithFallback<CdnClient> = CdnClientWithFallback::new().await?;

    // The client will automatically use these backup CDNs:
    // - http://cdn.arctium.tools/
    // - https://tact.mirror.reliquaryhq.com/

    info!("Default CDN hosts: {:?}", client.get_all_cdn_hosts());

    // Example 2: Add primary CDNs from Ribbit response
    info!("\nAdding primary CDNs (Blizzard servers)");
    client.add_primary_cdns(vec![
        "blzddist1-a.akamaihd.net",
        "level3.blizzard.com",
        "blzddist2-a.akamaihd.net",
    ]);

    info!("CDN order (Blizzard first, then community backups):");
    for (i, host) in client.get_all_cdn_hosts().iter().enumerate() {
        info!("  {}. {}", i + 1, host);
    }

    // Example 3: Custom configuration without default backups
    info!("\nCreating client with custom configuration");
    let custom_client: CdnClientWithFallback<CdnClient> = CdnClientWithFallbackBuilder::new()
        .add_primary_cdn("primary.example.com")
        .add_primary_cdn("secondary.example.com")
        .use_default_backups(false)
        .configure_base_client(|builder: CdnClientBuilder| {
            builder
                .max_retries(5)
                .initial_backoff_ms(200)
                .connect_timeout(60)
        })
        .build()
        .await?;

    info!(
        "Custom client CDN hosts: {:?}",
        custom_client.get_all_cdn_hosts()
    );

    // Example 4: Download with automatic fallback
    info!("\nDemonstrating download with fallback");
    info!("Note: This will fail since we're using example CDNs");

    // In a real scenario, if the primary CDN fails, it will automatically
    // try the next CDN in the list until one succeeds or all fail
    match custom_client
        .download("tpr/wow", "1234567890abcdef", "")
        .await
    {
        Ok(_response) => {
            info!("Download succeeded!");
        }
        Err(e) => {
            info!("Download failed (expected in this example): {}", e);
        }
    }

    // Example 5: Integrating with Ribbit client
    info!("\nExample integration with Ribbit client:");
    info!("// After getting CDN entries from Ribbit:");
    info!("// let cdn_entries = ribbit_client.get_cdns_parsed(\"wow\").await?;");
    info!("// ");
    info!("// // Add all CDN hosts from the response");
    info!("// for entry in cdn_entries {{");
    info!("//     client.add_primary_cdns(&entry.hosts);");
    info!("// }}");

    Ok(())
}
