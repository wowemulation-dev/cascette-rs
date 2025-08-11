//! Test utilities for cascette-rs
//!
//! Provides utilities for discovering WoW installation data for tests and examples.

use std::path::{Path, PathBuf};

/// Represents different WoW versions with their associated data paths
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WowVersion {
    /// World of Warcraft Classic Era (1.15.x)
    ClassicEra,
    /// World of Warcraft Classic (Season of Discovery, etc.)
    Classic,
    /// World of Warcraft Retail (current)
    Retail,
}

impl WowVersion {
    /// Get the environment variable name for this version
    pub fn env_var(&self) -> &'static str {
        match self {
            WowVersion::ClassicEra => "WOW_CLASSIC_ERA_DATA",
            WowVersion::Classic => "WOW_CLASSIC_DATA",
            WowVersion::Retail => "WOW_RETAIL_DATA",
        }
    }

    /// Get a human-readable name for this version
    pub fn display_name(&self) -> &'static str {
        match self {
            WowVersion::ClassicEra => "World of Warcraft: Classic Era",
            WowVersion::Classic => "World of Warcraft: Classic",
            WowVersion::Retail => "World of Warcraft: Retail",
        }
    }

    /// Get typical version patterns for this WoW version
    pub fn version_patterns(&self) -> &'static [&'static str] {
        match self {
            WowVersion::ClassicEra => &["1.15.", "1.14.", "1.13."],
            WowVersion::Classic => &["1.15.", "1.14.", "3.4."],
            WowVersion::Retail => &["10.", "11.", "9."],
        }
    }
}

/// Attempts to locate WoW data for a specific version
pub fn find_wow_data(version: WowVersion) -> Option<PathBuf> {
    // Strategy 1: Check environment variable
    if let Ok(path) = std::env::var(version.env_var()) {
        let path = PathBuf::from(path);
        if is_valid_wow_data(&path) {
            return Some(path);
        }
    }

    // Strategy 2: Check common installation paths
    let common_paths = get_common_wow_paths(version);
    common_paths
        .into_iter()
        .find(|path| is_valid_wow_data(path))
}

/// Get common installation paths for a WoW version
fn get_common_wow_paths(version: WowVersion) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Common base directories
    let base_dirs = if cfg!(windows) {
        vec![
            "C:\\Program Files\\World of Warcraft",
            "C:\\Program Files (x86)\\World of Warcraft",
            "C:\\Games\\World of Warcraft",
        ]
    } else if cfg!(target_os = "macos") {
        vec![
            "/Applications/World of Warcraft",
            "~/Applications/World of Warcraft",
        ]
    } else {
        vec![
            "~/wow",
            "~/Downloads/wow",
            "/opt/wow",
            "/usr/local/games/wow",
            // User's home directory patterns
            "~/Downloads",
            "~/Games",
        ]
    };

    for base in base_dirs {
        let base = PathBuf::from(shellexpand::tilde(base).to_string());

        // Add version-specific subdirectories
        match version {
            WowVersion::ClassicEra => {
                // Classic Era patterns
                paths.push(base.join("wow_classic_era/Data"));
                paths.push(base.join("classic-era/Data"));
                paths.push(base.join("1.15.2.55140.windows-win64/Data"));
                paths.push(base.join("1.14.4.54070.windows-win64/Data"));
                paths.push(base.join("1.13.7.46902.windows-win64/Data"));

                // Generic patterns
                for pattern in version.version_patterns() {
                    if let Some(entry) = find_matching_directory(&base, pattern) {
                        paths.push(entry.join("Data"));
                    }
                }
            }
            WowVersion::Classic => {
                // Classic patterns
                paths.push(base.join("wow_classic/Data"));
                paths.push(base.join("classic/Data"));
                paths.push(base.join("season-of-discovery/Data"));

                // Version patterns
                for pattern in version.version_patterns() {
                    if let Some(entry) = find_matching_directory(&base, pattern) {
                        paths.push(entry.join("Data"));
                    }
                }
            }
            WowVersion::Retail => {
                // Retail patterns
                paths.push(base.join("retail/Data"));
                paths.push(base.join("wow-retail/Data"));
                paths.push(base.join("Data")); // Direct Data folder

                // Version patterns
                for pattern in version.version_patterns() {
                    if let Some(entry) = find_matching_directory(&base, pattern) {
                        paths.push(entry.join("Data"));
                    }
                }
            }
        }
    }

    paths
}

