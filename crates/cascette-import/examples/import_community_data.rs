//! Community Data Import Example
//!
//! Demonstrates fetching and importing data from all three community sources:
//! - wago.tools: Historical build information
//! - WoWDev Listfile: File ID to path mappings
//! - WoWDev TACT Keys: Encryption keys
//!
//! Run with: cargo run --example import_community_data -p cascette-import
//!
//! Requires network access (makes HTTP requests to wago.tools, GitHub).

use cascette_import::{
    BuildSearchCriteria, ImportManager, ListfileProvider, TactKeysProvider, WagoProvider,
};
use std::error::Error;

/// Supported WoW product codes and their descriptions.
const WOW_PRODUCTS: &[(&str, &str)] = &[
    ("wow", "Retail"),
    ("wow_classic", "Classic (progressive)"),
    ("wow_classic_era", "Classic Era (pre-TBC)"),
    ("wow_classic_titan", "Classic WotLK China"),
    ("wow_anniversary", "Anniversary (TBC+)"),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Community Data Import ===\n");

    // Step 1: Set up cache directory
    let cache_dir = tempfile::tempdir()?;
    println!("Step 1: Cache directory: {}\n", cache_dir.path().display());

    // Step 2: Create providers
    println!("Step 2: Creating providers...");

    let wago = WagoProvider::new(cache_dir.path().join("wago"))?;
    println!("  - WagoProvider created");

    let listfile = ListfileProvider::new(cache_dir.path().join("listfile"))?;
    println!("  - ListfileProvider created");

    let tactkeys = TactKeysProvider::new(cache_dir.path().join("tactkeys"))?;
    println!("  - TactKeysProvider created");
    println!();

    // Step 3: Register with ImportManager
    // add_provider calls initialize() on each provider, which fetches data
    // from the upstream source and populates the disk cache.
    println!("Step 3: Registering providers with ImportManager...");

    let mut manager = ImportManager::new();

    print!("  Initializing wago.tools (fetches all builds)... ");
    manager.add_provider("wago", Box::new(wago)).await?;
    println!("done");

    print!("  Initializing listfile (downloads ~200 MB CSV)... ");
    manager.add_provider("listfile", Box::new(listfile)).await?;
    println!("done");

    print!("  Initializing TACT keys... ");
    manager.add_provider("tactkeys", Box::new(tactkeys)).await?;
    println!("done");

    println!("  Registered providers: {:?}\n", manager.list_providers());

    // Step 4: Fetch builds for each supported WoW product
    println!("Step 4: Fetching builds per product...\n");

    for &(code, description) in WOW_PRODUCTS {
        let builds = manager.get_builds(code).await?;
        let count = builds.len();

        print!("  {code:<25} ({description:<25}) {count:>4} builds");
        if let Some(latest) = builds.last() {
            print!("  latest: v{}", latest.version);
        }
        println!();
    }

    let all_builds = manager.get_all_builds().await?;
    println!(
        "\n  Total across all wago.tools products: {}\n",
        all_builds.len()
    );

    // Step 5: Search builds with criteria across products
    println!("Step 5: Searching builds with criteria...\n");

    // Classic Era 1.15.x builds
    let era_criteria = BuildSearchCriteria {
        product: Some("wow_classic_era".to_string()),
        version_pattern: Some("1.15.*".to_string()),
        ..BuildSearchCriteria::default()
    };
    let era_builds = manager.search_builds(&era_criteria).await?;
    println!("  Classic Era 1.15.*: {} builds", era_builds.len());
    for build in era_builds.iter().take(3) {
        println!("    v{} (build {})", build.version, build.build);
    }

    // Classic progressive Cataclysm builds (4.x)
    let cata_criteria = BuildSearchCriteria {
        product: Some("wow_classic".to_string()),
        version_pattern: Some("4.*".to_string()),
        ..BuildSearchCriteria::default()
    };
    let cata_builds = manager.search_builds(&cata_criteria).await?;
    println!("  Classic Cata 4.*: {} builds", cata_builds.len());
    for build in cata_builds.iter().take(3) {
        println!("    v{} (build {})", build.version, build.build);
    }

    // Retail live builds with build number >= 50000
    let retail_criteria = BuildSearchCriteria {
        product: Some("wow".to_string()),
        version_type: Some("live".to_string()),
        min_build: Some(50000),
        ..BuildSearchCriteria::default()
    };
    let retail_builds = manager.search_builds(&retail_criteria).await?;
    println!(
        "  Retail live (build >= 50000): {} builds",
        retail_builds.len()
    );

    // Anniversary edition builds
    let anniversary_criteria = BuildSearchCriteria {
        product: Some("wow_anniversary".to_string()),
        ..BuildSearchCriteria::default()
    };
    let anniversary_builds = manager.search_builds(&anniversary_criteria).await?;
    println!("  Anniversary: {} builds", anniversary_builds.len());

    // Titan (China Classic WotLK)
    let titan_criteria = BuildSearchCriteria {
        product: Some("wow_classic_titan".to_string()),
        ..BuildSearchCriteria::default()
    };
    let titan_builds = manager.search_builds(&titan_criteria).await?;
    println!("  Titan (CN WotLK): {} builds\n", titan_builds.len());

    // Step 6: Resolve FileDataIDs via listfile
    println!("Step 6: Resolving FileDataIDs...");

    // Well-known FileDataIDs from WoW
    let test_ids: &[(u32, &str)] = &[
        (1375801, "Interface/Icons/INV_Misc_QuestionMark"),
        (2724755, "World/Maps/Azeroth data"),
        (53187, "Interface/GLUES related"),
    ];

    for &(file_id, description) in test_ids {
        match manager.resolve_file_id(file_id).await? {
            Some(path) => println!("  FileDataID {file_id}: {path}"),
            None => println!("  FileDataID {file_id}: not found ({description})"),
        }
    }
    println!();

    // Step 7: Show TACT key count
    println!("Step 7: TACT key summary...");
    let stats = manager.get_cache_stats().await;
    if let Some(tact_stats) = stats.get("tactkeys") {
        println!("  Keys loaded: {}", tact_stats.entries);
        if let Some(ts) = tact_stats.last_refresh {
            println!("  Last refresh: {ts} (unix timestamp)");
        }
    }
    println!();

    // Step 8: Display cache stats from all providers
    println!("Step 8: Cache statistics...");
    for (name, provider_stats) in &stats {
        println!("  [{name}]");
        println!("    Entries: {}", provider_stats.entries);
        println!("    Size: {} KB", provider_stats.size_bytes / 1024);
        println!("    Hit rate: {:.1}%", provider_stats.hit_rate());
        if let Some(ts) = provider_stats.last_refresh {
            println!("    Last refresh: {ts}");
        }
    }

    // Health check
    println!("\n=== Provider Health ===\n");
    for name in manager.list_providers() {
        let healthy = manager.get_provider_health(&name).unwrap_or(false);
        let status = if healthy { "healthy" } else { "unavailable" };
        println!("  {name}: {status}");
    }

    // Inspect cache directory contents before tempdir cleanup
    println!("\n=== Cache Directory Contents ===\n");
    for entry in std::fs::read_dir(cache_dir.path())? {
        let entry = entry?;
        let subdir = entry.path();
        println!(
            "  {}:",
            subdir.file_name().unwrap_or_default().to_string_lossy()
        );
        if subdir.is_dir() {
            for file in std::fs::read_dir(&subdir)? {
                let file = file?;
                let meta = file.metadata()?;
                println!(
                    "    {} ({} KB)",
                    file.file_name().to_string_lossy(),
                    meta.len() / 1024
                );
            }
        }
    }

    println!("\nDone.");
    Ok(())
}
