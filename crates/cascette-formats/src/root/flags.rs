//! Content and locale flags for root file entries

use binrw::{BinRead, BinWrite};
use std::fmt;

/// Content flags indicate how files are stored and processed
///
/// V1-V3 use 32-bit flags, V4 extends to 40-bit (5 bytes)
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentFlags {
    /// Raw flag value (up to 40 bits for V4)
    pub value: u64,
}

impl ContentFlags {
    /// No special flags
    pub const NONE: u64 = 0x0000_0000;

    /// Windows platform (bit 0)
    pub const LOAD_ON_WINDOWS: u64 = 0x0001;

    /// macOS platform (bit 1)
    pub const LOAD_ON_MACOS: u64 = 0x0002;

    /// File should be installed (bit 2)
    pub const INSTALL: u64 = 0x0004;

    /// Low violence version (bit 3)
    pub const LOW_VIOLENCE: u64 = 0x0008;

    /// Do not load (bit 9)
    pub const DO_NOT_LOAD: u64 = 0x0200;

    /// Update plugin (bit 10)
    pub const UPDATE_PLUGIN: u64 = 0x0400;

    /// ARM64 architecture (bit 11)
    pub const ARM64: u64 = 0x0800;

    /// Encrypted content (bit 12)
    pub const ENCRYPTED: u64 = 0x1000;

    /// No name hash present in block (bit 13) - V2+ only
    pub const NO_NAME_HASH: u64 = 0x2000;

    /// Uncommon resolution (bit 14)
    pub const UNCOMMON_RESOLUTION: u64 = 0x4000;

    /// Bundled file (bit 15)
    pub const BUNDLE: u64 = 0x8000;

    /// No compression applied (bit 16)
    pub const NO_COMPRESSION: u64 = 0x0001_0000;

    /// No TOC hash (bit 17)
    pub const NO_TOC_HASH: u64 = 0x0002_0000;

    /// Create new content flags from raw value
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// Check if flag is set
    pub const fn has(&self, flag: u64) -> bool {
        (self.value & flag) != 0
    }

    /// Set flag
    pub fn set(&mut self, flag: u64) {
        self.value |= flag;
    }

    /// Clear flag
    pub fn clear(&mut self, flag: u64) {
        self.value &= !flag;
    }

    /// Check if name hashes should be present
    pub const fn has_name_hashes(&self) -> bool {
        !self.has(Self::NO_NAME_HASH)
    }

    /// Read as 32-bit value (V1-V3)
    pub fn read_v1_v3<R: std::io::Read + std::io::Seek>(reader: &mut R) -> binrw::BinResult<Self> {
        let value = u64::from(u32::read_le(reader)?);
        Ok(Self::new(value))
    }

    /// Write as 32-bit value (V1-V3)
    pub fn write_v1_v3<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
    ) -> binrw::BinResult<()> {
        let value = (self.value & 0xFFFF_FFFF) as u32;
        value.write_le(writer)
    }

    /// Read as 40-bit value (V4)
    pub fn read_v4<R: std::io::Read + std::io::Seek>(reader: &mut R) -> binrw::BinResult<Self> {
        let low = u64::from(u32::read_le(reader)?);
        let high = u64::from(u8::read_le(reader)?);
        let value = low | (high << 32);
        Ok(Self::new(value))
    }

    /// Write as 40-bit value (V4)
    pub fn write_v4<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
    ) -> binrw::BinResult<()> {
        let low = (self.value & 0xFFFF_FFFF) as u32;
        let high = ((self.value >> 32) & 0xFF) as u8;
        low.write_le(writer)?;
        high.write_le(writer)?;
        Ok(())
    }
}

impl fmt::Display for ContentFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}", self.value)
    }
}

impl From<u32> for ContentFlags {
    fn from(value: u32) -> Self {
        Self::new(u64::from(value))
    }
}

impl From<u64> for ContentFlags {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

/// Locale flags indicate which game locales a file applies to
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[brw(little)]
pub struct LocaleFlags(pub u32);

impl LocaleFlags {
    /// All locales
    pub const ALL: u32 = 0xFFFF_FFFF;

    /// English (United States)
    pub const ENUS: u32 = 0x0000_0002;

    /// Korean
    pub const KOKR: u32 = 0x0000_0004;

    /// French (France)
    pub const FRFR: u32 = 0x0000_0010;

    /// German (Germany)
    pub const DEDE: u32 = 0x0000_0020;

    /// Chinese (China)
    pub const ZHCN: u32 = 0x0000_0040;

    /// Spanish (Spain)
    pub const ESES: u32 = 0x0000_0080;

    /// Chinese (Taiwan)
    pub const ZHTW: u32 = 0x0000_0100;

    /// English (Great Britain)
    pub const ENGB: u32 = 0x0000_0200;

    /// Portuguese (Brazil)
    pub const PTBR: u32 = 0x0000_0400;

    /// Italian (Italy)
    pub const ITIT: u32 = 0x0000_0800;

    /// Russian
    pub const RURU: u32 = 0x0000_1000;

