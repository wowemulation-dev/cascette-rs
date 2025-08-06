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
        hex::decode("8E00C6F405873583DF8C76C101E5C8E1")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Shadowlands keys
    keys.insert(
        0xB76729641141CB34,
        hex::decode("9B1F39EE592CA99CDAFC0DFBF4B984EB")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    
    keys.insert(
        0xFFB9469FF16E6BF8,
        hex::decode("D514BD1909DEE57CFFDBDEFEFC13C961")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Dragonflight keys
    keys.insert(
        0x4F0FE18E9FA1AC1A,
        hex::decode("89381C748F6531BBFCD97753D06CC3CD")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    
    keys.insert(
        0x7758B2CF1E4E3E1B,
        hex::decode("3DE60D37C664723595F27C5CDBF08BFA")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Classic keys
    keys.insert(
        0x759F30E0A119B853,
        hex::decode("AD740CE3FFFF9231468126985708E1B9")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // The War Within keys
    keys.insert(
        0xE77EF1FFCEE5FD8A,
        hex::decode("9A89CC7E3ACB29CF14C60BC13B1E4616")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    
    keys.insert(
        0xC5753773F23C22C1,
        hex::decode("E6D38EE3D8DB0F4A86D3F73AB3F0D47C")
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // Note: In production, this would include hundreds more keys
    // These are just examples for initial implementation
    
    keys
}

/// Parse a key from hex string.
pub fn parse_key_hex(hex_str: &str) -> Result<[u8; 16], String> {
    let hex_str = hex_str.trim();
    let bytes = hex::decode(hex_str)
        .map_err(|e| format!("invalid hex: {}", e))?;
    
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
        u64::from_str_radix(&name[2..], 16)
            .map_err(|e| format!("invalid hex key name: {}", e))
    } else if name.chars().all(|c| c.is_ascii_hexdigit()) && name.len() == 16 {
        u64::from_str_radix(name, 16)
            .map_err(|e| format!("invalid hex key name: {}", e))
    } else {
        // Could add Jenkins hash support here for string key names
        Err("string key names not yet supported".to_string())
    }
}