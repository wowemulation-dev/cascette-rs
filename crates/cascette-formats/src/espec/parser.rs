use super::types::{BlockChunk, BlockSizeSpec, ESpec, ESpecError, ZLibVariant};

/// Parser for `ESpec` strings
pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given input
    #[must_use]
    pub const fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Parse the input string into an `ESpec`
    pub fn parse(mut self) -> Result<ESpec, ESpecError> {
        if self.input.is_empty() {
            return Err(ESpecError::EmptyInput);
        }
        let spec = self.parse_espec()?;
        // Ensure we consumed all input
        if self.pos < self.input.len() {
            let ch = self.peek().unwrap_or('?');
            return Err(ESpecError::UnexpectedChar {
                expected: '\0',
                found: ch,
                position: self.pos,
            });
        }
        Ok(spec)
    }

    /// Peek at the current character without consuming it
    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    /// Consume a specific character
    fn consume(&mut self, expected: char) -> Result<(), ESpecError> {
        match self.peek() {
            Some(ch) if ch == expected => {
                self.pos += ch.len_utf8();
                Ok(())
            }
            Some(ch) => Err(ESpecError::UnexpectedChar {
                expected,
                found: ch,
                position: self.pos,
            }),
            None => Err(ESpecError::UnexpectedEnd(self.pos)),
        }
    }

    /// Parse a number from the input
    fn parse_number(&mut self) -> Result<u64, ESpecError> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }

        if self.pos == start {
            return Err(ESpecError::InvalidNumber {
                position: self.pos,
                error: "Expected number".to_string(),
            });
        }

        self.input[start..self.pos]
            .parse::<u64>()
            .map_err(|e| ESpecError::InvalidNumber {
                position: start,
                error: e.to_string(),
            })
    }

    /// Parse an identifier (alphanumeric string)
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

    /// Parse an `ESpec` from the current position
    fn parse_espec(&mut self) -> Result<ESpec, ESpecError> {
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
            Some(ch) => Err(ESpecError::UnknownType(ch)),
            None => Err(ESpecError::UnexpectedEnd(self.pos)),
        }
    }

    /// Parse `ZLib` compression specification
    fn parse_zlib(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('z')?;

        // Check for optional parameters
        if self.peek() != Some(':') {
            return Ok(ESpec::ZLib {
                level: None,
                variant: None,
                window_bits: None,
            });
        }

        self.consume(':')?;

        // Check for braces (optional)
        let has_braces = self.peek() == Some('{');
        if has_braces {
            self.consume('{')?;
        }

        // Parse level if present
        let level = if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let num = self.parse_number()?;
            let level = u8::try_from(num).map_err(|_| ESpecError::InvalidLevel(255))?;
            if level == 0 || level > 9 {
                return Err(ESpecError::InvalidLevel(level));
            }
            Some(level)
        } else if !has_braces {
            // If no braces and no digit after colon, it's an error
            return match self.peek() {
                Some(ch) => Err(ESpecError::UnexpectedChar {
                    expected: '0',
                    found: ch,
                    position: self.pos,
                }),
                None => Err(ESpecError::UnexpectedEnd(self.pos)),
            };
        } else {
            None
        };

        // Parse variant or window_bits if we have a comma (only valid inside braces)
        let mut variant = None;
        let mut window_bits = None;

        if has_braces && self.peek() == Some(',') {
            self.consume(',')?;
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                // Numeric: window bits
                window_bits = Some(self.parse_window_bits()?);
            } else {
                // Identifier: variant name
                variant = Some(self.parse_zlib_variant()?);

                // Check for a third parameter (window_bits after variant)
                if self.peek() == Some(',') {
                    self.consume(',')?;
                    window_bits = Some(self.parse_window_bits()?);
                }
            }
        }

        if has_braces {
            self.consume('}')?;
        }

        Ok(ESpec::ZLib {
            level,
            variant,
            window_bits,
        })
    }

    /// Parse `ZLib` variant identifier
    fn parse_zlib_variant(&mut self) -> Result<ZLibVariant, ESpecError> {
        let ident = self.parse_identifier();
        match ident.as_str() {
            "mpq" => Ok(ZLibVariant::MPQ),
            "zlib" => Ok(ZLibVariant::ZLib),
            "lz4hc" => Ok(ZLibVariant::LZ4HC),
            _ => Err(ESpecError::InvalidNumber {
                position: self.pos - ident.len(),
                error: format!("Unknown zlib variant: {ident}"),
            }),
        }
    }

    /// Parse window bits value (8-15)
    fn parse_window_bits(&mut self) -> Result<u8, ESpecError> {
        let num = self.parse_number()?;
        let bits = u8::try_from(num).map_err(|_| ESpecError::InvalidBits(255))?;
        if !(8..=15).contains(&bits) {
            return Err(ESpecError::InvalidBits(bits));
        }
        Ok(bits)
    }

    /// Parse variable-length hex string up to a delimiter character.
    ///
    /// Returns the decoded bytes. Validates that the hex length is even
    /// and within `[min_bytes, max_bytes]`.
    fn parse_hex_until(
        &mut self,
        delimiter: char,
        min_bytes: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ESpecError> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == delimiter {
                break;
            }
            if !ch.is_ascii_hexdigit() {
                return Err(ESpecError::InvalidHex(format!(
                    "Invalid hex character '{ch}' at position {}",
                    self.pos
                )));
            }
            self.pos += 1;
        }

        let hex_str = &self.input[start..self.pos];
        if !hex_str.len().is_multiple_of(2) {
            return Err(ESpecError::InvalidHex(format!(
                "Odd number of hex chars: {}",
                hex_str.len()
            )));
        }

        let byte_len = hex_str.len() / 2;
        if byte_len < min_bytes || byte_len > max_bytes {
            return Err(ESpecError::InvalidIvLength(byte_len));
        }

        hex::decode(hex_str).map_err(|e| ESpecError::InvalidHex(e.to_string()))
    }

    /// Parse encrypted `ESpec`
    fn parse_encrypted(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('e')?;
        self.consume(':')?;
        self.consume('{')?;

        // Parse 8-byte hex key name (16 hex chars)
        let key = self.parse_identifier();
        if key.len() != 16 {
            return Err(ESpecError::InvalidHex(format!(
                "Key must be 16 hex chars, got {}",
                key.len()
            )));
        }

        self.consume(',')?;

        // Parse 1-8 byte hex IV (variable length, Agent.exe zero-pads to 8)
        let iv = self.parse_hex_until(',', 1, 8)?;

        self.consume(',')?;

        // Parse nested ESpec
        let spec = Box::new(self.parse_espec()?);

        self.consume('}')?;

        Ok(ESpec::Encrypted { key, iv, spec })
    }

    /// Parse block table specification
    fn parse_block_table(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('b')?;
        self.consume(':')?;

        // Check if it's a single spec or multiple chunks
        if self.peek() != Some('{') {
            // Could be either `b:<espec>` (no size spec) or `b:<size_spec>=<espec>` (shorthand)
            if self.peek().is_some_and(|c| c.is_ascii_digit() || c == '*') {
                // Shorthand with size spec, no braces (e.g., `b:256K*=z`)
                let size_spec = if self.peek() == Some('*') {
                    self.consume('*')?;
                    None
                } else {
                    Some(self.parse_block_size_spec()?)
                };
                self.consume('=')?;
                let spec = self.parse_espec()?;
                return Ok(ESpec::BlockTable {
                    chunks: vec![BlockChunk { size_spec, spec }],
                });
            }
            // Single spec without size specification (e.g., `b:n`)
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
        let mut variable_count: usize = 0;

        loop {
            // Parse size spec or final chunk marker
            let size_spec = if self.peek() == Some('*') {
                self.consume('*')?;
                variable_count += 1;
                if variable_count > 1 {
                    return Err(ESpecError::MultipleVariableBlocks);
                }
                if self.peek() == Some('=') {
                    None // Final chunk with no size
                } else {
                    // Parse count after *
                    let num = self.parse_number()?;
                    let count = u32::try_from(num).map_err(|_| ESpecError::InvalidNumber {
                        position: self.pos,
                        error: "Count too large".to_string(),
                    })?;
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

    /// Parse block size specification
    fn parse_block_size_spec(&mut self) -> Result<BlockSizeSpec, ESpecError> {
        let mut size = self.parse_number()?;

        // Check for unit (K or M)
        if let Some(unit) = self.peek() {
            match unit {
                'K' => {
                    self.consume('K')?;
                    size *= 1024;
                }
                'M' => {
                    self.consume('M')?;
                    size *= 1024 * 1024;
                }
                'G' | 'T' | 'P' => {
                    return Err(ESpecError::InvalidUnit(unit));
                }
                _ => {}
            }
        }

        // Check for count
        let count = if self.peek() == Some('*') {
            self.consume('*')?;
            if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                let num = self.parse_number()?;
                Some(u32::try_from(num).map_err(|_| ESpecError::InvalidNumber {
                    position: self.pos,
                    error: "Count too large".to_string(),
                })?)
            } else {
                None // * without number means "rest of file"
            }
        } else {
            None
        };

        Ok(BlockSizeSpec { size, count })
    }

    /// Parse `BCPack` compression
    fn parse_bcpack(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('c')?;

        // Check for optional parameters
        if self.peek() != Some(':') {
            return Ok(ESpec::BCPack { bcn: None });
        }

        self.consume(':')?;
        self.consume('{')?;
        let num = self.parse_number()?;
        let bcn = u8::try_from(num).map_err(|_| ESpecError::InvalidNumber {
            position: self.pos,
            error: "BCPack version too large".to_string(),
        })?;
        if bcn == 0 || bcn > 7 {
            return Err(ESpecError::InvalidBcn(bcn));
        }
        self.consume('}')?;
        Ok(ESpec::BCPack { bcn: Some(bcn) })
    }

    /// Parse `GDeflate` compression
    fn parse_gdeflate(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('g')?;

        // Check for optional parameters
        if self.peek() != Some(':') {
            return Ok(ESpec::GDeflate { level: None });
        }

        self.consume(':')?;
        self.consume('{')?;
        let num = self.parse_number()?;
        let level = u8::try_from(num).map_err(|_| ESpecError::InvalidLevel(255))?;
        if level == 0 || level > 12 {
            return Err(ESpecError::InvalidLevel(level));
        }
        self.consume('}')?;
        Ok(ESpec::GDeflate { level: Some(level) })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_none() {
        let spec = ESpec::parse("n").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::None);
        assert_eq!(spec.to_string(), "n");
    }

    #[test]
    fn test_parse_zlib_variants() {
        // Default
        let spec = ESpec::parse("z").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: None,
                variant: None,
                window_bits: None,
            }
        );
        assert_eq!(spec.to_string(), "z");

        // With level
        let spec = ESpec::parse("z:9").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                variant: None,
                window_bits: None,
            }
        );
        assert_eq!(spec.to_string(), "z:9");

        // With level and window bits
        let spec = ESpec::parse("z:{9,15}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                variant: None,
                window_bits: Some(15),
            }
        );
        assert_eq!(spec.to_string(), "z:{9,15}");

        // With level and variant
        let spec = ESpec::parse("z:{9,mpq}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                variant: Some(ZLibVariant::MPQ),
                window_bits: None,
            }
        );
        assert_eq!(spec.to_string(), "z:{9,mpq}");

        // Window bits 8 is now valid
        let spec = ESpec::parse("z:{9,8}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                variant: None,
                window_bits: Some(8),
            }
        );
        assert_eq!(spec.to_string(), "z:{9,8}");

        // 3-param: level + variant + window_bits
        let spec = ESpec::parse("z:{6,zlib,15}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(6),
                variant: Some(ZLibVariant::ZLib),
                window_bits: Some(15),
            }
        );
        assert_eq!(spec.to_string(), "z:{6,zlib,15}");

        // 3-param: level + mpq variant + window_bits
        let spec = ESpec::parse("z:{6,mpq,12}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(6),
                variant: Some(ZLibVariant::MPQ),
                window_bits: Some(12),
            }
        );
        assert_eq!(spec.to_string(), "z:{6,mpq,12}");
    }

    #[test]
    fn test_parse_block_table() {
        // Simple
        let spec = ESpec::parse("b:n").expect("Test operation should succeed");
        match &spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 1);
                assert_eq!(chunks[0].spec, ESpec::None);
                assert!(chunks[0].size_spec.is_none());
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
        assert_eq!(spec.to_string(), "b:n");

        // Complex with sizes
        let spec =
            ESpec::parse("b:{256K=n,512K*2=z:6,*=z:9}").expect("Test operation should succeed");
        match &spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 3);

                // First chunk
                assert_eq!(
                    chunks[0].size_spec,
                    Some(BlockSizeSpec {
                        size: 256 * 1024,
                        count: None,
                    })
                );
                assert_eq!(chunks[0].spec, ESpec::None);

                // Second chunk
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
                        variant: None,
                        window_bits: None,
                    }
                );

                // Final chunk
                assert!(chunks[2].size_spec.is_none());
                assert_eq!(
                    chunks[2].spec,
                    ESpec::ZLib {
                        level: Some(9),
                        variant: None,
                        window_bits: None,
                    }
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
        assert_eq!(spec.to_string(), "b:{256K=n,512K*2=z:6,*=z:9}");
    }

    #[test]
    fn test_parse_bcpack() {
        // Bare bcpack
        let spec = ESpec::parse("c").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::BCPack { bcn: None });
        assert_eq!(spec.to_string(), "c");

        // With version
        let spec = ESpec::parse("c:{4}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::BCPack { bcn: Some(4) });
        assert_eq!(spec.to_string(), "c:{4}");

        // BCn range boundaries
        let spec = ESpec::parse("c:{1}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::BCPack { bcn: Some(1) });
        let spec = ESpec::parse("c:{7}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::BCPack { bcn: Some(7) });

        // BCn out of range
        assert!(matches!(
            ESpec::parse("c:{0}"),
            Err(ESpecError::InvalidBcn(0))
        ));
        assert!(matches!(
            ESpec::parse("c:{8}"),
            Err(ESpecError::InvalidBcn(8))
        ));
    }

    #[test]
    fn test_parse_gdeflate() {
        // Bare gdeflate
        let spec = ESpec::parse("g").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::GDeflate { level: None });
        assert_eq!(spec.to_string(), "g");

        // With level
        let spec = ESpec::parse("g:{5}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::GDeflate { level: Some(5) });
        assert_eq!(spec.to_string(), "g:{5}");

        // GDeflate level 12 is valid
        let spec = ESpec::parse("g:{12}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::GDeflate { level: Some(12) });

        // GDeflate level 6
        let spec = ESpec::parse("g:{6}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::GDeflate { level: Some(6) });

        // GDeflate level out of range
        assert!(matches!(
            ESpec::parse("g:{0}"),
            Err(ESpecError::InvalidLevel(0))
        ));
        assert!(matches!(
            ESpec::parse("g:{13}"),
            Err(ESpecError::InvalidLevel(13))
        ));
    }

    #[test]
    fn test_parse_errors() {
        // Empty input
        assert!(matches!(ESpec::parse(""), Err(ESpecError::EmptyInput)));

        // Unknown type
        assert!(matches!(
            ESpec::parse("x"),
            Err(ESpecError::UnknownType('x'))
        ));

        // Invalid compression level (0 is rejected)
        assert!(matches!(
            ESpec::parse("z:0"),
            Err(ESpecError::InvalidLevel(0))
        ));

        // Invalid compression level (10 is rejected)
        assert!(matches!(
            ESpec::parse("z:10"),
            Err(ESpecError::InvalidLevel(10))
        ));

        // Invalid window bits (7 is too low)
        assert!(matches!(
            ESpec::parse("z:{9,7}"),
            Err(ESpecError::InvalidBits(7))
        ));

        // Invalid window bits (16 is too high)
        assert!(matches!(
            ESpec::parse("z:{9,16}"),
            Err(ESpecError::InvalidBits(16))
        ));

        // Trailing characters
        assert!(matches!(
            ESpec::parse("n extra"),
            Err(ESpecError::UnexpectedChar { .. })
        ));
    }

    #[test]
    fn test_round_trip() {
        let test_cases = vec![
            "n",
            "z",
            "z:5",
            "z:{9,15}",
            "z:{9,mpq}",
            "z:{6,zlib,15}",
            "b:n",
            "b:{1M=z:9,*=n}",
            "b:{256K=n,512K*2=z:6,*=z:9}",
            "c",
            "c:{4}",
            "g",
            "g:{5}",
        ];

        for input in test_cases {
            let spec = ESpec::parse(input).expect("Test operation should succeed");
            let output = spec.to_string();
            let reparsed = ESpec::parse(&output).expect("Test operation should succeed");
            assert_eq!(spec, reparsed, "Round-trip failed for {input}");
        }
    }

    #[test]
    fn test_helpers() {
        // Test is_compressed
        assert!(!ESpec::None.is_compressed());
        assert!(
            ESpec::ZLib {
                level: None,
                variant: None,
                window_bits: None,
            }
            .is_compressed()
        );
        assert!(ESpec::BCPack { bcn: Some(1) }.is_compressed());
        assert!(ESpec::BCPack { bcn: None }.is_compressed());
        assert!(ESpec::GDeflate { level: Some(5) }.is_compressed());
        assert!(ESpec::GDeflate { level: None }.is_compressed());

        // Test is_encrypted
        assert!(!ESpec::None.is_encrypted());
        assert!(
            !ESpec::ZLib {
                level: Some(9),
                variant: None,
                window_bits: None,
            }
            .is_encrypted()
        );

        // Test compression_type
        assert_eq!(ESpec::None.compression_type(), "none");
        assert_eq!(
            ESpec::ZLib {
                level: None,
                variant: None,
                window_bits: None,
            }
            .compression_type(),
            "zlib"
        );
        assert_eq!(ESpec::BCPack { bcn: Some(4) }.compression_type(), "bcpack");
        assert_eq!(
            ESpec::GDeflate { level: Some(5) }.compression_type(),
            "gdeflate"
        );
    }

    #[test]
    fn test_validate() {
        // Valid specs
        assert!(ESpec::validate("n"));
        assert!(ESpec::validate("z"));
        assert!(ESpec::validate("z:5"));
        assert!(ESpec::validate("z:{9,15}"));
        assert!(ESpec::validate("z:{9,8}"));
        assert!(ESpec::validate("z:{6,zlib,15}"));
        assert!(ESpec::validate("b:n"));
        assert!(ESpec::validate("c"));
        assert!(ESpec::validate("c:{1}"));
        assert!(ESpec::validate("g"));
        assert!(ESpec::validate("g:{5}"));
        assert!(ESpec::validate("g:{12}"));

        // Invalid specs
        assert!(!ESpec::validate(""));
        assert!(!ESpec::validate("invalid"));
        assert!(!ESpec::validate("x"));
        assert!(!ESpec::validate("z:abc"));
        assert!(!ESpec::validate("z:10"));
        assert!(!ESpec::validate("c:{0}"));
        assert!(!ESpec::validate("c:{8}"));
        assert!(!ESpec::validate("g:{0}"));
        assert!(!ESpec::validate("g:{13}"));
    }

    #[test]
    fn test_wowdev_wiki_example_mixed_blocks() {
        // Example 1: Mixed block sizes with zlib
        let spec = ESpec::parse("b:{164=z,16K*565=z,1656=z,140164=z}")
            .expect("Test operation should succeed");
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 4);
                assert_eq!(
                    chunks[0].size_spec,
                    Some(BlockSizeSpec {
                        size: 164,
                        count: None
                    })
                );
                assert_eq!(
                    chunks[1].size_spec,
                    Some(BlockSizeSpec {
                        size: 16_384,
                        count: Some(565)
                    })
                );
                assert_eq!(
                    chunks[2].size_spec,
                    Some(BlockSizeSpec {
                        size: 1656,
                        count: None
                    })
                );
                assert_eq!(
                    chunks[3].size_spec,
                    Some(BlockSizeSpec {
                        size: 140_164,
                        count: None
                    })
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_wowdev_wiki_example_mixed_compression() {
        // Example 2: Mixed compression
        let spec = ESpec::parse("b:{1768=z,66443=n}").expect("Test operation should succeed");
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 2);
                assert_eq!(
                    chunks[0].spec,
                    ESpec::ZLib {
                        level: None,
                        variant: None,
                        window_bits: None,
                    }
                );
                assert_eq!(chunks[1].spec, ESpec::None);
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_wowdev_wiki_example_encrypted() {
        // Example 3: Encrypted blocks
        let valid_encrypted = "b:{256K*=e:{0123456789ABCDEF,06FC152E,z}}";
        let spec = ESpec::parse(valid_encrypted).expect("Test operation should succeed");
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 1);
                assert_eq!(
                    chunks[0].size_spec,
                    Some(BlockSizeSpec {
                        size: 262_144,
                        count: None
                    })
                );
                match &chunks[0].spec {
                    ESpec::Encrypted { key, spec, .. } => {
                        assert_eq!(key, "0123456789ABCDEF");
                        assert_eq!(
                            **spec,
                            ESpec::ZLib {
                                level: None,
                                variant: None,
                                window_bits: None,
                            }
                        );
                    }
                    _ => unreachable!("Test should produce Encrypted"),
                }
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_wowdev_wiki_example_complex_table() {
        // Example 5: Complex mixed block table
        let spec = ESpec::parse("b:{22=n,31943=z,211232=n,27037696=n,138656=n,17747968=n,*=z}")
            .expect("Test operation should succeed");
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 7);
                assert_eq!(chunks[0].spec, ESpec::None);
                assert_eq!(
                    chunks[1].spec,
                    ESpec::ZLib {
                        level: None,
                        variant: None,
                        window_bits: None,
                    }
                );
                assert_eq!(chunks[2].spec, ESpec::None);
                assert!(chunks[6].size_spec.is_none()); // Final chunk with *
                assert_eq!(
                    chunks[6].spec,
                    ESpec::ZLib {
                        level: None,
                        variant: None,
                        window_bits: None,
                    }
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_wowdev_wiki_example_mpq() {
        // Example 6: MPQ compression mode
        let spec = ESpec::parse("b:{16K*=z:{6,mpq}}").expect("Test operation should succeed");
        match spec {
            ESpec::BlockTable { chunks } => {
                assert_eq!(chunks.len(), 1);
                assert_eq!(
                    chunks[0].size_spec,
                    Some(BlockSizeSpec {
                        size: 16_384,
                        count: None
                    })
                );
                assert_eq!(
                    chunks[0].spec,
                    ESpec::ZLib {
                        level: Some(6),
                        variant: Some(ZLibVariant::MPQ),
                        window_bits: None,
                    }
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }
}
