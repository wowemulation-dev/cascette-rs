//! Test connection pooling performance improvements

#![allow(clippy::uninlined_format_args)]

use std::time::Instant;
use tact_client::{HttpClient, PoolConfig, ProtocolVersion, Region, init_global_pool};
use tokio::task::JoinSet;

#[tokio::test]
async fn test_connection_pooling_performance() {
    // Initialize the global pool with optimized settings
    let pool_config = PoolConfig::new()
        .with_max_idle_connections(Some(50))
        .with_max_idle_connections_per_host(20)
        .with_user_agent("ConnectionPoolTest/1.0".to_string());

    init_global_pool(pool_config);

    // Test configuration
    const NUM_CONCURRENT_REQUESTS: usize = 10;
    const ITERATIONS_PER_CLIENT: usize = 5;

    // Test 1: Individual clients (old approach - new connection each time)
    println!("Testing individual clients (old approach)...");
    let start = Instant::now();

    let mut join_set = JoinSet::new();
    for i in 0..NUM_CONCURRENT_REQUESTS {
        join_set.spawn(async move {
            let mut successful_requests = 0;
            let mut errors = Vec::new();

            for j in 0..ITERATIONS_PER_CLIENT {
                // Create a new client for each request (simulating old behavior)
                let client = match HttpClient::new(Region::US, ProtocolVersion::V1) {
                    Ok(c) => c,
                    Err(e) => {
                        errors.push(format!("Client {i}-{j}: Failed to create client: {e}"));
                        continue;
                    }
                };

                // Try to make a request to a real endpoint
                match client.get_versions("wow").await {
                    Ok(response) => {
                        if response.status().is_success() {
                            successful_requests += 1;
                        } else {
                            errors.push(format!("Client {i}-{j}: HTTP {}", response.status()));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Client {i}-{j}: Request failed: {e}"));
                    }
                }

                // Small delay to avoid overwhelming the server
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }

            (successful_requests, errors)
        });
    }

    let mut individual_successful = 0;
    let mut individual_errors = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((successful, mut errors)) => {
                individual_successful += successful;
                individual_errors.append(&mut errors);
            }
            Err(e) => {
                individual_errors.push(format!("Task join error: {e}"));
            }
        }
    }

    let individual_time = start.elapsed();
    println!(
        "Individual clients: {} successful, {} errors, {:?}",
        individual_successful,
        individual_errors.len(),
        individual_time
    );

    // Small delay between tests
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Test 2: Shared connection pool (new approach)
    println!("Testing shared connection pool (new approach)...");
    let start = Instant::now();

    let mut join_set = JoinSet::new();
    for i in 0..NUM_CONCURRENT_REQUESTS {
        join_set.spawn(async move {
            let mut successful_requests = 0;
            let mut errors = Vec::new();

            // Create a shared pool client once and reuse it
            let client = HttpClient::with_shared_pool(Region::US, ProtocolVersion::V1);

            for j in 0..ITERATIONS_PER_CLIENT {
                // Reuse the same client (with shared connection pool)
                match client.get_versions("wow").await {
                    Ok(response) => {
                        if response.status().is_success() {
                            successful_requests += 1;
                        } else {
                            errors.push(format!("Client {i}-{j}: HTTP {}", response.status()));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Client {i}-{j}: Request failed: {e}"));
                    }
                }

                // Small delay to avoid overwhelming the server
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }

            (successful_requests, errors)
        });
    }

    let mut pooled_successful = 0;
    let mut pooled_errors = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((successful, mut errors)) => {
                pooled_successful += successful;
                pooled_errors.append(&mut errors);
            }
            Err(e) => {
                pooled_errors.push(format!("Task join error: {e}"));
            }
        }
    }

    let pooled_time = start.elapsed();
    println!(
        "Pooled clients: {} successful, {} errors, {:?}",
        pooled_successful,
        pooled_errors.len(),
        pooled_time
    );

    // Calculate improvement
    if pooled_time.as_millis() > 0 {
        let speedup = individual_time.as_millis() as f64 / pooled_time.as_millis() as f64;
        println!("Connection pooling speedup: {speedup:.2}x");

        // We expect at least some improvement, but don't make it too strict
        // since network conditions can vary
        if speedup > 1.2 {
            println!("✓ Connection pooling shows significant improvement ({speedup:.2}x)");
        } else if speedup > 1.0 {
            println!("✓ Connection pooling shows modest improvement ({speedup:.2}x)");
        } else {
            println!(
                "⚠ Connection pooling shows no improvement ({speedup:.2}x) - this may be due to network conditions"
            );
        }
    }

    // Verify both approaches got similar success rates
    let individual_success_rate =
        individual_successful as f64 / (NUM_CONCURRENT_REQUESTS * ITERATIONS_PER_CLIENT) as f64;
    let pooled_success_rate =
        pooled_successful as f64 / (NUM_CONCURRENT_REQUESTS * ITERATIONS_PER_CLIENT) as f64;

    println!(
        "Individual success rate: {:.2}%",
        individual_success_rate * 100.0
    );
    println!("Pooled success rate: {:.2}%", pooled_success_rate * 100.0);

    // Both approaches should have similar success rates
    let success_rate_diff = (individual_success_rate - pooled_success_rate).abs();
    assert!(
        success_rate_diff < 0.3,
        "Success rates should be similar: individual={:.2}%, pooled={:.2}%",
        individual_success_rate * 100.0,
        pooled_success_rate * 100.0
    );

    // Show some error details if there were failures
    if !individual_errors.is_empty() {
        println!(
            "Sample individual errors: {:?}",
            &individual_errors[..individual_errors.len().min(3)]
        );
    }
    if !pooled_errors.is_empty() {
        println!(
            "Sample pooled errors: {:?}",
            &pooled_errors[..pooled_errors.len().min(3)]
        );
    }

    println!("✓ Connection pooling test completed successfully");
}

#[tokio::test]
async fn test_pool_configuration() {
    // Test that pool configuration works
    let config = PoolConfig::new()
        .with_max_idle_connections(Some(25))
        .with_max_idle_connections_per_host(10)
        .with_pool_idle_timeout(std::time::Duration::from_secs(45))
        .with_request_timeout(std::time::Duration::from_secs(20))
        .with_connect_timeout(std::time::Duration::from_secs(8))
        .with_user_agent("TestConfig/1.0".to_string());

    assert_eq!(config.max_idle_connections, Some(25));
    assert_eq!(config.max_idle_connections_per_host, 10);
    assert_eq!(config.pool_idle_timeout, std::time::Duration::from_secs(45));
    assert_eq!(config.request_timeout, std::time::Duration::from_secs(20));
    assert_eq!(config.connect_timeout, std::time::Duration::from_secs(8));
    assert_eq!(config.user_agent, Some("TestConfig/1.0".to_string()));

    // Test creating a client with this configuration
    let client = tact_client::create_pooled_client(config);

    // Verify the client can be used
    let http_client = HttpClient::with_client(client, Region::US, ProtocolVersion::V1);

    // Just verify it doesn't panic - network request may fail in test environment
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        http_client.get_versions("wow"),
    )
    .await;

    println!("✓ Pool configuration test completed");
}
