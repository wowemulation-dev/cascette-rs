//! Patch Index header and block descriptor types
//!
//! The patch index header is at the start of the file (unlike CDN archive
//! indices which have a footer). Parsed by
//! `tact::PatchIndexReader::ParseHeader` at 0x6a4f0b.

use super::error::{PatchIndexError, PatchIndexResult};

/// Minimum header size in bytes before block descriptors
///
/// 12 bytes preamble (header_size + version + data_size) + 2 bytes
/// extra_header_length. The actual header may be larger depending on
/// extra header data and block descriptor count.
pub const MIN_HEADER_SIZE: usize = 14;

/// Patch Index file header
///
/// Layout (little-endian):
/// ```text
/// offset 0x00: u32 header_size       (total header bytes including descriptors)
/// offset 0x04: u32 version           (format version, currently 1)
/// offset 0x08: u32 data_size         (total file size)
/// offset 0x0C: u16 extra_header_len  (bytes of extra header data)
/// if extra_header_len > 0:
///   offset 0x0E: u8  key_size        (EKey size in bytes)
///   offset 0x0F: [u8; key_size]      (key data, stored in reader struct)
///   remaining: extra_header_len - key_size - 1 bytes (additional data)
/// u32 block_count
/// [BlockDescriptor; block_count]      (8 bytes each)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIndexHeader {
    /// Total header size in bytes (includes preamble, extra header, block
    /// descriptors). Block data starts at this offset.
    pub header_size: u32,

    /// Format version (1 in all known files)
    pub version: u32,

    /// Total file data size (should match actual file length)
    pub data_size: u32,

    /// Extra header key size (0 if no key present)
    pub key_size: u8,

    /// Extra header key data (up to 16 bytes, zero-padded)
    pub key_data: [u8; 16],

    /// Additional extra header bytes beyond key_size + 1
    pub extra_data: Vec<u8>,

    /// Block descriptors
    pub blocks: Vec<BlockDescriptor>,
}

/// Block descriptor in the patch index header
///
/// Each descriptor is 8 bytes: u32 block_type + u32 block_size.
/// The block offset is computed as header_size + sum of preceding block sizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDescriptor {
    /// Block type identifier (1-10)
    pub block_type: u32,

    /// Block data size in bytes
    pub block_size: u32,
}

impl PatchIndexHeader {
    /// Parse the header from raw data
    pub fn parse(data: &[u8]) -> PatchIndexResult<Self> {
        if data.len() < MIN_HEADER_SIZE {
            return Err(PatchIndexError::DataTooShort {
                actual: data.len(),
                minimum: MIN_HEADER_SIZE,
            });
        }

        let header_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let data_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let extra_header_len = u16::from_le_bytes([data[12], data[13]]);

        if version != 1 {
            return Err(PatchIndexError::UnsupportedVersion(version));
        }

        if data.len() < header_size as usize {
            return Err(PatchIndexError::TruncatedHeader {
                header_size,
                actual: data.len(),
            });
        }

        let mut pos = 14;

        // Parse extra header
        let mut key_size = 0u8;
        let mut key_data = [0u8; 16];
        let mut extra_data = Vec::new();

        if extra_header_len > 0 {
            if pos >= data.len() {
                return Err(PatchIndexError::TruncatedHeader {
                    header_size,
                    actual: data.len(),
                });
            }
            key_size = data[pos];
            pos += 1;

            let key_bytes = key_size.min(16) as usize;
            if pos + key_bytes > data.len() {
                return Err(PatchIndexError::TruncatedHeader {
                    header_size,
                    actual: data.len(),
                });
            }
            key_data[..key_bytes].copy_from_slice(&data[pos..pos + key_bytes]);
            pos += key_size as usize;

            // Remaining extra header bytes
            let consumed = key_size as u16 + 1;
            if extra_header_len > consumed {
                let remaining = (extra_header_len - consumed) as usize;
                if pos + remaining > data.len() {
                    return Err(PatchIndexError::TruncatedHeader {
                        header_size,
                        actual: data.len(),
                    });
                }
                extra_data = data[pos..pos + remaining].to_vec();
                pos += remaining;
            }
        }

        // Read block count
        if pos + 4 > data.len() {
            return Err(PatchIndexError::TruncatedHeader {
                header_size,
                actual: data.len(),
            });
        }
        let block_count =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        // Read block descriptors
        let mut blocks = Vec::with_capacity(block_count as usize);
        for _ in 0..block_count {
            if pos + 8 > data.len() {
                return Err(PatchIndexError::TruncatedHeader {
                    header_size,
                    actual: data.len(),
                });
            }
            let block_type =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            let block_size =
                u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
            blocks.push(BlockDescriptor {
                block_type,
                block_size,
            });
            pos += 8;
        }

        // Validate total size
        let total_block_size: u64 = blocks.iter().map(|b| u64::from(b.block_size)).sum();
        let expected = u64::from(header_size) + total_block_size;
        if (data.len() as u64) < expected {
            return Err(PatchIndexError::TruncatedData {
                expected: expected as usize,
                actual: data.len(),
            });
        }

        Ok(Self {
            header_size,
            version,
            data_size,
            key_size,
            key_data,
            extra_data,
            blocks,
        })
    }