/// Find a directory matching a version pattern
fn find_matching_directory(base: &Path, pattern: &str) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.contains(pattern) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Check if a path contains valid WoW CASC data
pub fn is_valid_wow_data(path: &Path) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    // Check for CASC structure
    let has_indices = path.join("indices").exists();
    let has_data = path.join("data").exists();
    let has_config = path.join("config").exists();

    // At minimum, we need the data directory and some indices or config
    has_data && (has_indices || has_config)
}

/// Find any available WoW data directory from any version
pub fn find_any_wow_data() -> Option<(WowVersion, PathBuf)> {
    for &version in &[
        WowVersion::ClassicEra,
        WowVersion::Classic,
        WowVersion::Retail,
    ] {
        if let Some(path) = find_wow_data(version) {
            return Some((version, path));
        }
    }
    None
}

/// Print instructions for setting up WoW data paths
pub fn print_setup_instructions() {
    println!("WoW Data Setup Instructions:");
    println!("===========================");
    println!();
    println!("To run tests and examples that require WoW game files, set environment variables:");
    println!();

    for &version in &[
        WowVersion::ClassicEra,
        WowVersion::Classic,
        WowVersion::Retail,
    ] {
        println!("  {} = /path/to/wow/Data", version.env_var());
        println!("    For: {}", version.display_name());
        println!();
    }

    println!("Examples:");
    println!(
        "  export WOW_CLASSIC_ERA_DATA=\"$HOME/Downloads/wow/1.15.2.55140.windows-win64/Data\""
    );
    println!("  export WOW_CLASSIC_DATA=\"$HOME/Downloads/wow/classic/Data\"");
    println!("  export WOW_RETAIL_DATA=\"$HOME/Downloads/wow/retail/Data\"");
    println!();

    println!("The Data directory should contain:");
    println!("  - data/          (CASC archive files)");
    println!("  - indices/       (CASC index files) ");
    println!("  - config/        (CASC configuration files)");
    println!();

    println!("Alternatively, place WoW installations in common locations:");
    if cfg!(windows) {
        println!("  C:\\Program Files\\World of Warcraft\\Data");
        println!("  C:\\Games\\World of Warcraft\\wow_classic_era\\Data");
    } else {
        println!("  ~/Downloads/wow/*/Data");
        println!("  ~/wow/*/Data");
        println!("  /opt/wow/*/Data");
    }
}

/// Skip a test if no WoW data is available, with helpful message
#[macro_export]
macro_rules! skip_test_if_no_wow_data {
    () => {
        if $crate::find_any_wow_data().is_none() {
            println!("Skipping test - no WoW data found");
            $crate::print_setup_instructions();
            return;
        }
    };
    ($version:expr) => {
        if $crate::find_wow_data($version).is_none() {
            println!("Skipping test - no {} data found", $version.display_name());
            $crate::print_setup_instructions();
            return;
        }
    };
}

/// Get a WoW data path or skip test with helpful message
#[macro_export]
macro_rules! require_wow_data {
    () => {
        match $crate::find_any_wow_data() {
            Some((_, path)) => path,
            None => {
                println!("Skipping test - no WoW data found");
                $crate::print_setup_instructions();
                return;
            }
        }
    };
    ($version:expr) => {
        match $crate::find_wow_data($version) {
            Some(path) => path,
            None => {
                println!("Skipping test - no {} data found", $version.display_name());
                $crate::print_setup_instructions();
                return;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_properties() {
        let classic_era = WowVersion::ClassicEra;
        assert_eq!(classic_era.env_var(), "WOW_CLASSIC_ERA_DATA");
        assert!(classic_era.display_name().contains("Classic Era"));
        assert!(classic_era.version_patterns().contains(&"1.15."));
    }

    #[test]
    fn test_path_validation() {
        // Invalid path
        assert!(!is_valid_wow_data(&PathBuf::from("/nonexistent/path")));

        // Test with temporary directory structure
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path();

        // Empty directory is not valid
        assert!(!is_valid_wow_data(temp_path));

        // Directory with data subdirectory and config is valid
        std::fs::create_dir(temp_path.join("data")).unwrap();
        std::fs::create_dir(temp_path.join("config")).unwrap();
        assert!(is_valid_wow_data(temp_path));
    }
}
