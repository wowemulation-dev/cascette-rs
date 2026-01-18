//! Entry structures for encoding file

use binrw::{BinRead, BinResult, BinWrite};
use cascette_crypto::{ContentKey, EncodingKey};
use std::io::{Read, Seek, Write};

/// Content key page entry
#[derive(Debug, Clone)]
pub struct CKeyPageEntry {
    /// Number of encoding keys for this content
    pub key_count: u8,
    /// File size (40-bit: 8-bit high + 32-bit low)
    pub file_size: u64,
    /// Content key
    pub content_key: ContentKey,
    /// Encoding keys
    pub encoding_keys: Vec<EncodingKey>,
}

impl BinRead for CKeyPageEntry {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read key count
        let key_count = u8::read_options(reader, endian, ())?;

        // Check for padding - zero indicates padding or end of page
        if key_count == 0x00 {
            // This is padding, not a real entry
            return Err(binrw::Error::Custom {
                pos: 0, // We don't need the exact position for padding
                err: Box::new(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Padding detected",
                )),
            });
        }

        // Read file size (40-bit: 1 byte high + 4 bytes low)
        // Note: CASC format is big-endian
        let file_size_high = u8::read_options(reader, binrw::Endian::Big, ())?;
        let file_size_low = u32::read_options(reader, binrw::Endian::Big, ())?;
        let file_size = (u64::from(file_size_high) << 32) | u64::from(file_size_low);

        // Read content key (16 bytes)
        let mut ckey_bytes = [0u8; 16];
        reader.read_exact(&mut ckey_bytes)?;
        let content_key = ContentKey::from_bytes(ckey_bytes);

        // Read encoding keys
        let mut encoding_keys = Vec::with_capacity(key_count as usize);
        for _ in 0..key_count {
            let mut ekey_bytes = [0u8; 16];
            reader.read_exact(&mut ekey_bytes)?;
            encoding_keys.push(EncodingKey::from_bytes(ekey_bytes));
        }

        Ok(Self {
            key_count,
            file_size,
            content_key,
            encoding_keys,
        })
    }
}

impl BinWrite for CKeyPageEntry {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write key count
        self.key_count.write_options(writer, endian, ())?;

        // Write file size (40-bit) - CASC format is big-endian
        let file_size_high = ((self.file_size >> 32) & 0xFF) as u8;
        let file_size_low = (self.file_size & 0xFFFF_FFFF) as u32;
        file_size_high.write_options(writer, binrw::Endian::Big, ())?;
        file_size_low.write_options(writer, binrw::Endian::Big, ())?;

        // Write content key
        writer.write_all(self.content_key.as_bytes())?;

        // Write encoding keys
        for ekey in &self.encoding_keys {
            writer.write_all(ekey.as_bytes())?;
        }

        Ok(())
    }
}

/// Encoding key page entry
#[derive(Debug, Clone)]
pub struct EKeyPageEntry {
    /// Encoding key
    pub encoding_key: EncodingKey,
    /// Index into `ESpec` table
    pub espec_index: u32,
    /// File size (40-bit: 8-bit high + 32-bit low)
    pub file_size: u64,
}

impl BinRead for EKeyPageEntry {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Read encoding key (16 bytes)
        let mut ekey_bytes = [0u8; 16];
        reader.read_exact(&mut ekey_bytes)?;

        // Check for padding - all zero bytes indicate padding
        if ekey_bytes.iter().all(|&b| b == 0x00) {
            return Err(binrw::Error::Custom {
                pos: 0, // We don't need the exact position for padding
                err: Box::new(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Padding detected",
                )),
            });
        }

        let encoding_key = EncodingKey::from_bytes(ekey_bytes);

        // Read ESpec index
        let espec_index = u32::read_options(reader, binrw::Endian::Big, ())?;

        // Read file size (40-bit)
        let file_size_high = u8::read_options(reader, binrw::Endian::Big, ())?;
        let file_size_low = u32::read_options(reader, binrw::Endian::Big, ())?;
        let file_size = (u64::from(file_size_high) << 32) | u64::from(file_size_low);

        Ok(Self {
            encoding_key,
            espec_index,
            file_size,
        })
    }
}

impl BinWrite for EKeyPageEntry {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        // Write encoding key
        writer.write_all(self.encoding_key.as_bytes())?;

        // Write ESpec index
        self.espec_index
            .write_options(writer, binrw::Endian::Big, ())?;

