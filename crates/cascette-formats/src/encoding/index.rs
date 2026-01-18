//! Index structures for encoding file

use binrw::{BinRead, BinWrite};
use cascette_crypto::{ContentKey, EncodingKey};

/// Index entry pointing to a page
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
pub struct IndexEntry {
    /// First key in the page (for binary search)
    pub first_key: [u8; 16],
    /// MD5 checksum of the page data
    pub checksum: [u8; 16],
}

impl IndexEntry {
    /// Create a new index entry
    pub fn new(first_key: [u8; 16], checksum: [u8; 16]) -> Self {
        Self {
            first_key,
            checksum,
        }
    }

    /// Get first key as `ContentKey`
    pub fn first_content_key(&self) -> ContentKey {
        ContentKey::from_bytes(self.first_key)
    }

    /// Get first key as `EncodingKey`
    pub fn first_encoding_key(&self) -> EncodingKey {
        EncodingKey::from_bytes(self.first_key)
    }

    /// Verify page data against checksum
    pub fn verify(&self, page_data: &[u8]) -> bool {
        let digest = md5::compute(page_data);
        digest.as_ref() == self.checksum
    }
}
