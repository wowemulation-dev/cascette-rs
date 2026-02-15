use super::types::{BlockChunk, BlockSizeSpec, ESpec, ESpecError, ZLibBits};

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

    /// Parse a hex string of specified length (in bytes)
    fn parse_hex_string(&mut self, byte_len: usize) -> Result<Vec<u8>, ESpecError> {
        let hex_len = byte_len * 2;
        let start = self.pos;
        let end = (self.pos + hex_len).min(self.input.len());

        if end - start < hex_len {
            return Err(ESpecError::InvalidHex(format!(
                "Expected {} hex chars, got {}",
                hex_len,
                end - start
            )));
        }

        self.pos = end;
        let hex_str = &self.input[start..end];
        hex::decode(hex_str).map_err(|e| ESpecError::InvalidHex(e.to_string()))
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
                bits: None,
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

        // Parse bits if we have a comma (only valid inside braces)
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

    /// Parse `ZLib` bits specification
    fn parse_zlib_bits(&mut self) -> Result<ZLibBits, ESpecError> {
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            let num = self.parse_number()?;
            let bits = u8::try_from(num).map_err(|_| ESpecError::InvalidBits(255))?;
            if !(9..=15).contains(&bits) {
                return Err(ESpecError::InvalidBits(bits));
            }
            Ok(ZLibBits::Bits(bits))
        } else {
            let ident = self.parse_identifier();
            match ident.as_str() {
                "mpq" => Ok(ZLibBits::MPQ),
                "zlib" => Ok(ZLibBits::ZLib),
                "lz4hc" => Ok(ZLibBits::LZ4HC),
                _ => Err(ESpecError::InvalidNumber {
                    position: self.pos - ident.len(),
                    error: format!("Unknown zlib bits type: {ident}"),
                }),
            }
        }
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

        // Parse 4-byte hex IV
        let iv = self.parse_hex_string(4)?;

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
            // Single spec without size specification
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
            // Parse size spec or final chunk marker
            let size_spec = if self.peek() == Some('*') {
                self.consume('*')?;
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
        self.consume(':')?;
        self.consume('{')?;
        let num = self.parse_number()?;
        let bcn = u8::try_from(num).map_err(|_| ESpecError::InvalidNumber {
            position: self.pos,
            error: "BCPack version too large".to_string(),
        })?;
        self.consume('}')?;
        Ok(ESpec::BCPack { bcn })
    }

    /// Parse `GDeflate` compression
    fn parse_gdeflate(&mut self) -> Result<ESpec, ESpecError> {
        self.consume('g')?;
        self.consume(':')?;
        self.consume('{')?;
        let num = self.parse_number()?;
        let level = u8::try_from(num).map_err(|_| ESpecError::InvalidLevel(255))?;
        if level > 9 {
            return Err(ESpecError::InvalidLevel(level));
        }
        self.consume('}')?;
        Ok(ESpec::GDeflate { level })
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
                bits: None
            }
        );
        assert_eq!(spec.to_string(), "z");

        // With level
        let spec = ESpec::parse("z:9").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: None
            }
        );
        assert_eq!(spec.to_string(), "z:9");

        // With level and bits
        let spec = ESpec::parse("z:{9,15}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: Some(ZLibBits::Bits(15))
            }
        );
        assert_eq!(spec.to_string(), "z:{9,15}");

        // With special modes
        let spec = ESpec::parse("z:{9,mpq}").expect("Test operation should succeed");
        assert_eq!(
            spec,
            ESpec::ZLib {
                level: Some(9),
                bits: Some(ZLibBits::MPQ)
            }
        );
        assert_eq!(spec.to_string(), "z:{9,mpq}");
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
                        bits: None,
                    }
                );

                // Final chunk
                assert!(chunks[2].size_spec.is_none());
                assert_eq!(
                    chunks[2].spec,
                    ESpec::ZLib {
                        level: Some(9),
                        bits: None,
                    }
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
        assert_eq!(spec.to_string(), "b:{256K=n,512K*2=z:6,*=z:9}");
    }

    #[test]
    fn test_parse_bcpack() {
        let spec = ESpec::parse("c:{4}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::BCPack { bcn: 4 });
        assert_eq!(spec.to_string(), "c:{4}");
    }

    #[test]
    fn test_parse_gdeflate() {
        let spec = ESpec::parse("g:{5}").expect("Test operation should succeed");
        assert_eq!(spec, ESpec::GDeflate { level: 5 });
        assert_eq!(spec.to_string(), "g:{5}");
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

        // Invalid window bits
        assert!(matches!(
            ESpec::parse("z:{9,8}"),
            Err(ESpecError::InvalidBits(8))
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
            "b:n",
            "b:{1M=z:9,*=n}",
            "b:{256K=n,512K*2=z:6,*=z:9}",
            "c:{4}",
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
                bits: None
            }
            .is_compressed()
        );
        assert!(ESpec::BCPack { bcn: 1 }.is_compressed());
        assert!(ESpec::GDeflate { level: 5 }.is_compressed());

        // Test is_encrypted
        assert!(!ESpec::None.is_encrypted());
        assert!(
            !ESpec::ZLib {
                level: Some(9),
                bits: None
            }
            .is_encrypted()
        );

        // Test compression_type
        assert_eq!(ESpec::None.compression_type(), "none");
        assert_eq!(
            ESpec::ZLib {
                level: None,
                bits: None
            }
            .compression_type(),
            "zlib"
        );
        assert_eq!(ESpec::BCPack { bcn: 4 }.compression_type(), "bcpack");
        assert_eq!(ESpec::GDeflate { level: 5 }.compression_type(), "gdeflate");
    }

    #[test]
    fn test_validate() {
        // Valid specs
        assert!(ESpec::validate("n"));
        assert!(ESpec::validate("z"));
        assert!(ESpec::validate("z:5"));
        assert!(ESpec::validate("z:{9,15}"));
        assert!(ESpec::validate("b:n"));
        assert!(ESpec::validate("c:{1}"));
        assert!(ESpec::validate("g:{5}"));

        // Invalid specs
        assert!(!ESpec::validate(""));
        assert!(!ESpec::validate("invalid"));
        assert!(!ESpec::validate("x"));
        assert!(!ESpec::validate("z:abc"));
        assert!(!ESpec::validate("z:10"));
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
                        bits: None
                    }
                );
                assert_eq!(chunks[1].spec, ESpec::None);
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }

    #[test]
    fn test_wowdev_wiki_example_encrypted() {
        // Example 3: Encrypted blocks (note: this has an invalid key in the wiki, we'll create a valid one)
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
                                bits: None
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
                        bits: None
                    }
                );
                assert_eq!(chunks[2].spec, ESpec::None);
                assert!(chunks[6].size_spec.is_none()); // Final chunk with *
                assert_eq!(
                    chunks[6].spec,
                    ESpec::ZLib {
                        level: None,
                        bits: None
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
                        bits: Some(ZLibBits::MPQ)
                    }
                );
            }
            _ => unreachable!("Test should produce BlockTable"),
        }
    }
}
