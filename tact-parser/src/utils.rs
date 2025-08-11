//! Utility functions for binary operations used in TACT file formats

use crate::jenkins3::hashlittle2;
use crate::{Error, Result};

/// Perform a [`HashPath`][0] with [`hashlittle2`][] (aka: jenkins3).
///
/// This normalises `path` using the same rules as [`SStrHash`][1], and then
/// merges the two `u32`s of [`hashlittle2`][] into a `u64`, with `pc` as the
/// high bytes.
///
/// [0]: https://wowdev.wiki/TACT#hashpath
/// [1]: https://wowdev.wiki/SStrHash
pub fn jenkins3_hashpath(path: &str) -> u64 {
    let normalised = path.to_ascii_uppercase().replace('/', "\\");
    let mut pc = 0;
    let mut pb = 0;
    hashlittle2(normalised.as_bytes(), &mut pc, &mut pb);

    (u64::from(pc) << 32) | u64::from(pb)
}

/// Read a 40-bit (5-byte) unsigned integer from a byte slice (little-endian)
///
/// 40-bit integers are used throughout TACT formats for file sizes and offsets.
/// They allow representing values up to 1TB while saving space compared to 64-bit.
///
/// # Arguments
/// * `data` - Byte slice containing at least 5 bytes
///
/// # Returns
/// * The 40-bit value as a u64
///
/// # Errors
/// * Returns error if data contains less than 5 bytes
///
/// # Example
/// ```
/// use tact_parser::utils::read_uint40;
///
/// let data = [0x12, 0x34, 0x56, 0x78, 0x9A];
/// let value = read_uint40(&data).unwrap();
/// assert_eq!(value, 0x9A78563412);
/// ```
pub fn read_uint40(data: &[u8]) -> Result<u64> {
    if data.len() < 5 {
        return Err(Error::IOError(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("Need 5 bytes for uint40, got {}", data.len()),
        )));
    }

    Ok((data[0] as u64)
        | ((data[1] as u64) << 8)
        | ((data[2] as u64) << 16)
        | ((data[3] as u64) << 24)
        | ((data[4] as u64) << 32))
}

/// Write a 40-bit (5-byte) unsigned integer to a byte array (little-endian)
///
/// # Arguments
/// * `value` - The value to write (must fit in 40 bits)
///
/// # Returns
/// * A 5-byte array containing the value in little-endian format
///
/// # Panics
/// * Panics if value exceeds 40-bit range (>= 2^40)
///
/// # Example
/// ```
/// use tact_parser::utils::write_uint40;
///
/// let bytes = write_uint40(0x9A78563412);
/// assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78, 0x9A]);
/// ```
pub fn write_uint40(value: u64) -> [u8; 5] {
    assert!(
        value < (1u64 << 40),
        "Value {value:#x} exceeds 40-bit range"
    );

    [
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
        ((value >> 24) & 0xFF) as u8,
        ((value >> 32) & 0xFF) as u8,
    ]
}

/// Read a 40-bit unsigned integer from a cursor (little-endian)
///
/// This is a convenience function for use with `std::io::Cursor` or `BufReader`.
///
/// # Arguments
/// * `reader` - A reader implementing `std::io::Read`
///
/// # Returns
/// * The 40-bit value as a u64
///
/// # Errors
/// * Returns error if unable to read 5 bytes
pub fn read_uint40_from<R: std::io::Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 5];
    reader.read_exact(&mut buf)?;
    read_uint40(&buf)
}

/// Read a variable-length integer from a byte slice
///
/// Variable-length integers use 7 bits per byte with a continuation bit.
/// This is compatible with protobuf/varint encoding.
///
/// # Arguments
/// * `data` - Byte slice to read from
///
/// # Returns
/// * Tuple of (value, bytes_consumed)
///
/// # Errors
/// * Returns error if varint is malformed or exceeds 5 bytes
///
/// # Example
/// ```
/// use tact_parser::utils::read_varint;
///
/// let data = [0x08]; // Value 8
/// let (value, consumed) = read_varint(&data).unwrap();
/// assert_eq!(value, 8);
/// assert_eq!(consumed, 1);
///
/// let data = [0x96, 0x01]; // Value 150
/// let (value, consumed) = read_varint(&data).unwrap();
/// assert_eq!(value, 150);
/// assert_eq!(consumed, 2);
/// ```
pub fn read_varint(data: &[u8]) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut consumed = 0;

    for &byte in data {
        consumed += 1;

        // Extract 7-bit value
        let value = (byte & 0x7F) as u32;

        // Check for overflow
        if shift >= 32 || (shift == 28 && value > 0x0F) {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Varint exceeds 32-bit range",
            )));
        }

        result |= value << shift;

        // Check continuation bit
        if byte & 0x80 == 0 {
            return Ok((result, consumed));
        }

        shift += 7;

        // Varints shouldn't exceed 5 bytes for 32-bit values
        if consumed >= 5 {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Varint exceeds maximum length",
            )));
        }
    }

    Err(Error::IOError(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "Incomplete varint",
    )))
}

