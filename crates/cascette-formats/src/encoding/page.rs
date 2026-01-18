use binrw::{BinRead, BinResult, BinWrite};
use std::io::{Cursor, Read, Seek, Write};

/// Page information (first key and checksum)
#[derive(Debug, Clone, BinRead, BinWrite)]
pub struct PageInfo {
    /// First key in this page (for binary search)
    pub first_key: [u8; 16],
    /// MD5 checksum of page content
    pub checksum: [u8; 16],
}

/// Encoding page containing entries
#[derive(Debug, Clone)]
pub struct EncodingPage<T> {
    /// Page metadata (first key and checksum)
    pub info: PageInfo,
    /// Entries in this page
    pub entries: Vec<T>,
}

impl<T> BinRead for EncodingPage<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    type Args<'a> = (usize,); // Page size in bytes

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let (page_size,) = args;

        // Read page info
        let info = PageInfo::read_options(reader, endian, ())?;

        // Read page content
        let mut page_data = vec![0u8; page_size];
        reader.read_exact(&mut page_data)?;

        // Parse entries from page data
        let mut cursor = Cursor::new(page_data);
        let mut entries = Vec::new();

        while cursor.position() < cursor.get_ref().len() as u64 {
            // Check if we have at least one byte to read
            let pos = cursor.position();
            if pos >= cursor.get_ref().len() as u64 {
                break;
            }

            // Peek at the next byte to see if it's a valid entry
            let mut peek_byte = [0u8; 1];
            if cursor.read_exact(&mut peek_byte).is_err() {
                break;
            }

            // If it's zero, we've hit padding
            if peek_byte[0] == 0 {
                break;
            }

            // Reset position and read the entry
            cursor.set_position(pos);

            match T::read_options(&mut cursor, endian, ()) {
                Ok(entry) => entries.push(entry),
                Err(_) => break, // End of valid entries
            }
        }

        Ok(Self { info, entries })
    }
}

impl<T> BinWrite for EncodingPage<T>
where
    T: for<'a> BinWrite<Args<'a> = ()>,
{
    type Args<'a> = (usize,); // Page size in bytes

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        let (page_size,) = args;

        // Write page info
        self.info.write_options(writer, endian, ())?;

        // Create page buffer
        let mut page_data = vec![0u8; page_size];
        let mut cursor = Cursor::new(&mut page_data);

        // Write entries to buffer
        for entry in &self.entries {
            entry.write_options(&mut cursor, endian, ())?;
        }

        // Write page data
        writer.write_all(&page_data)?;

        Ok(())
    }
}

impl<T> EncodingPage<T> {
    /// Create a new encoding page
    pub fn new(first_key: [u8; 16], entries: Vec<T>) -> Self {
        Self {
            info: PageInfo {
                first_key,
                checksum: [0; 16], // Will be calculated later
            },
            entries,
        }
    }

    /// Calculate and update the checksum for this page
    pub fn update_checksum(&mut self, page_data: &[u8]) {
        let digest = md5::compute(page_data);
        self.info.checksum = digest.into();
    }
}
