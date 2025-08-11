//! ESpec (Encoding Specification) parser for BLTE compression
//!
//! ESpec strings define how data should be encoded/compressed in BLTE format.
//! This module implements a parser for the EBNF grammar defined at:
//! <https://wowdev.wiki/BLTE#Encoding_Specification_(ESpec)>

use std::fmt;
use std::str::FromStr;

use crate::{Error, Result};

/// Encoding specification (ESpec) defining how to encode/compress data
#[derive(Debug, Clone, PartialEq)]
pub enum ESpec {
    /// No compression ('n')
    None,
    /// ZLib compression ('z')
    ZLib {
        level: Option<u8>,
        bits: Option<ZLibBits>,
    },
    /// Encryption ('e')
    Encrypted {
        key: String,
        iv: Vec<u8>,
        spec: Box<ESpec>,
    },
    /// Block table ('b')
    BlockTable { chunks: Vec<BlockChunk> },
    /// BCPack compression ('c')
    BCPack { bcn: u8 },
    /// GDeflate compression ('g')
    GDeflate { level: u8 },
}

/// Block chunk specification
#[derive(Debug, Clone, PartialEq)]
pub struct BlockChunk {
    /// Block size specification (optional for final chunk)
    pub size_spec: Option<BlockSizeSpec>,
    /// Encoding specification for this chunk
    pub spec: ESpec,
}

/// Block size specification
#[derive(Debug, Clone, PartialEq)]
pub struct BlockSizeSpec {
    /// Block size in bytes
    pub size: u64,
    /// Number of blocks (optional)
    pub count: Option<u32>,
}

/// ZLib compression bits specification
#[derive(Debug, Clone, PartialEq)]
pub enum ZLibBits {
    /// Numeric window bits
    Bits(u8),
    /// MPQ compression
    MPQ,
    /// ZLib compression
    ZLib,
    /// LZ4HC compression
    LZ4HC,
}

impl ESpec {
    /// Parse an ESpec string
    pub fn parse(input: &str) -> Result<Self> {
        Parser::new(input).parse_espec()
    }

    /// Check if this ESpec uses encryption
    pub fn is_encrypted(&self) -> bool {
        matches!(self, ESpec::Encrypted { .. })
    }

    /// Check if this ESpec uses compression
    pub fn is_compressed(&self) -> bool {
        match self {
            ESpec::None => false,
            ESpec::ZLib { .. } | ESpec::BCPack { .. } | ESpec::GDeflate { .. } => true,
            ESpec::BlockTable { chunks } => chunks.iter().any(|c| c.spec.is_compressed()),
            ESpec::Encrypted { spec, .. } => spec.is_compressed(),
        }
    }

    /// Get the compression type as a string
    pub fn compression_type(&self) -> &str {
        match self {
            ESpec::None => "none",
            ESpec::ZLib { .. } => "zlib",
            ESpec::BCPack { .. } => "bcpack",
            ESpec::GDeflate { .. } => "gdeflate",
            ESpec::BlockTable { .. } => "block",
            ESpec::Encrypted { .. } => "encrypted",
        }
    }
}

impl fmt::Display for ESpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ESpec::None => write!(f, "n"),
            ESpec::ZLib { level, bits } => {
                write!(f, "z")?;
                if let Some(level) = level {
                    write!(f, ":{}", level)?;
                    if let Some(bits) = bits {
                        write!(f, ",{}", bits)?;
                    }
                }
                Ok(())
            }
            ESpec::Encrypted { key, iv, spec } => {
                write!(f, "e:{{{},{},{}}}", key, hex::encode(iv), spec)
            }
            ESpec::BlockTable { chunks } => {
                write!(f, "b:")?;
                if chunks.len() == 1 && chunks[0].size_spec.is_none() {
                    write!(f, "{}", chunks[0].spec)
                } else {
                    write!(f, "{{")?;
                    for (i, chunk) in chunks.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        if let Some(size_spec) = &chunk.size_spec {
                            write!(f, "{}=", size_spec)?;
                        } else {
                            write!(f, "*=")?;
                        }
                        write!(f, "{}", chunk.spec)?;
                    }
                    write!(f, "}}")
                }
            }
            ESpec::BCPack { bcn } => write!(f, "c:{{{}}}", bcn),
            ESpec::GDeflate { level } => write!(f, "g:{{{}}}", level),
        }
    }
}

impl fmt::Display for BlockSizeSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.size % (1024 * 1024) == 0 {
            write!(f, "{}M", self.size / (1024 * 1024))?;
        } else if self.size % 1024 == 0 {
            write!(f, "{}K", self.size / 1024)?;
        } else {
            write!(f, "{}", self.size)?;
        }
        if let Some(count) = self.count {
            write!(f, "*{}", count)?;
        }
        Ok(())
    }
}

impl fmt::Display for ZLibBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZLibBits::Bits(n) => write!(f, "{}", n),
            ZLibBits::MPQ => write!(f, "mpq"),
            ZLibBits::ZLib => write!(f, "zlib"),
            ZLibBits::LZ4HC => write!(f, "lz4hc"),
        }
    }
}