        // Write file size (40-bit)
        let file_size_high = ((self.file_size >> 32) & 0xFF) as u8;
        let file_size_low = (self.file_size & 0xFFFF_FFFF) as u32;
        file_size_high.write_options(writer, binrw::Endian::Big, ())?;
        file_size_low.write_options(writer, binrw::Endian::Big, ())?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;
        use proptest::test_runner::TestCaseError;
        use std::io::Cursor;

        /// Generate arbitrary content keys
        #[allow(dead_code)]
        fn content_key() -> impl Strategy<Value = ContentKey> {
            prop::array::uniform16(0u8..255).prop_map(ContentKey::from_bytes)
        }

        /// Generate arbitrary encoding keys
        #[allow(dead_code)]
        fn encoding_key() -> impl Strategy<Value = EncodingKey> {
            prop::array::uniform16(0u8..255).prop_map(EncodingKey::from_bytes)
        }

        /// Generate arbitrary CKey page entries
        #[allow(dead_code)]
        fn ckey_page_entry() -> impl Strategy<Value = CKeyPageEntry> {
            (
                1u8..=255u8,        // key_count (must be non-zero for valid entries)
                0u64..(1u64 << 40), // file_size (40-bit limit)
                content_key(),
                prop::collection::vec(encoding_key(), 1..=255), // encoding_keys
            )
                .prop_map(|(key_count, file_size, content_key, encoding_keys)| {
                    // Adjust encoding_keys length to match key_count
                    let mut adjusted_keys = encoding_keys;
                    adjusted_keys.truncate(key_count as usize);
                    while adjusted_keys.len() < key_count as usize {
                        adjusted_keys.push(EncodingKey::from_bytes([0u8; 16]));
                    }

                    CKeyPageEntry {
                        key_count,
                        file_size,
                        content_key,
                        encoding_keys: adjusted_keys,
                    }
                })
        }

        /// Generate arbitrary EKey page entries
        #[allow(dead_code)]
        fn ekey_page_entry() -> impl Strategy<Value = EKeyPageEntry> {
            (
                encoding_key(),
                0u32..u32::MAX,     // espec_index
                0u64..(1u64 << 40), // file_size (40-bit limit)
            )
                .prop_map(|(encoding_key, espec_index, file_size)| EKeyPageEntry {
                    encoding_key,
                    espec_index,
                    file_size,
                })
        }