    /// Create new locale flags
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get raw value
    pub const fn value(&self) -> u32 {
        self.0
    }

    /// Check if locale flag is set
    pub const fn has(&self, locale: u32) -> bool {
        (self.0 & locale) != 0
    }

    /// Set locale flag
    pub fn set(&mut self, locale: u32) {
        self.0 |= locale;
    }

    /// Clear locale flag
    pub fn clear(&mut self, locale: u32) {
        self.0 &= !locale;
    }

    /// Check if matches any of the specified locales
    pub const fn matches(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl fmt::Display for LocaleFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}", self.0)
    }
}

impl From<u32> for LocaleFlags {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

/// Bitwise AND for locale filtering
impl std::ops::BitAnd for LocaleFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// Bitwise OR for locale combining
impl std::ops::BitOr for LocaleFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use binrw::io::Cursor;

    #[test]
    fn test_content_flags_basic() {
        let flags = ContentFlags::new(ContentFlags::INSTALL | ContentFlags::BUNDLE);

        assert!(flags.has(ContentFlags::INSTALL));
        assert!(flags.has(ContentFlags::BUNDLE));
        assert!(!flags.has(ContentFlags::LOW_VIOLENCE));
        assert!(flags.has_name_hashes()); // NO_NAME_HASH not set
    }

    #[test]
    fn test_content_flags_no_name_hash() {
        let flags = ContentFlags::new(ContentFlags::NO_NAME_HASH);
        assert!(!flags.has_name_hashes());
    }

