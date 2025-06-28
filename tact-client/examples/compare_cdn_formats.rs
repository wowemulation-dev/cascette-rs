//! Example demonstrating CDN data handling from both HTTP and Ribbit protocols

use tact_client::{HttpClient, ProtocolVersion, Region, parse_cdns};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Create HTTP client for TACT v1
    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

    println!("Fetching CDN data for World of Warcraft...\n");

    // Get CDN information
    let response = client.get_cdns("wow").await?;
    let cdn_data = response.text().await?;
    let cdn_entries = parse_cdns(&cdn_data)?;

    println!("Found {} CDN configurations:", cdn_entries.len());
    println!("{:-<80}", "");

    for entry in &cdn_entries {
        println!("Region: {}", entry.name);
        println!("Path: {}", entry.path);
        println!("Config Path: {}", entry.config_path);

        println!("\nHosts ({}):", entry.hosts.len());
        for host in &entry.hosts {
            println!("  - {host}");
        }

        println!("\nServers ({}):", entry.servers.len());
        if entry.servers.is_empty() {
            println!("  (none - legacy CDN entry)");
        } else {
            for server in &entry.servers {
                println!("  - {server}");
            }
        }

        println!("\n{:-<80}\n", "");
    }

    // Demonstrate the difference between hosts and servers
    if let Some(cdn) = cdn_entries.first() {
        println!("CDN Usage Examples:");
        println!("==================\n");

        println!("Using legacy 'hosts' field:");
        for host in &cdn.hosts {
            println!("  http://{}/{}/data/", host, cdn.path);
        }

        println!("\nUsing modern 'servers' field:");
        if cdn.servers.is_empty() {
            println!("  (No servers field available for this CDN)");
        } else {
            for server in &cdn.servers {
                // Servers already include protocol and query params
                println!("  {}/{}/data/", server.trim_end_matches('/'), cdn.path);
            }
        }

        println!("\nExample file URL construction:");
        let example_hash = "e359107662e72559b4e1ab721b157cb0";
        if let Some(host) = cdn.hosts.first() {
            println!(
                "  Legacy: http://{}/{}/data/{}/{}/{}",
                host,
                cdn.path,
                &example_hash[0..2],
                &example_hash[2..4],
                example_hash
            );
        }
        if let Some(server) = cdn.servers.first() {
            println!(
                "  Modern: {}/{}/data/{}/{}/{}",
                server.trim_end_matches('/'),
                cdn.path,
                &example_hash[0..2],
                &example_hash[2..4],
                example_hash
            );
        }
    }

    Ok(())
}
