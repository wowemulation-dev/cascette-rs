//! Hardcoded encryption keys for WoW.
//!
//! These keys are publicly known and used for decrypting game files.
//! Keys are sourced from CascLib and the WoW community.

use std::collections::HashMap;

/// Create a map of hardcoded encryption keys.
pub fn hardcoded_keys() -> HashMap<u64, [u8; 16]> {
    let mut keys = HashMap::new();

    // Add some well-known WoW encryption keys
    // Format: (key_name_hash, key_bytes)

    // Battle for Azeroth keys
    keys.insert(
        0xFA505078126ACB3E,
        hex::decode("BDC51862ABED79B2DE48C8E7E66C6200")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    keys.insert(
        0xFF813F7D062AC0BC,
        hex::decode("AA0B5C77F088CCC2D39049BD267F066D")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    keys.insert(
        0xD1E9B5EDF9283668,
        hex::decode("8E4A2579894E38B4AB9058BA5C7328EE")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Shadowlands keys
    keys.insert(
        0xB76729641141CB34,
        hex::decode("9849D1AA7B1FD09819C5C66283A326EC")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    keys.insert(
        0xFFB9469FF16E6BF8,
        hex::decode("D514BD1909A9E5DC8703F4B8BB1DFD9A")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // The War Within / Recent keys
    keys.insert(
        0x0EBE36B5010DFD7F, // Correct key name from WoW.txt
        hex::decode("9A89CC7E3ACB29CF14C60BC13B1E4616")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Classic key
    keys.insert(
        0xDEE3A0521EFF6F03, // Correct key name from WoW.txt
        hex::decode("AD740CE3FFFF9231468126985708E1B9")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Note: Additional keys should be loaded from TACTKeys repository
    // These are just a few essential keys for initial bootstrap
    // Use `ngdp keys update` to download the full key database

    keys
}

/// Parse a key from hex string.
pub fn parse_key_hex(hex_str: &str) -> Result<[u8; 16], String> {
    let hex_str = hex_str.trim();
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid hex: {e}"))?;

    if bytes.len() != 16 {
        return Err(format!("key must be 16 bytes, got {}", bytes.len()));
    }

    Ok(bytes.try_into().unwrap())
}

/// Parse a key name to u64.
pub fn parse_key_name(name: &str) -> Result<u64, String> {
    let name = name.trim();

    // Try to parse as hex u64
    if name.starts_with("0x") || name.starts_with("0X") {
        u64::from_str_radix(&name[2..], 16).map_err(|e| format!("invalid hex key name: {e}"))
    } else if name.chars().all(|c| c.is_ascii_hexdigit()) && name.len() == 16 {
        u64::from_str_radix(name, 16).map_err(|e| format!("invalid hex key name: {e}"))
    } else {
        // Could add Jenkins hash support here for string key names
        Err("string key names not yet supported".to_string())
    }
}
