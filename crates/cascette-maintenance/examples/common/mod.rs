#![allow(dead_code)]

use std::fmt::Write;
use std::path::PathBuf;

/// Read the WoW installation path from `CASCETTE_WOW_PATH`.
///
/// Panics with an informative message if the variable is unset or empty.
pub fn wow_path() -> PathBuf {
    let path = std::env::var("CASCETTE_WOW_PATH").expect(
        "CASCETTE_WOW_PATH environment variable not set.\n\
         Set it to your WoW installation root, e.g.:\n\
         export CASCETTE_WOW_PATH=\"/path/to/World of Warcraft\"",
    );
    assert!(!path.is_empty(), "CASCETTE_WOW_PATH is empty");
    let p = PathBuf::from(&path);
    assert!(p.exists(), "CASCETTE_WOW_PATH does not exist: {path}");
    p
}

/// Return `<wow_path>/Data/data`.
pub fn data_path() -> PathBuf {
    wow_path().join("Data").join("data")
}

/// Hex-encode bytes to a lowercase string.
pub fn hex_str(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