    /// Build the header to bytes
    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.extend_from_slice(&self.header_size.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.data_size.to_le_bytes());

        // Extra header length
        let extra_len = if self.key_size > 0 || !self.extra_data.is_empty() {
            1 + self.key_size as u16 + self.extra_data.len() as u16
        } else {
            // Even with key_size=0, fixtures show extra_header_len=1
            1u16
        };
        out.extend_from_slice(&extra_len.to_le_bytes());

        // Extra header content
        if extra_len > 0 {
            out.push(self.key_size);
            if self.key_size > 0 {
                out.extend_from_slice(&self.key_data[..self.key_size as usize]);
            }
            out.extend_from_slice(&self.extra_data);
        }

        // Block count + descriptors
        out.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for block in &self.blocks {
            out.extend_from_slice(&block.block_type.to_le_bytes());
            out.extend_from_slice(&block.block_size.to_le_bytes());
        }

        out
    }

    /// Compute the absolute byte offset where a block's data starts
    pub fn block_offset(&self, block_index: usize) -> u64 {
        let mut offset = u64::from(self.header_size);
        for desc in &self.blocks[..block_index] {
            offset += u64::from(desc.block_size);
        }
        offset
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_header_round_trip() {
        let header = PatchIndexHeader {
            header_size: 43,
            version: 1,
            data_size: 1000,
            key_size: 0,
            key_data: [0; 16],
            extra_data: Vec::new(),
            blocks: vec![
                BlockDescriptor {
                    block_type: 1,
                    block_size: 7,
                },
                BlockDescriptor {
                    block_type: 2,
                    block_size: 100,
                },
            ],
        };

        let built = header.build();
        // Pad with enough data to satisfy total size validation
        let mut data = built.clone();
        data.resize(1000, 0);

        let reparsed = PatchIndexHeader::parse(&data).unwrap();
        assert_eq!(reparsed.header_size, 43);
        assert_eq!(reparsed.version, 1);
        assert_eq!(reparsed.blocks.len(), 2);
        assert_eq!(reparsed.blocks[0].block_type, 1);
        assert_eq!(reparsed.blocks[1].block_type, 2);
    }

    #[test]
    fn test_block_offset() {
        let header = PatchIndexHeader {
            header_size: 43,
            version: 1,
            data_size: 500,
            key_size: 0,
            key_data: [0; 16],
            extra_data: Vec::new(),
            blocks: vec![
                BlockDescriptor {
                    block_type: 1,
                    block_size: 7,
                },
                BlockDescriptor {
                    block_type: 2,
                    block_size: 100,
                },
                BlockDescriptor {
                    block_type: 8,
                    block_size: 200,
                },
            ],
        };

        assert_eq!(header.block_offset(0), 43);
        assert_eq!(header.block_offset(1), 50);
        assert_eq!(header.block_offset(2), 150);
    }

    #[test]
    fn test_reject_unsupported_version() {
        let mut data = vec![0u8; 100];
        // header_size=43, version=2
        data[0..4].copy_from_slice(&43u32.to_le_bytes());
        data[4..8].copy_from_slice(&2u32.to_le_bytes());
        data[8..12].copy_from_slice(&100u32.to_le_bytes());
        data[12..14].copy_from_slice(&0u16.to_le_bytes());

        let result = PatchIndexHeader::parse(&data);
        assert!(result.is_err());
    }
}