/// Parser for ESpec strings
struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn consume(&mut self, ch: char) -> Result<()> {
        if self.peek() == Some(ch) {
            self.pos += ch.len_utf8();
            Ok(())
        } else {
            Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Expected '{}' at position {}", ch, self.pos),
            )))
        }
    }

    fn parse_number(&mut self) -> Result<u64> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Expected number at position {}", self.pos),
            )));
        }
        self.input[start..self.pos].parse().map_err(|e| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid number: {}", e),
            ))
        })
    }

    fn parse_hex_string(&mut self, len: usize) -> Result<Vec<u8>> {
        let start = self.pos;
        self.pos = (self.pos + len * 2).min(self.input.len());
        let hex_str = &self.input[start..self.pos];
        hex::decode(hex_str).map_err(|e| {
            Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid hex string: {}", e),
            ))
        })
    }

    fn parse_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }

    fn parse_espec(&mut self) -> Result<ESpec> {
        match self.peek() {
            Some('n') => {
                self.consume('n')?;
                Ok(ESpec::None)
            }
            Some('z') => self.parse_zlib(),
            Some('e') => self.parse_encrypted(),
            Some('b') => self.parse_block_table(),
            Some('c') => self.parse_bcpack(),
            Some('g') => self.parse_gdeflate(),
            _ => Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown ESpec type at position {}", self.pos),
            ))),
        }
    }

    fn parse_zlib(&mut self) -> Result<ESpec> {
        self.consume('z')?;

        if self.peek() != Some(':') {
            return Ok(ESpec::ZLib {
                level: None,
                bits: None,
            });
        }

        self.consume(':')?;

        // Check for braces (optional)
        let has_braces = self.peek() == Some('{');
        if has_braces {
            self.consume('{')?;
        }

        // Parse level if present and it's a number
        let level = if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            Some(self.parse_number()? as u8)
        } else {
            None
        };

        // Only parse bits if we have a comma AND we're inside braces
        // (bits require braces in the format)
        let bits = if has_braces && self.peek() == Some(',') {
            self.consume(',')?;
            Some(self.parse_zlib_bits()?)
        } else {
            None
        };

        if has_braces {
            self.consume('}')?;
        }

        Ok(ESpec::ZLib { level, bits })
    }

    fn parse_zlib_bits(&mut self) -> Result<ZLibBits> {
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            Ok(ZLibBits::Bits(self.parse_number()? as u8))
        } else {
            let ident = self.parse_identifier();
            if ident.is_empty() {
                // No bits specified
                return Err(Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Expected zlib bits specification after comma",
                )));
            }
            match ident.as_str() {
                "mpq" => Ok(ZLibBits::MPQ),
                "zlib" => Ok(ZLibBits::ZLib),
                "lz4hc" => Ok(ZLibBits::LZ4HC),
                _ => Err(Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unknown zlib bits type: {}", ident),
                ))),
            }
        }
    }

    fn parse_encrypted(&mut self) -> Result<ESpec> {
        self.consume('e')?;
        self.consume(':')?;
        self.consume('{')?;

        // Parse 8-byte hex key name
        let key = self.parse_identifier();
        if key.len() != 16 {
            return Err(Error::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Encryption key must be 16 hex chars, got {}", key.len()),
            )));
        }

        self.consume(',')?;

        // Parse 4-byte hex IV
        let iv = self.parse_hex_string(4)?;

        self.consume(',')?;

        // Parse nested ESpec
        let spec = Box::new(self.parse_espec()?);

        self.consume('}')?;

        Ok(ESpec::Encrypted { key, iv, spec })
    }

    fn parse_block_table(&mut self) -> Result<ESpec> {
        self.consume('b')?;
        self.consume(':')?;

        // Check if it's a single spec or multiple chunks
        if self.peek() != Some('{') {
            // Single spec without size
            let spec = self.parse_espec()?;
            return Ok(ESpec::BlockTable {
                chunks: vec![BlockChunk {
                    size_spec: None,
                    spec,
                }],
            });
        }

        self.consume('{')?;

        let mut chunks = Vec::new();

        loop {
            // Parse size spec or final chunk
            let size_spec = if self.peek() == Some('*') {
                self.consume('*')?;
                if self.peek() == Some('=') {
                    None // Final chunk with no size
                } else {
                    // Parse count after *
                    let count = self.parse_number()? as u32;
                    Some(BlockSizeSpec {
                        size: 0, // Will be determined by total size
                        count: Some(count),
                    })
                }
            } else {
                Some(self.parse_block_size_spec()?)
            };

            self.consume('=')?;

            let spec = self.parse_espec()?;
            chunks.push(BlockChunk { size_spec, spec });

            if self.peek() == Some(',') {
                self.consume(',')?;
            } else {
                break;
            }
        }

        self.consume('}')?;

        Ok(ESpec::BlockTable { chunks })
    }

    fn parse_block_size_spec(&mut self) -> Result<BlockSizeSpec> {
        let mut size = self.parse_number()?;

        // Check for unit (K or M)
        if let Some(unit) = self.peek() {
            if unit == 'K' {
                self.consume('K')?;
                size *= 1024;
            } else if unit == 'M' {
                self.consume('M')?;
                size *= 1024 * 1024;
            }
        }

        // Check for count
        let count = if self.peek() == Some('*') {
            self.consume('*')?;
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                Some(self.parse_number()? as u32)
            } else {
                None // * without number means "rest of file"
            }
        } else {
            None
        };

        Ok(BlockSizeSpec { size, count })
    }

    fn parse_bcpack(&mut self) -> Result<ESpec> {
        self.consume('c')?;
        self.consume(':')?;
        self.consume('{')?;
        let bcn = self.parse_number()? as u8;
        self.consume('}')?;
        Ok(ESpec::BCPack { bcn })
    }

    fn parse_gdeflate(&mut self) -> Result<ESpec> {
        self.consume('g')?;
        self.consume(':')?;
        self.consume('{')?;
        let level = self.parse_number()? as u8;
        self.consume('}')?;
        Ok(ESpec::GDeflate { level })
    }
}

