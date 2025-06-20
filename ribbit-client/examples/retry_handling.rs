//! Example demonstrating retry handling with exponential backoff

use ribbit_client::{Endpoint, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see retry attempts
    tracing_subscriber::fmt::init();

    println!("Ribbit Client Retry Example");
    println!("===========================\n");

    // Example 1: Default client with no retries (backward compatible)
    println!("1. Default client (no retries):");
    let default_client = RibbitClient::new(Region::US);
    match default_client.request_raw(&Endpoint::Summary).await {
        Ok(data) => println!("  Success! Received {} bytes", data.len()),
        Err(e) => println!("  Failed: {}", e),
    }

    println!();

    // Example 2: Client with basic retry configuration
    println!("2. Client with 3 retries:");
    let retry_client = RibbitClient::new(Region::US).with_max_retries(3);
    match retry_client.request_raw(&Endpoint::Summary).await {
        Ok(data) => println!("  Success! Received {} bytes", data.len()),
        Err(e) => println!("  Failed after retries: {}", e),
    }

    println!();

    // Example 3: Custom retry configuration
    println!("3. Client with custom retry configuration:");
    let custom_client = RibbitClient::new(Region::US)
        .with_max_retries(5)
        .with_initial_backoff_ms(200) // Start with 200ms
        .with_max_backoff_ms(5000) // Cap at 5 seconds
        .with_backoff_multiplier(1.5) // Less aggressive backoff
        .with_jitter_factor(0.2); // 20% jitter

    match custom_client.request_raw(&Endpoint::Summary).await {
        Ok(data) => println!("  Success! Received {} bytes", data.len()),
        Err(e) => println!("  Failed after custom retries: {}", e),
    }

    println!();

    // Example 4: Testing with a region that might have connectivity issues
    // CN region often has connection issues from outside China
    println!("4. Testing retry with potentially unreachable region (CN):");
    let cn_client = RibbitClient::new(Region::CN)
        .with_max_retries(2)
        .with_initial_backoff_ms(500);

    match cn_client.request_raw(&Endpoint::Summary).await {
        Ok(data) => println!("  Success! Received {} bytes", data.len()),
        Err(e) => {
            println!("  Expected failure: {}", e);
            println!("  (CN region is often unreachable from outside China)");
        }
    }

    println!();

    // Example 5: Demonstrating different retry strategies
    println!("5. Retry strategies comparison:");

    // Aggressive retry - fast initial backoff, high multiplier
    let _aggressive = RibbitClient::new(Region::US)
        .with_max_retries(3)
        .with_initial_backoff_ms(50)
        .with_backoff_multiplier(3.0);
    println!("  Aggressive: 50ms -> 150ms -> 450ms");

    // Conservative retry - slow initial backoff, low multiplier
    let _conservative = RibbitClient::new(Region::US)
        .with_max_retries(3)
        .with_initial_backoff_ms(1000)
        .with_backoff_multiplier(1.2);
    println!("  Conservative: 1000ms -> 1200ms -> 1440ms");

    // Balanced retry - moderate settings
    let _balanced = RibbitClient::new(Region::US)
        .with_max_retries(3)
        .with_initial_backoff_ms(200)
        .with_backoff_multiplier(2.0);
    println!("  Balanced: 200ms -> 400ms -> 800ms");

    println!("\nNote: Actual backoff times will vary due to jitter");

    Ok(())
}
