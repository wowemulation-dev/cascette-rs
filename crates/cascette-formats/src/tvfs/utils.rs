//! Utility functions for TVFS parsing

use crate::tvfs::error::{TvfsError, TvfsResult};

/// Read a variable-length integer from data
/// TVFS uses LEB128-like encoding for variable integers
pub fn read_varint(data: &[u8], offset: &mut usize) -> TvfsResult<u32> {
    if *offset >= data.len() {
        return Err(TvfsError::VarIntError(*offset));
    }

    let mut result = 0u32;
    let mut shift = 0;

    loop {
        if *offset >= data.len() {
            return Err(TvfsError::VarIntError(*offset));
        }

        let byte = data[*offset];
        *offset += 1;

        // Take lower 7 bits
        let value = u32::from(byte & 0x7F);
        result |= value << shift;

        // If high bit is not set, we're done
        if (byte & 0x80) == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            return Err(TvfsError::VarIntError(*offset - 1));
        }
    }

    Ok(result)
}

/// Write a variable-length integer to data
pub fn write_varint(value: u32, data: &mut Vec<u8>) {
    let mut value = value;

    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80; // Set continuation bit
        }

        data.push(byte);

        if value == 0 {
            break;
        }
    }
}

/// Calculate the size needed to encode a variable-length integer
#[allow(dead_code)] // Public API function
pub fn varint_size(value: u32) -> usize {
    if value == 0 {
        1
    } else {
        (32 - value.leading_zeros()).div_ceil(7) as usize
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_round_trip() {
        let test_values = [
            0,
            1,
            127,
            128,
            255,
            256,
            16383,
            16384,
            0x001F_FFFF,
            0x0020_0000,
            0x0FFF_FFFF,
            0x1000_0000,
        ];

        for &value in &test_values {
            let mut data = Vec::new();
            write_varint(value, &mut data);

            let mut offset = 0;
            let parsed = read_varint(&data, &mut offset).expect("Operation should succeed");

            assert_eq!(parsed, value);
            assert_eq!(offset, data.len());
        }
    }

    #[test]
    fn test_varint_size() {
        assert_eq!(varint_size(0), 1);
        assert_eq!(varint_size(1), 1);
        assert_eq!(varint_size(127), 1);
        assert_eq!(varint_size(128), 2);
        assert_eq!(varint_size(255), 2);
        assert_eq!(varint_size(16383), 2);
        assert_eq!(varint_size(16384), 3);
    }
}