/// Write a variable-length integer to a byte vector
///
/// # Arguments
/// * `value` - The value to encode
///
/// # Returns
/// * A vector containing the encoded varint
///
/// # Example
/// ```
/// use tact_parser::utils::write_varint;
///
/// let encoded = write_varint(8);
/// assert_eq!(encoded, vec![0x08]);
///
/// let encoded = write_varint(150);
/// assert_eq!(encoded, vec![0x96, 0x01]);
/// ```
pub fn write_varint(mut value: u32) -> Vec<u8> {
    let mut result = Vec::new();

    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80; // Set continuation bit
            result.push(byte);
        } else {
            result.push(byte);
            break;
        }
    }

    result
}

/// Read a null-terminated C string from a byte slice
///
/// # Arguments
/// * `data` - Byte slice to read from
///
/// # Returns
/// * Tuple of (string, bytes_consumed)
///
/// # Errors
/// * Returns error if no null terminator found or invalid UTF-8
pub fn read_cstring(data: &[u8]) -> Result<(String, usize)> {
    // Find null terminator
    let null_pos = data.iter().position(|&b| b == 0).ok_or_else(|| {
        Error::IOError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No null terminator found in C string",
        ))
    })?;

    // Convert to string
    let string = std::str::from_utf8(&data[..null_pos])
        .map_err(|e| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in C string: {e}"),
            ))
        })?
        .to_string();

    Ok((string, null_pos + 1)) // +1 for null terminator
}

/// Read a 40-bit (5-byte) unsigned integer from a byte slice (big-endian)
///
/// 40-bit integers are used throughout TACT formats for file sizes and offsets.
/// They allow representing values up to 1TB while saving space compared to 64-bit.
///
/// # Arguments
/// * `data` - Byte slice containing at least 5 bytes
///
/// # Returns
/// * The 40-bit value as a u64
///
/// # Errors
/// * Returns error if data contains less than 5 bytes
///
/// # Example
/// ```
/// use tact_parser::utils::read_uint40_be;
///
/// let data = [0x01, 0x00, 0x00, 0x00, 0x00]; // 4GB file
/// let value = read_uint40_be(&data).unwrap();
/// assert_eq!(value, 0x100000000);
/// ```
pub fn read_uint40_be(data: &[u8]) -> Result<u64> {
    if data.len() < 5 {
        return Err(Error::IOError(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("Need 5 bytes for uint40, got {}", data.len()),
        )));
    }

    // TACT encoding format: 1 byte for high bits (32-39) + 4 bytes big-endian u32 (0-31)
    let high_byte = data[0] as u64;
    let low_u32 = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as u64;

    Ok((high_byte << 32) | low_u32)
}

/// Write a 40-bit (5-byte) unsigned integer to a byte array (big-endian)
///
/// # Arguments
/// * `value` - The value to write (must fit in 40 bits)
///
/// # Returns
/// * A 5-byte array containing the value in big-endian format
///
/// # Panics
/// * Panics if value exceeds 40-bit range (>= 2^40)
///
/// # Example
/// ```
/// use tact_parser::utils::write_uint40_be;
///
/// let bytes = write_uint40_be(0x100000000); // 4GB
/// assert_eq!(bytes, [0x01, 0x00, 0x00, 0x00, 0x00]);
/// ```
pub fn write_uint40_be(value: u64) -> [u8; 5] {
    assert!(
        value < (1u64 << 40),
        "Value {value:#x} exceeds 40-bit range"
    );

    // TACT encoding format: 1 byte for high bits (32-39) + 4 bytes big-endian u32 (0-31)
    let high_byte = ((value >> 32) & 0xFF) as u8;
    let low_u32 = (value & 0xFFFFFFFF) as u32;
    let low_bytes = low_u32.to_be_bytes();

    [
        high_byte,
        low_bytes[0],
        low_bytes[1],
        low_bytes[2],
        low_bytes[3],
    ]
}

/// Read a 40-bit unsigned integer from a cursor (big-endian)
///
/// This is a convenience function for use with `std::io::Cursor` or `BufReader`.
///
/// # Arguments
/// * `reader` - A reader implementing `std::io::Read`
///
/// # Returns
/// * The 40-bit value as a u64
///
/// # Errors
/// * Returns error if unable to read 5 bytes
pub fn read_uint40_be_from<R: std::io::Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 5];
    reader.read_exact(&mut buf)?;
    read_uint40_be(&buf)
}

