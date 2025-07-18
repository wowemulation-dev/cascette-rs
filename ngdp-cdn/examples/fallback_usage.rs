//! Example demonstrating CDN fallback functionality

use ngdp_cdn::{CdnClient, DummyCacheProvider, PriorityHostList, Result};
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to see fallback behavior
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Example 1: Create client with backup CDNs
    info!("Creating CDN client with backup CDNs");

    // Example CDN hosts (these would typically come from TACT CDN manifest)
    let cdn_hosts = PriorityHostList(vec![
        vec![
            "blzddist1-a.akamaihd.net".to_string(),
            "level3.blizzard.com".to_string(),
            "cdn.blizzard.com".to_string(),
        ],
        vec![
            "cdn.arctium.tools".to_string(),
            "tact.mirror.reliquaryhq.com".to_string(),
        ],
    ]);

    // Create a CDN client with default configuration
    let client: CdnClient<PriorityHostList, DummyCacheProvider> =
    CdnClient::builder().hosts(cdn_hosts.clone()).build()?;

    info!("Created CDN client with default configuration");

    // The client will automatically use these backup CDNs:
    // - http://cdn.arctium.tools/
    // - https://tact.mirror.reliquaryhq.com/

    info!("Default CDN hosts: {:?}", client.get_all_cdn_hosts());

    // TODO: rewrite this
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
