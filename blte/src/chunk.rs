//! BLTE chunk handling and file structure

#[cfg(feature = "async")]
use tokio::io::AsyncReadExt;

use crate::{Error, Result};
use byteorder::ReadBytesExt;
use std::io::Read;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct EncryptedChunkHeader {
    key_name: Vec<u8>,
    iv: Vec<u8>,
}

impl EncryptedChunkHeader {
    /// Parses an encrypted chunk header.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let key_name_length = f.read_u8()?;
        let mut key_name = vec![0; key_name_length as usize];
        f.read_exact(&mut key_name)?;

        let iv_length = f.read_u8()?;
        let mut iv = vec![0; iv_length as usize];
        f.read_exact(&mut iv)?;

        Ok(Self { key_name, iv })
    }

    #[cfg(feature = "async")]
    /// Parses an encrypted block header asynchronously.
    pub async fn aparse<R: AsyncReadExt + Unpin>(f: &mut R) -> Result<Self> {
        let key_name_length = f.read_u8().await?;
        let mut key_name = vec![0; key_name_length as usize];
        f.read_exact(&mut key_name).await?;

        let iv_length = f.read_u8().await?;
        let mut iv = vec![0; iv_length as usize];
        f.read_exact(&mut iv).await?;

        Ok(Self { key_name, iv })
    }

    /// Length of the [`EncryptedBlockHeader`] on disk, including length
    /// prefixes.
    pub fn len(&self) -> usize {
        self.key_name.len() + self.iv.len() + 2
    }

    /// `true` if the [`EncryptedBlockHeader`] would take up 0 bytes.
    ///
    /// This is always `false`.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
    }
}

/// BLTE compression / encoding modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkEncodingHeader {
    /// No compression (mode 'N')
    None,
    /// ZLib compression (mode 'Z')
    ZLib,
    /// LZ4HC compression (mode '4')
    Lz4hc,
    /// Frame/Recursive BLTE (mode 'F')
    Frame,
    /// Encrypted (mode 'E')
    Encrypted(EncryptedChunkHeader),
}

impl ChunkEncodingHeader {
    /// Parses an chunk encoding header.
    pub fn parse<R: Read>(f: &mut R) -> Result<Self> {
        let mode = f.read_u8()?;

        Ok(match mode {
            b'N' => Self::None,
            b'Z' => Self::ZLib,
            b'4' => Self::Lz4hc,
            b'F' => Self::Frame,
            b'E' => Self::Encrypted(EncryptedChunkHeader::parse(f)?),
            other => return Err(Error::UnknownCompressionMode(other)),
        })
    }

    #[cfg(feature = "async")]
    /// Parses an block encoding header asynchronously.
    pub async fn aparse<R: AsyncReadExt + Unpin>(f: &mut R) -> Result<Self> {
        let mode = f.read_u8().await?;

        Ok(match mode {
            b'N' => Self::None,
            b'Z' => Self::ZLib,
            b'4' => Self::Lz4hc,
            b'F' => Self::Frame,
            b'E' => Self::Encrypted(EncryptedChunkHeader::aparse(f).await?),
            other => return Err(Error::UnknownCompressionMode(other)),
        })
    }

    /// Length of the encoding header on disk.
    pub fn len(&self) -> usize {
        1 + if let Self::Encrypted(h) = self {
            h.len()
        } else {
            0
        }
    }

    /// `true` if the [`BlockEncoding`] would take up 0 bytes.
    ///
    /// This is always `false`.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_compression_mode_detection() -> Result<()> {
        let test_cases = [
            // TODO: add more to other modes
            (b"N".as_slice(), ChunkEncodingHeader::None),
            (b"Z", ChunkEncodingHeader::ZLib),
            (b"4", ChunkEncodingHeader::Lz4hc),
            (b"F", ChunkEncodingHeader::Frame),
            (
                b"E\x05Hello\x0DPlanet Earth!",
                ChunkEncodingHeader::Encrypted(EncryptedChunkHeader {
                    key_name: b"Hello".to_vec(),
                    iv: b"Planet Earth!".to_vec(),
                }),
            ),
        ];

        for (payload, expected) in test_cases {
            let actual = ChunkEncodingHeader::parse(&mut Cursor::new(payload))?;
            assert_eq!(expected, actual, "payload: {:?}", hex::encode(payload));
        }

        Ok(())
    }

    #[test]
    fn test_unknown_compression_mode() {
        let payload = b"XUnknown Mode!";
        let err = ChunkEncodingHeader::parse(&mut Cursor::new(payload)).unwrap_err();
        assert!(matches!(err, Error::UnknownCompressionMode(b'X')));
    }
}