impl FromStr for ESpec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_none() {
        let spec = ESpec::parse("n").unwrap();
        assert_eq!(spec, ESpec::None);
        assert_eq!(spec.to_string(), "n");
    }

    #[test]
    fn test_parse_zlib_default() {
        let spec = ESpec::parse("z").unwrap();
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: None,
                bits: None
            }
        );
        assert_eq!(spec.to_string(), "z");
    }

    #[test]
    fn test_parse_zlib_with_level() {
        let spec = ESpec::parse("z:9").unwrap();
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: None
            }
        );
        assert_eq!(spec.to_string(), "z:9");
    }

    #[test]
    fn test_parse_zlib_with_level_and_bits() {
        let spec = ESpec::parse("z:{9,15}").unwrap();
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: Some(ZLibBits::Bits(15))
            }
        );
    }

    #[test]
    fn test_parse_zlib_with_mpq() {
        let spec = ESpec::parse("z:{9,mpq}").unwrap();
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: Some(ZLibBits::MPQ)
            }
        );
    }

    #[test]
    fn test_parse_block_table_simple() {
        let spec = ESpec::parse("b:n").unwrap();
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 1);
                assert_eq!(chunks[0].spec, ESpec::None);
                assert!(chunks[0].size_spec.is_none());
            }
            _ => panic!("Expected BlockTable"),
        }
    }

    #[test]
    fn test_parse_block_table_with_sizes() {
        let spec = ESpec::parse("b:{1M*3=z:9,*=n}").unwrap();
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 2);

                // First chunk: 1M * 3 blocks with zlib level 9
                let first = &chunks[0];
                assert_eq!(
                    first.size_spec,
                    Some(BlockSizeSpec {
                        size: 1024 * 1024,
                        count: Some(3),
                    })
                );
                assert_eq!(
                    first.spec,
                    ESpec::ZLib {
                        level: Some(9),
                        bits: None
                    }
                );

                // Final chunk: rest of file uncompressed
                let second = &chunks[1];
                assert!(second.size_spec.is_none());
                assert_eq!(second.spec, ESpec::None);
            }
            _ => panic!("Expected BlockTable"),
        }
    }

    #[test]
    fn test_parse_bcpack() {
        let spec = ESpec::parse("c:{4}").unwrap();
        assert_eq!(spec, ESpec::BCPack { bcn: 4 });
        assert_eq!(spec.to_string(), "c:{4}");
    }

    #[test]
    fn test_parse_gdeflate() {
        let spec = ESpec::parse("g:{5}").unwrap();
        assert_eq!(spec, ESpec::GDeflate { level: 5 });
        assert_eq!(spec.to_string(), "g:{5}");
    }

    #[test]
    fn test_compression_detection() {
        assert!(!ESpec::None.is_compressed());
        assert!(
            ESpec::ZLib {
                level: None,
                bits: None
            }
            .is_compressed()
        );
        assert!(ESpec::BCPack { bcn: 4 }.is_compressed());
        assert!(ESpec::GDeflate { level: 5 }.is_compressed());
    }

    #[test]
    fn test_complex_block_table() {
        let spec = ESpec::parse("b:{256K=n,512K*2=z:6,*=z:9}").unwrap();
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 3);

                // 256KB uncompressed
                assert_eq!(
                    chunks[0].size_spec,
                    Some(BlockSizeSpec {
                        size: 256 * 1024,
                        count: None,
                    })
                );
                assert_eq!(chunks[0].spec, ESpec::None);

                // 512KB * 2 with zlib 6
                assert_eq!(
                    chunks[1].size_spec,
                    Some(BlockSizeSpec {
                        size: 512 * 1024,
                        count: Some(2),
                    })
                );
                assert_eq!(
                    chunks[1].spec,
                    ESpec::ZLib {
                        level: Some(6),
                        bits: None,
                    }
                );

                // Rest with zlib 9
                assert!(chunks[2].size_spec.is_none());
                assert_eq!(
                    chunks[2].spec,
                    ESpec::ZLib {
                        level: Some(9),
                        bits: None,
                    }
                );
            }
            _ => panic!("Expected BlockTable"),
        }
    }
}
