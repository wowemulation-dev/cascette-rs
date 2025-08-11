# test-utils

Test utilities for cascette-rs, providing WoW data discovery and test helpers.

## Overview

This crate provides utilities for discovering World of Warcraft installation data for use in tests and examples. It uses environment variables to locate WoW installations, falling back to common installation paths when environment variables are not set.

## Supported WoW Versions

- **Classic Era** (`WOW_CLASSIC_ERA_DATA`) - World of Warcraft Classic Era (1.15.x)
- **Classic** (`WOW_CLASSIC_DATA`) - World of Warcraft Classic with expansions
- **Retail** (`WOW_RETAIL_DATA`) - Current World of Warcraft retail version

## Environment Variables

Set these environment variables to point to your WoW installation Data directories:

```bash
export WOW_CLASSIC_ERA_DATA="/path/to/wow/classic-era/Data"
export WOW_CLASSIC_DATA="/path/to/wow/classic/Data"
export WOW_RETAIL_DATA="/path/to/wow/retail/Data"
```

### Example Paths

Linux/macOS:
```bash
export WOW_CLASSIC_ERA_DATA="$HOME/Downloads/wow/1.15.2.55140.windows-win64/Data"
export WOW_CLASSIC_DATA="$HOME/wow/classic/Data"
export WOW_RETAIL_DATA="$HOME/wow/retail/Data"
```

Windows:
```cmd
set WOW_CLASSIC_ERA_DATA=C:\Games\World of Warcraft\classic-era\Data
set WOW_CLASSIC_DATA=C:\Games\World of Warcraft\classic\Data
set WOW_RETAIL_DATA=C:\Program Files\World of Warcraft\Data
```

## Usage in Tests

### Basic Usage

```rust
use test_utils::{find_wow_data, WowVersion};

#[test]
fn test_with_classic_era_data() {
    let data_path = match find_wow_data(WowVersion::ClassicEra) {
        Some(path) => path,
        None => {
            println!("Skipping test - no WoW Classic Era data found");
            return;
        }
    };
    
    // Use data_path for testing...
}
```

### Using Convenience Macros

```rust
use test_utils::{require_wow_data, skip_test_if_no_wow_data, WowVersion};

#[test]
fn test_with_required_data() {
    // This will return early if no Classic Era data is found
    let data_path = require_wow_data!(WowVersion::ClassicEra);
    
    // Test code here...
}

#[test] 
fn test_with_any_wow_data() {
    // Skip if no WoW data of any version is found
    skip_test_if_no_wow_data!();
    
    // Test code here...
}
```

### Finding Any Available Version

```rust
use test_utils::{find_any_wow_data};

#[test]
fn test_with_any_version() {
    let (version, data_path) = match find_any_wow_data() {
        Some(found) => found,
        None => {
            println!("No WoW data found");
            return;
        }
    };
    
    println!("Using {} data from {}", version.display_name(), data_path.display());
    // Test code here...
}
```

## Usage in Examples

```rust
use test_utils::{find_any_wow_data, print_setup_instructions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (version, data_path) = match find_any_wow_data() {
        Some(found) => found,
        None => {
            println!("No WoW installation found.");
            print_setup_instructions();
            return Ok(());
        }
    };
    
    println!("Using {} data from {}", version.display_name(), data_path.display());
    
    // Example code here...
    Ok(())
}
```

## Valid Data Directory Structure

The Data directory should contain CASC (Content Addressable Storage Container) structure:

```
Data/
├── data/          # CASC archive files (required)
├── indices/       # CASC index files
├── config/        # CASC configuration files  
└── ...
```

At minimum, the `data/` subdirectory must exist. The presence of `indices/` or `config/` directories indicates a valid CASC installation.

## Automatic Fallback Paths

If environment variables are not set, the utility will check common installation paths:

### Linux/macOS
- `~/Downloads/wow/*/Data`
- `~/wow/*/Data`
- `/opt/wow/*/Data`
- `/usr/local/games/wow/*/Data`

### Windows
- `C:\Program Files\World of Warcraft\*\Data`
- `C:\Program Files (x86)\World of Warcraft\*\Data`
- `C:\Games\World of Warcraft\*\Data`

### macOS
- `/Applications/World of Warcraft/*/Data`
- `~/Applications/World of Warcraft/*/Data`

## Helper Functions

- `find_wow_data(version)` - Find data for specific WoW version
- `find_any_wow_data()` - Find any available WoW version  
- `is_valid_wow_data(path)` - Validate a WoW data directory
- `print_setup_instructions()` - Print setup help for users

## Integration with CI

Tests using these utilities will automatically skip when no WoW data is available, making them CI-friendly while still useful for local development with real game data.

The macros provide clear messaging about why tests are skipped and how to set up data for local testing.