        proptest! {
            /// Test that CKey entries round-trip correctly
            fn ckey_entry_round_trip(entry in ckey_page_entry()) {
                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                let mut cursor = Cursor::new(&buffer);
                let parsed = CKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.key_count, entry.key_count);
                prop_assert_eq!(parsed.file_size, entry.file_size);
                prop_assert_eq!(parsed.content_key.as_bytes(), entry.content_key.as_bytes());
                prop_assert_eq!(parsed.encoding_keys.len(), entry.encoding_keys.len());

                for (parsed_key, original_key) in parsed.encoding_keys.iter().zip(&entry.encoding_keys) {
                    prop_assert_eq!(parsed_key.as_bytes(), original_key.as_bytes());
                }
            }
        }

        proptest! {
            /// Test that EKey entries round-trip correctly
            fn ekey_entry_round_trip(entry in ekey_page_entry()) {
                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                let mut cursor = Cursor::new(&buffer);
                let parsed = EKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.encoding_key.as_bytes(), entry.encoding_key.as_bytes());
                prop_assert_eq!(parsed.espec_index, entry.espec_index);
                prop_assert_eq!(parsed.file_size, entry.file_size);
            }
        }

        proptest! {
            /// Test that 40-bit file sizes are handled correctly
            fn file_size_40_bit_handling(
                file_size in 0u64..(1u64 << 40) // Test full 40-bit range
            ) {
                let entry = CKeyPageEntry {
                    key_count: 1,
                    file_size,
                    content_key: ContentKey::from_bytes([0u8; 16]),
                    encoding_keys: vec![EncodingKey::from_bytes([0u8; 16])],
                };

                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                let mut cursor = Cursor::new(&buffer);
                let parsed = CKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.file_size, file_size);

                // Verify the high byte is properly encoded/decoded
                let expected_high = ((file_size >> 32) & 0xFF) as u8;
                let expected_low = (file_size & 0xFFFF_FFFF) as u32;

                prop_assert_eq!((parsed.file_size >> 32) as u8, expected_high);
                prop_assert_eq!((parsed.file_size & 0xFFFF_FFFF) as u32, expected_low);
            }
        }

        proptest! {
            /// Test that key count matches encoding keys length
            fn key_count_matches_keys_length(
                key_count in 1u8..=255u8,
                encoding_keys in prop::collection::vec(encoding_key(), 1..=255)
            ) {
                // Create entry where key_count might not match encoding_keys length
                let mut entry = CKeyPageEntry {
                    key_count,
                    file_size: 1000,
                    content_key: ContentKey::from_bytes([1u8; 16]),
                    encoding_keys,
                };

                // Adjust to match key_count (simulating what should happen in real usage)
                entry.encoding_keys.truncate(key_count as usize);
                while entry.encoding_keys.len() < key_count as usize {
                    entry.encoding_keys.push(EncodingKey::from_bytes([0u8; 16]));
                }

                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                let mut cursor = Cursor::new(&buffer);
                let parsed = CKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.encoding_keys.len(), key_count as usize);
                prop_assert_eq!(parsed.key_count, key_count);
            }
        }

        /// Test that padding (zero key count) is properly rejected
        #[test]
        fn padding_entries_rejected() {
            // Create data that looks like padding (starts with zero)
            let padding_data = vec![0u8; 50];
            let mut cursor = Cursor::new(&padding_data);

            let result = CKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ());

            // Should fail because key_count is 0 (padding)
            assert!(result.is_err());
        }

        proptest! {
            /// Test that very large file sizes (beyond 40-bit) are handled
            fn large_file_sizes_clamped(
                large_size in (1u64 << 40)..u64::MAX
            ) {
                // When creating entries with sizes larger than 40-bit,
                // the high bits should be truncated during encoding
                let entry = CKeyPageEntry {
                    key_count: 1,
                    file_size: large_size,
                    content_key: ContentKey::from_bytes([0u8; 16]),
                    encoding_keys: vec![EncodingKey::from_bytes([0u8; 16])],
                };

                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                let mut cursor = Cursor::new(&buffer);
                let parsed = CKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // The parsed size should be the original size truncated to 40 bits
                let expected_truncated = large_size & ((1u64 << 40) - 1);
                prop_assert_eq!(parsed.file_size, expected_truncated);
            }
        }

        proptest! {
            /// Test that different key arrays produce different serializations
            fn different_keys_different_serialization(
                keys1 in prop::collection::vec(encoding_key(), 1..10),
                keys2 in prop::collection::vec(encoding_key(), 1..10)
            ) {
                prop_assume!(keys1 != keys2); // Only test when keys are actually different

                let entry1 = CKeyPageEntry {
                    key_count: keys1.len() as u8,
                    file_size: 1000,
                    content_key: ContentKey::from_bytes([1u8; 16]),
                    encoding_keys: keys1,
                };

                let entry2 = CKeyPageEntry {
                    key_count: keys2.len() as u8,
                    file_size: 1000,
                    content_key: ContentKey::from_bytes([1u8; 16]),
                    encoding_keys: keys2,
                };

                let mut buffer1 = Vec::new();
                let mut buffer2 = Vec::new();

                {
                    let mut cursor1 = Cursor::new(&mut buffer1);
                    let mut cursor2 = Cursor::new(&mut buffer2);
                    entry1.write_options(&mut cursor1, binrw::Endian::Big, ())?;
                    entry2.write_options(&mut cursor2, binrw::Endian::Big, ())?;
                }

                // Different keys should produce different serializations
                prop_assert_ne!(buffer1, buffer2);
            }
        }

        proptest! {
            /// Test EKey entry size calculations are correct
            fn ekey_entry_size_correct(entry in ekey_page_entry()) {
                let mut buffer = Vec::new();
                {
                    let mut cursor = Cursor::new(&mut buffer);
                    entry.write_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                // EKey entries should be exactly: 16 (encoding_key) + 4 (compressed_size) + 16 (size_key_spec) = 36 bytes
                prop_assert_eq!(buffer.len(), 36);

                // Verify we can parse it back
                let mut cursor = Cursor::new(&buffer);
                let _parsed = EKeyPageEntry::read_options(&mut cursor, binrw::Endian::Big, ()).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(cursor.position(), 36); // Should have consumed all bytes
            }
        }
    }
}
