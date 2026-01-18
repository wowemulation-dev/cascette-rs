//! Utility functions for Patch Archive format

use crate::patch_archive::error::PatchArchiveError;
use binrw::{
    BinResult,
    io::{Read, Seek},
};

/// Read a null-terminated string from binary data
pub fn read_null_terminated_string<R: Read + Seek>(reader: &mut R) -> BinResult<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 1];

    loop {
        reader.read_exact(&mut buffer)?;
        if buffer[0] == 0 {
            break;
        }
        bytes.push(buffer[0]);

        // Safety check to prevent infinite loops
        if bytes.len() > 1024 {
            return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(PatchArchiveError::StringTooLong),
            });
        }
    }

    String::from_utf8(bytes).map_err(|e| binrw::Error::Custom {
        pos: reader.stream_position().unwrap_or(0),
        err: Box::new(PatchArchiveError::InvalidString(e)),
    })
}

/// Read a variable-length unsigned integer (LEB128 format)
#[allow(dead_code)] // Public API function
pub fn read_varint<R: Read + Seek>(reader: &mut R) -> BinResult<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut buffer = [0u8; 1];

    loop {
        reader.read_exact(&mut buffer)?;
        let byte = buffer[0];

        // Extract 7 bits of data
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        // If high bit is not set, we're done
        if (byte & 0x80) == 0 {
            break;
        }

        // Safety check to prevent overflow
        if shift >= 64 {
            return Err(binrw::Error::Custom {
                pos: reader.stream_position().unwrap_or(0),
                err: Box::new(PatchArchiveError::InvalidEntry(
                    "varint too large".to_string(),
                )),
            });
        }
    }

    Ok(result)
}

/// Write a variable-length unsigned integer (LEB128 format)
#[allow(dead_code)] // Public API function
pub fn write_varint<W: std::io::Write>(writer: &mut W, mut value: u64) -> BinResult<()> {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80; // Set continuation bit
        }

        writer.write_all(&[byte])?;

        if value == 0 {
            break;
        }
    }

    Ok(())
}

/// Calculate encoded size of a varint
#[allow(dead_code)] // Public API function
pub fn varint_size(value: u64) -> usize {
    if value == 0 {
        return 1;
    }

    let mut size = 0;
    let mut temp = value;
    while temp > 0 {
        size += 1;
        temp >>= 7;
    }
    size
}

/// Decompress data using ZLib
#[allow(dead_code)] // Public API function
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, PatchArchiveError> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(PatchArchiveError::DecompressionError)?;

    Ok(decompressed)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_null_terminated_string() {
        let data = b"hello\x00world";
        let mut cursor = Cursor::new(data);

        let s = read_null_terminated_string(&mut cursor).expect("Operation should succeed");
        assert_eq!(s, "hello");

        // Cursor should be positioned after the null terminator
        let mut remaining = Vec::new();
        cursor
            .read_to_end(&mut remaining)
            .expect("Operation should succeed");
        assert_eq!(remaining, b"world");
    }

    #[test]
    fn test_varint_round_trip() {
        let test_values = [0, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX];

        for &value in &test_values {
            let mut buffer = Vec::new();
            write_varint(&mut buffer, value).expect("Operation should succeed");

            let parsed = read_varint(&mut Cursor::new(&buffer)).expect("Operation should succeed");
            assert_eq!(parsed, value, "Failed for value {}", value);
        }
    }

    #[test]
    fn test_varint_size() {
        assert_eq!(varint_size(0), 1);
        assert_eq!(varint_size(127), 1);
        assert_eq!(varint_size(128), 2);
        assert_eq!(varint_size(16383), 2);
        assert_eq!(varint_size(16384), 3);
    }
}
