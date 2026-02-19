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

/// Print a hex dump of `bytes`, capped at `max_bytes`.
pub fn hex_dump(bytes: &[u8], max_bytes: usize) {
    let len = bytes.len().min(max_bytes);
    for (i, chunk) in bytes[..len].chunks(16).enumerate() {
        let offset = i * 16;
        let hex = hex_str(chunk);
        let spaced: String = hex
            .as_bytes()
            .chunks(2)
            .map(|c| std::str::from_utf8(c).expect("valid utf8"))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = chunk
            .iter()
            .map(|b| {
                if b.is_ascii_graphic() || *b == b' ' {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("  {offset:08x}  {spaced:<48}  {ascii}");
    }
    if bytes.len() > max_bytes {
        println!("  ... ({} more bytes)", bytes.len() - max_bytes);
    }
}