/// Read a C string from a reader
///
/// # Arguments
/// * `reader` - A reader implementing `std::io::Read`
///
/// # Returns
/// * The string without null terminator
///
/// # Errors
/// * Returns error if unable to read or invalid UTF-8
pub fn read_cstring_from<R: std::io::Read>(reader: &mut R) -> Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        reader.read_exact(&mut byte)?;
        if byte[0] == 0 {
            break;
        }
        bytes.push(byte[0]);
    }

    String::from_utf8(bytes).map_err(|e| {
        Error::IOError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid UTF-8 in C string: {e}"),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uint40_roundtrip() {
        let test_values = [
            0u64,
            1,
            255,
            256,
            65535,
            65536,
            0xFFFFFFFF,
            0x123456789A,
            0xFFFFFFFFFF, // Max 40-bit value
        ];

        for value in test_values {
            let bytes = write_uint40(value);
            let decoded = read_uint40(&bytes).unwrap();
            assert_eq!(value, decoded, "Failed for value {value:#x}");
        }
    }

    #[test]
    fn test_uint40_little_endian() {
        let data = [0x12, 0x34, 0x56, 0x78, 0x9A];
        let value = read_uint40(&data).unwrap();
        assert_eq!(value, 0x9A78563412);

        let bytes = write_uint40(0x9A78563412);
        assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78, 0x9A]);
    }

    #[test]
    #[should_panic(expected = "exceeds 40-bit range")]
    fn test_uint40_overflow() {
        write_uint40(0x10000000000); // 2^40
    }

    #[test]
    fn test_uint40_insufficient_data() {
        let data = [0x12, 0x34, 0x56, 0x78]; // Only 4 bytes
        assert!(read_uint40(&data).is_err());
    }

    #[test]
    fn test_varint_single_byte() {
        let data = [0x08];
        let (value, consumed) = read_varint(&data).unwrap();
        assert_eq!(value, 8);
        assert_eq!(consumed, 1);

        let encoded = write_varint(8);
        assert_eq!(encoded, vec![0x08]);
    }

    #[test]
    fn test_varint_multi_byte() {
        let data = [0x96, 0x01]; // 150 = 0x96
        let (value, consumed) = read_varint(&data).unwrap();
        assert_eq!(value, 150);
        assert_eq!(consumed, 2);

        let encoded = write_varint(150);
        assert_eq!(encoded, vec![0x96, 0x01]);
    }

    #[test]
    fn test_varint_max_value() {
        let value = 0xFFFFFFFF;
        let encoded = write_varint(value);
        let (decoded, _) = read_varint(&encoded).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_varint_known_values() {
        // Test cases from protobuf spec
        let test_cases = [
            (0, vec![0x00]),
            (1, vec![0x01]),
            (127, vec![0x7F]),
            (128, vec![0x80, 0x01]),
            (300, vec![0xAC, 0x02]),
            (16384, vec![0x80, 0x80, 0x01]),
        ];

        for (value, expected) in test_cases {
            let encoded = write_varint(value);
            assert_eq!(encoded, expected, "Encoding failed for {value}");

            let (decoded, consumed) = read_varint(&expected).unwrap();
            assert_eq!(decoded, value, "Decoding failed for {value}");
            assert_eq!(consumed, expected.len());
        }
    }

    #[test]
    fn test_cstring() {
        let data = b"Hello, World!\0extra data";
        let (string, consumed) = read_cstring(data).unwrap();
        assert_eq!(string, "Hello, World!");
        assert_eq!(consumed, 14); // Including null terminator
    }

    #[test]
    fn test_cstring_empty() {
        let data = b"\0";
        let (string, consumed) = read_cstring(data).unwrap();
        assert_eq!(string, "");
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_cstring_no_terminator() {
        let data = b"No null here";
        assert!(read_cstring(data).is_err());
    }

    #[test]
    fn test_uint40_big_endian() {
        // Test TACT encoding format: 1 high byte + 4 bytes big-endian u32
        // Example: 4GB file (0x100000000)
        let data = [0x01, 0x00, 0x00, 0x00, 0x00];
        let value = read_uint40_be(&data).unwrap();
        assert_eq!(value, 0x100000000); // 4GB

        // Test a more complex value: 0x0A << 32 | 0x12345678
        let data = [0x0A, 0x12, 0x34, 0x56, 0x78];
        let value = read_uint40_be(&data).unwrap();
        assert_eq!(value, 0x0A12345678);

        // Test round-trip
        let original = 0x0A12345678u64;
        let bytes = write_uint40_be(original);
        let restored = read_uint40_be(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_uint40_be_from_reader() {
        use std::io::Cursor;

        let data = [0x01, 0x00, 0x00, 0x00, 0x00]; // 4GB
        let mut cursor = Cursor::new(&data);
        let value = read_uint40_be_from(&mut cursor).unwrap();
        assert_eq!(value, 0x100000000);
    }
}
