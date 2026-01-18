use std::fmt;
use thiserror::Error;

/// `ESpec` parsing and validation errors
#[derive(Debug, Error)]
pub enum ESpecError {
    /// Empty input string
    #[error("Empty `ESpec` string")]
    EmptyInput,

    /// Unknown compression type
    #[error("Unknown compression type: {0}")]
    UnknownType(char),

    /// Invalid compression level
    #[error("Invalid compression level {0}, must be 0-9")]
    InvalidLevel(u8),

    /// Invalid window bits
    #[error("Invalid window bits {0}, must be 9-15")]
    InvalidBits(u8),

    /// Invalid hex string
    #[error("Invalid hex string: {0}")]
    InvalidHex(String),

    /// Unexpected character
    #[error("Expected '{expected}' at position {position}, found '{found}'")]
    UnexpectedChar {
        expected: char,
        found: char,
        position: usize,
    },

    /// Unexpected end of input
    #[error("Unexpected end of input at position {0}")]
    UnexpectedEnd(usize),

    /// Invalid number format
    #[error("Invalid number at position {position}: {error}")]
    InvalidNumber { position: usize, error: String },

    /// Invalid size unit
    #[error("Invalid size unit '{0}', must be K or M")]
    InvalidUnit(char),

    /// Missing encryption parameters
    #[error("Encryption requires key, IV, and nested spec")]
    MissingEncryptionParams,
}

/// Encoding specification defining how to encode/compress data
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ESpec {
    /// No compression ('n')
    None,

    /// `ZLib` compression ('z')
    ZLib {
        /// Compression level (0-9)
        level: Option<u8>,
        /// Window bits or special mode
        bits: Option<ZLibBits>,
    },

    /// Encryption ('e')
    Encrypted {
        /// Encryption key name (8 bytes as 16 hex chars)
        key: String,
        /// Initialization vector (4 bytes)
        iv: Vec<u8>,
        /// Nested specification for encrypted data
        spec: Box<ESpec>,
    },

    /// Block table ('b')
    BlockTable {
        /// List of block chunks with their specifications
        chunks: Vec<BlockChunk>,
    },

    /// `BCPack` compression ('c')
    BCPack {
        /// `BCPack` version number
        bcn: u8,
    },

    /// `GDeflate` compression ('g')
    GDeflate {
        /// Compression level
        level: u8,
    },
}

/// Block chunk specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockChunk {
    /// Block size specification (None for final chunk marked with *)
    pub size_spec: Option<BlockSizeSpec>,
    /// Encoding specification for this chunk
    pub spec: ESpec,
}

/// Block size specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSizeSpec {
    /// Block size in bytes
    pub size: u64,
    /// Number of blocks (None means single block)
    pub count: Option<u32>,
}

/// `ZLib` compression bits specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZLibBits {
    /// Numeric window bits (9-15)
    Bits(u8),
    /// MPQ compression mode
    MPQ,
    /// Standard `ZLib` mode
    ZLib,
    /// LZ4HC compression mode
    LZ4HC,
}

impl ESpec {
    /// Parse an `ESpec` string
    pub fn parse(input: &str) -> Result<Self, ESpecError> {
        if input.is_empty() {
            return Err(ESpecError::EmptyInput);
        }
        crate::espec::Parser::new(input).parse()
    }

    /// Check if this `ESpec` uses encryption
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        matches!(self, Self::Encrypted { .. })
    }

    /// Check if this `ESpec` uses compression
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        match self {
            Self::None => false,
            Self::ZLib { .. } | Self::BCPack { .. } | Self::GDeflate { .. } => true,
            Self::BlockTable { chunks } => chunks.iter().any(|c| c.spec.is_compressed()),
            Self::Encrypted { spec, .. } => spec.is_compressed(),
        }
    }

    /// Get the compression type as a string
    #[must_use]
    pub fn compression_type(&self) -> &str {
        match self {
            Self::None => "none",
            Self::ZLib { .. } => "zlib",
            Self::BCPack { .. } => "bcpack",
            Self::GDeflate { .. } => "gdeflate",
            Self::BlockTable { .. } => "block",
            Self::Encrypted { .. } => "encrypted",
        }
    }

    /// Validate that an `ESpec` string is syntactically correct
    pub fn validate(input: &str) -> bool {
        Self::parse(input).is_ok()
    }
}

impl fmt::Display for ESpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "n"),

            Self::ZLib { level, bits } => {
                write!(f, "z")?;
                match (level, bits) {
                    (None, None) => Ok(()),
                    (Some(l), None) => write!(f, ":{l}"),
                    (Some(l), Some(b)) => write!(f, ":{{{l},{b}}}"),
                    (None, Some(b)) => write!(f, ":{{{b}}}"),
                }
            }

            Self::Encrypted { key, iv, spec } => {
                write!(f, "e:{{{},{},{spec}}}", key, hex::encode(iv))
            }

            Self::BlockTable { chunks } => {
                write!(f, "b:")?;
                // Special case: single chunk without size spec
                if chunks.len() == 1 && chunks[0].size_spec.is_none() {
                    write!(f, "{}", chunks[0].spec)
                } else {
                    write!(f, "{{")?;
                    for (i, chunk) in chunks.iter().enumerate() {
                        if i > 0 {
                            write!(f, ",")?;
                        }
                        if let Some(size_spec) = &chunk.size_spec {
                            write!(f, "{size_spec}=")?;
                        } else {
                            write!(f, "*=")?;
                        }
                        write!(f, "{}", chunk.spec)?;
                    }
                    write!(f, "}}")
                }
            }

            Self::BCPack { bcn } => write!(f, "c:{{{bcn}}}"),

            Self::GDeflate { level } => write!(f, "g:{{{level}}}"),
        }
    }
}

impl fmt::Display for BlockSizeSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use M suffix for megabytes
        if self.size % (1024 * 1024) == 0 {
            write!(f, "{}M", self.size / (1024 * 1024))?;
        }
        // Use K suffix for kilobytes
        else if self.size % 1024 == 0 {
            write!(f, "{}K", self.size / 1024)?;
        }
        // Raw bytes
        else {
            write!(f, "{}", self.size)?;
        }

        // Add count if specified
        if let Some(count) = self.count {
            write!(f, "*{count}")?;
        }

        Ok(())
    }
}

impl fmt::Display for ZLibBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bits(n) => write!(f, "{n}"),
            Self::MPQ => write!(f, "mpq"),
            Self::ZLib => write!(f, "zlib"),
            Self::LZ4HC => write!(f, "lz4hc"),
        }
    }
}

/// Parse an `ESpec` string (convenience function)
pub fn parse(input: &str) -> Result<ESpec, ESpecError> {
    ESpec::parse(input)
}

impl crate::CascFormat for ESpec {
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let input = std::str::from_utf8(data)?;
        Self::parse(input).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(self.to_string().into_bytes())
    }
}
