//! Cross-platform game process detection via the sysinfo crate.
//!
//! Maps product codes to known executable names so the agent can detect
//! running games and prevent operations like uninstall while a game is active.

use sysinfo::System;

/// Known executable names for each product code.
///
/// Based on real agent behavior: each product has multiple possible
/// binary names across platforms and client variants (retail, beta, PTR).
#[must_use]
pub fn executable_names(product_code: &str) -> &'static [&'static str] {
    match product_code {
        "wow" => &[
            "Wow.exe",
            "WowB.exe",
            "WowT.exe",
            "Wow-64.exe",
            "WoW",        // macOS
            "WoW-x86_64", // Linux
        ],
        "wow_classic" => &[
            "WowClassic.exe",
            "WowClassicB.exe",
            "WowClassicT.exe",
            "WoWClassic",
            "WoWClassic-x86_64",
        ],
        "wow_classic_era" => &[
            "WowClassic.exe",
            "WowClassicB.exe",
            "WoWClassic",
            "WoWClassic-x86_64",
        ],
        _ => &[],
    }
}

/// Check if any game process for the given product is running.
#[must_use]
pub fn is_game_running(product_code: &str) -> bool {
    let names = executable_names(product_code);
    if names.is_empty() {
        return false;
    }

    let sys = System::new_all();
    for process in sys.processes().values() {
        let proc_name = process.name().to_string_lossy();
        for name in names {
            if proc_name == *name {
                return true;
            }
        }
    }
    false
}

/// Get PIDs of running game processes for a product.
#[must_use]
pub fn game_pids(product_code: &str) -> Vec<u32> {
    let names = executable_names(product_code);
    if names.is_empty() {
        return Vec::new();
    }

    let sys = System::new_all();
    let mut pids = Vec::new();
    for (pid, process) in sys.processes() {
        let proc_name = process.name().to_string_lossy();
        for name in names {
            if proc_name == *name {
                pids.push(pid.as_u32());
            }
        }
    }
    pids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executable_names() {
        let names = executable_names("wow");
        assert!(!names.is_empty());
        assert!(names.contains(&"Wow.exe"));
    }

    #[test]
    fn test_executable_names_unknown() {
        let names = executable_names("unknown_product");
        assert!(names.is_empty());
    }

    #[test]
    fn test_is_game_running_unknown_product() {
        // Unknown products never have running processes
        assert!(!is_game_running("nonexistent_game"));
    }

    #[test]
    fn test_game_pids_unknown_product() {
        assert!(game_pids("nonexistent_game").is_empty());
    }
}
