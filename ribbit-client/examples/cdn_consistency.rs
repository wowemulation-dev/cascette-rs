//! Example demonstrating consistent CDN handling between Ribbit and TACT clients

use ribbit_client::{Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Ribbit client
    let client = RibbitClient::new(Region::US);

    println!("Fetching CDN data via Ribbit protocol...\n");

    // Get CDN information
    let cdns = client.get_product_cdns("wow").await?;

    println!("Found {} CDN configurations:", cdns.entries.len());
    println!("{:-<80}", "");

    for entry in &cdns.entries {
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

    // Demonstrate URL construction
    if let Some(cdn) = cdns.entries.first() {
        println!("CDN Usage Examples:");
        println!("==================\n");

        let example_hash = "e359107662e72559b4e1ab721b157cb0";

        // Using hosts field (legacy)
        if let Some(host) = cdn.hosts.first() {
            println!("Using hosts field (add protocol manually):");
            println!(
                "  http://{}/{}/data/{}/{}/{}",
                host,
                cdn.path,
                &example_hash[0..2],
                &example_hash[2..4],
                example_hash
            );
        }

        // Using servers field (modern)
        if let Some(server) = cdn.servers.first() {
            println!("\nUsing servers field (protocol included):");
            println!(
                "  {}/{}/data/{}/{}/{}",
                server.trim_end_matches('/'),
                cdn.path,
                &example_hash[0..2],
                &example_hash[2..4],
                example_hash
            );
        }

        println!("\nNote: Both Ribbit and TACT HTTP clients now parse servers as Vec<String>");
        println!("This ensures consistent handling across both protocols!");
    }

    Ok(())
}