    #[test]
    fn test_content_flags_all_combinations() {
        // Test all individual flags
        let flag_values = [
            ContentFlags::NONE,
            ContentFlags::LOAD_ON_WINDOWS,
            ContentFlags::LOAD_ON_MACOS,
            ContentFlags::INSTALL,
            ContentFlags::LOW_VIOLENCE,
            ContentFlags::DO_NOT_LOAD,
            ContentFlags::UPDATE_PLUGIN,
            ContentFlags::ARM64,
            ContentFlags::ENCRYPTED,
            ContentFlags::NO_NAME_HASH,
            ContentFlags::UNCOMMON_RESOLUTION,
            ContentFlags::BUNDLE,
            ContentFlags::NO_COMPRESSION,
            ContentFlags::NO_TOC_HASH,
        ];

        for &flag in &flag_values {
            let flags = ContentFlags::new(flag);

            // NONE is special - it means no flags are set
            if flag == ContentFlags::NONE {
                assert_eq!(flags.value, 0);
            } else {
                assert!(flags.has(flag));
            }

            // Test that other flags are not set
            for &other_flag in &flag_values {
                if other_flag != flag && other_flag != ContentFlags::NONE {
                    assert!(
                        !flags.has(other_flag),
                        "Flag {flag:08x} incorrectly has {other_flag:08x}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_content_flags_set_clear() {
        let mut flags = ContentFlags::new(ContentFlags::NONE);
        assert!(!flags.has(ContentFlags::INSTALL));

        // Set flag
        flags.set(ContentFlags::INSTALL);
        assert!(flags.has(ContentFlags::INSTALL));

        // Set another flag
        flags.set(ContentFlags::BUNDLE);
        assert!(flags.has(ContentFlags::INSTALL));
        assert!(flags.has(ContentFlags::BUNDLE));

        // Clear one flag
        flags.clear(ContentFlags::INSTALL);
        assert!(!flags.has(ContentFlags::INSTALL));
        assert!(flags.has(ContentFlags::BUNDLE));

        // Clear remaining flag
        flags.clear(ContentFlags::BUNDLE);
        assert!(!flags.has(ContentFlags::BUNDLE));
    }

    #[test]
    fn test_content_flags_v1_v3_round_trip() {
        let original = ContentFlags::new(0x1234_5678);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_v1_v3(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = ContentFlags::read_v1_v3(&mut cursor).expect("Operation should succeed");

        assert_eq!(original, restored);
        assert_eq!(buffer.len(), 4);
    }

    #[test]
    fn test_content_flags_v1_v3_truncation() {
        // Test that 64-bit values are truncated to 32-bit for V1-V3
        let original = ContentFlags::new(0x1234_56789abcdef0);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_v1_v3(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = ContentFlags::read_v1_v3(&mut cursor).expect("Operation should succeed");

        // Should be truncated to lower 32 bits
        assert_eq!(restored.value, 0x9abc_def0);
        assert_ne!(original, restored);
    }

    #[test]
    fn test_content_flags_v4_round_trip() {
        let original = ContentFlags::new(0x1234_567890); // 40-bit value

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_v4(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = ContentFlags::read_v4(&mut cursor).expect("Operation should succeed");

        assert_eq!(original, restored);
        assert_eq!(buffer.len(), 5); // 4 + 1 bytes
    }

    #[test]
    fn test_content_flags_v4_truncation() {
        // Test that values larger than 40-bit are truncated
        let original = ContentFlags::new(0xff00_000000ff); // 48-bit value

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_v4(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = ContentFlags::read_v4(&mut cursor).expect("Operation should succeed");

        // Should be truncated to 40 bits (5 bytes) - keeps lower 40 bits
        assert_eq!(restored.value, 0x0000_000000ff);
    }

    #[test]
    fn test_content_flags_conversion_traits() {
        let flags_from_u32 = ContentFlags::from(0x1234_5678u32);
        assert_eq!(flags_from_u32.value, 0x1234_5678);

        let flags_from_u64 = ContentFlags::from(0x1234_56789abcdef0u64);
        assert_eq!(flags_from_u64.value, 0x1234_56789abcdef0);
    }

    #[test]
    fn test_content_flags_display() {
        let flags = ContentFlags::new(0x1234_5678);
        let display_string = format!("{flags}");
        assert_eq!(display_string, "0x12345678");
    }

    #[test]
    fn test_locale_flags_basic() {
        let flags = LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE);

        assert!(flags.has(LocaleFlags::ENUS));
        assert!(flags.has(LocaleFlags::DEDE));
        assert!(!flags.has(LocaleFlags::FRFR));
    }

    #[test]
    fn test_locale_flags_all_locales() {
        let all_locales = [
            LocaleFlags::ENUS,
            LocaleFlags::KOKR,
            LocaleFlags::FRFR,
            LocaleFlags::DEDE,
            LocaleFlags::ZHCN,
            LocaleFlags::ESES,
            LocaleFlags::ZHTW,
            LocaleFlags::ENGB,
            LocaleFlags::PTBR,
            LocaleFlags::ITIT,
            LocaleFlags::RURU,
        ];

        for &locale in &all_locales {
            let flags = LocaleFlags::new(locale);
            assert!(flags.has(locale));

            // Test that other locales are not set
            for &other_locale in &all_locales {
                if other_locale != locale {
                    assert!(!flags.has(other_locale));
                }
            }
        }
    }

    #[test]
    fn test_locale_flags_all_flag() {
        let all_flags = LocaleFlags::new(LocaleFlags::ALL);

        // ALL flag should match any locale
        let test_locales = [
            LocaleFlags::ENUS,
            LocaleFlags::DEDE,
            LocaleFlags::FRFR,
            LocaleFlags::ZHCN,
        ];

        for &locale in &test_locales {
            assert!(all_flags.has(locale));
            assert!(all_flags.matches(LocaleFlags::new(locale)));
        }
    }

    #[test]
    fn test_locale_flags_set_clear() {
        let mut flags = LocaleFlags::new(0);
        assert!(!flags.has(LocaleFlags::ENUS));

        // Set locale
        flags.set(LocaleFlags::ENUS);
        assert!(flags.has(LocaleFlags::ENUS));

        // Set another locale
        flags.set(LocaleFlags::DEDE);
        assert!(flags.has(LocaleFlags::ENUS));
        assert!(flags.has(LocaleFlags::DEDE));

        // Clear one locale
        flags.clear(LocaleFlags::ENUS);
        assert!(!flags.has(LocaleFlags::ENUS));
        assert!(flags.has(LocaleFlags::DEDE));
    }

    #[test]
    fn test_locale_flags_matches() {
        let file_locales = LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE);
        let request_locales = LocaleFlags::new(LocaleFlags::ENUS);

        assert!(file_locales.matches(request_locales));

        let no_match = LocaleFlags::new(LocaleFlags::FRFR);
        assert!(!file_locales.matches(no_match));

        // Test empty locale doesn't match anything
        let empty = LocaleFlags::new(0);
        assert!(!empty.matches(request_locales));
        assert!(!request_locales.matches(empty));
    }

    #[test]
    fn test_locale_flags_bitwise_ops() {
        let flags1 = LocaleFlags::new(LocaleFlags::ENUS);
        let flags2 = LocaleFlags::new(LocaleFlags::DEDE);

        let combined = flags1 | flags2;
        assert_eq!(combined.value(), LocaleFlags::ENUS | LocaleFlags::DEDE);

        let intersection = combined & flags1;
        assert_eq!(intersection.value(), LocaleFlags::ENUS);

        // Test intersection with no overlap
        let flags3 = LocaleFlags::new(LocaleFlags::FRFR);
        let no_intersection = combined & flags3;
        assert_eq!(no_intersection.value(), 0);
    }

    #[test]
    fn test_locale_flags_conversion_traits() {
        let flags_from_u32 = LocaleFlags::from(0x1234_5678u32);
        assert_eq!(flags_from_u32.value(), 0x1234_5678);
    }

    #[test]
    fn test_locale_flags_display() {
        let flags = LocaleFlags::new(0x1234_5678);
        let display_string = format!("{flags}");
        assert_eq!(display_string, "0x12345678");
    }

    #[test]
    fn test_locale_flags_round_trip() {
        let original = LocaleFlags::new(0x1234_5678);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        original
            .write_le(&mut cursor)
            .expect("Operation should succeed");

        let mut cursor = Cursor::new(&buffer);
        let restored = LocaleFlags::read_le(&mut cursor).expect("Operation should succeed");

        assert_eq!(original, restored);
        assert_eq!(buffer.len(), 4);
    }
}
