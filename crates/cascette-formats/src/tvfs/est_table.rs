//! TVFS Encoding Spec Table (EST) parsing
//!
//! The EST contains null-terminated encoding specification strings,
//! present when the TVFS_FLAG_ENCODING_SPEC flag (0x02) is set.
//! This matches the ESpec table format used in encoding files.

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};

/// Encoding Spec Table for TVFS
#[derive(Debug, Clone)]
pub struct EstTable {
    /// Encoding spec strings (null-terminated in serialized form)
    pub specs: Vec<String>,
}

impl BinRead for EstTable {
    type Args<'a> = (u32,); // table_size

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let table_size = args.0 as usize;
        let mut data = vec![0u8; table_size];
        reader.read_exact(&mut data)?;

        // Parse null-terminated strings
        let mut specs = Vec::new();
        let mut start = 0;
        for (i, &byte) in data.iter().enumerate() {
            if byte == 0 {
                if i > start
                    && let Ok(s) = std::str::from_utf8(&data[start..i])
                {
                    specs.push(s.to_string());
                }
                start = i + 1;
            }
        }

        Ok(EstTable { specs })
    }
}

impl BinWrite for EstTable {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        for spec in &self.specs {
            writer.write_all(spec.as_bytes())?;
            writer.write_all(&[0])?; // null terminator
        }
        Ok(())
    }
}

impl EstTable {
    /// Create a new empty EST table
    pub fn new() -> Self {
        Self { specs: Vec::new() }
    }

    /// Add an encoding spec string
    pub fn add_spec(&mut self, spec: String) {
        self.specs.push(spec);
    }

    /// Calculate serialized size in bytes
    pub fn calculate_size(&self) -> u32 {
        self.specs
            .iter()
            .map(|s| s.len() + 1) // string bytes + null terminator
            .sum::<usize>() as u32
    }

    /// Find spec by index
    pub fn get_spec(&self, index: usize) -> Option<&str> {
        self.specs.get(index).map(String::as_str)
    }
}

impl Default for EstTable {
    fn default() -> Self {
        Self::new()
    }
}
