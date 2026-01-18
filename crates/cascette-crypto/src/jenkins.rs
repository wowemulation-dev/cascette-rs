//! Jenkins96 hash implementation for CASC archive indices
//!
//! This is a port of Bob Jenkins' lookup3.c hash function used by CASC
//! for archive index lookups and legacy .idx v1/v2 format validation.
//!
//! Provides both `hashlittle()` (single 32-bit hash) and `hashlittle2()`
//! (dual 32-bit hashes) for compatibility with all CASC formats from `WoW` 6.0.x onwards.

use std::fmt;

/// Jenkins96 hash result containing both 64-bit and 32-bit components
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Jenkins96 {
    /// Primary 64-bit hash value
    pub hash64: u64,
    /// Secondary 32-bit hash value
    pub hash32: u32,
}

impl Jenkins96 {
    /// Compute Jenkins96 hash of data
    pub fn hash(data: &[u8]) -> Self {
        let mut pc = 0u32;
        let mut pb = 0u32;
        hashlittle2_impl(data, &mut pc, &mut pb);

        // Combine into 64-bit value (pc is high, pb is low)
        let hash64 = (u64::from(pc) << 32) | u64::from(pb);

        Self { hash64, hash32: pc }
    }

    /// Create from raw components
    pub fn from_parts(hash64: u64, hash32: u32) -> Self {
        Self { hash64, hash32 }
    }
}

impl fmt::Display for Jenkins96 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}:{:08x}", self.hash64, self.hash32)
    }
}

/// Compute Jenkins hash producing single 32-bit value
///
/// This is the `hashlittle()` function from Bob Jenkins' lookup3.c.
/// Used by CASC for legacy .idx v1/v2 format `FILE_INDEX_GUARDED_BLOCK` validation.
///
/// # Arguments
///
/// * `data` - Data to hash
/// * `initval` - Initial value (typically 0)
///
/// # Returns
///
/// Single 32-bit hash value
///
/// # Examples
///
/// ```
/// use cascette_crypto::jenkins::hashlittle;
///
/// let hash = hashlittle(b"test data", 0);
/// assert_ne!(hash, 0);
/// ```
pub fn hashlittle(data: &[u8], initval: u32) -> u32 {
    // Initialize state
    let mut a = 0xdead_beef_u32
        .wrapping_add(u32::try_from(data.len()).unwrap_or(u32::MAX))
        .wrapping_add(initval);
    let mut b = a;
    let mut c = a;
    let mut k = data;

    if k.is_empty() {
        return c;
    }

    // Process 12-byte chunks
    while k.len() > 12 {
        a = a.wrapping_add(u32::from_le_bytes([k[0], k[1], k[2], k[3]]));
        b = b.wrapping_add(u32::from_le_bytes([k[4], k[5], k[6], k[7]]));
        c = c.wrapping_add(u32::from_le_bytes([k[8], k[9], k[10], k[11]]));
        mix(&mut a, &mut b, &mut c);
        k = &k[12..];
    }

    // Handle last chunk (0-12 bytes)
    match k.len() {
        12 => {
            c = c.wrapping_add(u32::from(k[11]) << 24);
            c = c.wrapping_add(u32::from(k[10]) << 16);
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        11 => {
            c = c.wrapping_add(u32::from(k[10]) << 16);
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        10 => {
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        9 => {
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        8 => {
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        7 => {
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        6 => {
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        5 => {
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        4 => {
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        3 => {
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        2 => {
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        1 => {
            a = a.wrapping_add(u32::from(k[0]));
        }
        0 => {
            return c;
        }
        _ => unreachable!("k.len() should be <= 12"),
    }

    final_mix(&mut a, &mut b, &mut c);
    c
}

/// Compute Jenkins hash producing two 32-bit values
///
/// This is the `hashlittle2()` function from Bob Jenkins' lookup3.c.
/// Used by CASC for legacy .idx v1/v2 format validation.
///
/// # Arguments
///
/// * `data` - Data to hash
/// * `pc` - Primary hash value (input/output)
/// * `pb` - Secondary hash value (input/output)
///
/// # Examples
///
/// ```
/// use cascette_crypto::jenkins::hashlittle2;
///
/// let mut pc = 0u32;
/// let mut pb = 0u32;
/// hashlittle2(b"test data", &mut pc, &mut pb);
/// assert_ne!(pc, 0);
/// assert_ne!(pb, 0);
/// ```
pub fn hashlittle2(key: &[u8], pc: &mut u32, pb: &mut u32) {
    hashlittle2_impl(key, pc, pb);
}

/// Mix 3 u32 values reversibly
fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(4);
    *c = c.wrapping_add(*b);

    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(6);
    *a = a.wrapping_add(*c);

    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(8);
    *b = b.wrapping_add(*a);

    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(16);
    *c = c.wrapping_add(*b);

    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(19);
    *a = a.wrapping_add(*c);

    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(4);
    *b = b.wrapping_add(*a);
}

/// Final mixing of 3 u32 values
fn final_mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(14));

    *a ^= *c;
    *a = a.wrapping_sub(c.rotate_left(11));

    *b ^= *a;
    *b = b.wrapping_sub(a.rotate_left(25));

    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(16));

    *a ^= *c;
    *a = a.wrapping_sub(c.rotate_left(4));

    *b ^= *a;
    *b = b.wrapping_sub(a.rotate_left(14));

    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(24));
}

/// Internal implementation of hashlittle2
fn hashlittle2_impl(key: &[u8], pc: &mut u32, pb: &mut u32) {
    let mut a = 0xdead_beef_u32
        .wrapping_add(u32::try_from(key.len()).unwrap_or(u32::MAX))
        .wrapping_add(*pc);
    let mut b = a;
    let mut c = a.wrapping_add(*pb);
    let mut k = key;

    if k.is_empty() {
        *pc = c;
        *pb = b;
        return;
    }

    // Process 12-byte chunks
    while k.len() > 12 {
        a = a.wrapping_add(u32::from_le_bytes([k[0], k[1], k[2], k[3]]));
        b = b.wrapping_add(u32::from_le_bytes([k[4], k[5], k[6], k[7]]));
        c = c.wrapping_add(u32::from_le_bytes([k[8], k[9], k[10], k[11]]));
        mix(&mut a, &mut b, &mut c);
        k = &k[12..];
    }

    // Handle last chunk (0-12 bytes)
    // Must match Python byte-by-byte logic - only add bytes that actually exist
    match k.len() {
        12 => {
            c = c.wrapping_add(u32::from(k[11]) << 24);
            c = c.wrapping_add(u32::from(k[10]) << 16);
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        11 => {
            c = c.wrapping_add(u32::from(k[10]) << 16);
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        10 => {
            c = c.wrapping_add(u32::from(k[9]) << 8);
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        9 => {
            c = c.wrapping_add(u32::from(k[8]));
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        8 => {
            b = b.wrapping_add(u32::from(k[7]) << 24);
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        7 => {
            b = b.wrapping_add(u32::from(k[6]) << 16);
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        6 => {
            b = b.wrapping_add(u32::from(k[5]) << 8);
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        5 => {
            b = b.wrapping_add(u32::from(k[4]));
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        4 => {
            a = a.wrapping_add(u32::from(k[3]) << 24);
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        3 => {
            a = a.wrapping_add(u32::from(k[2]) << 16);
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        2 => {
            a = a.wrapping_add(u32::from(k[1]) << 8);
            a = a.wrapping_add(u32::from(k[0]));
        }
        1 => {
            a = a.wrapping_add(u32::from(k[0]));
        }
        0 => {
            // Zero-length remaining - return without final mixing (matches Python)
            *pc = c;
            *pb = b;
            return;
        }
        _ => unreachable!("k.len() should be <= 12"),
    }

    final_mix(&mut a, &mut b, &mut c);

    *pc = c;
    *pb = b;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jenkins96_empty() {
        let hash = Jenkins96::hash(b"");
        // Empty string should produce deterministic output
        assert_ne!(hash.hash64, 0);
        assert_ne!(hash.hash32, 0);
    }

    #[test]
    fn test_jenkins96_consistent() {
        let data = b"test data";
        let hash1 = Jenkins96::hash(data);
        let hash2 = Jenkins96::hash(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_jenkins96_different() {
        let hash1 = Jenkins96::hash(b"test1");
        let hash2 = Jenkins96::hash(b"test2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_jenkins96_display() {
        let hash = Jenkins96::from_parts(0x1234_5678_9abc_def0, 0x1122_3344);
        assert_eq!(format!("{hash}"), "123456789abcdef0:11223344");
    }

    #[test]
    fn test_hashlittle_basic() {
        // Test basic hashlittle() functionality
        let result = hashlittle(b"", 0);
        assert_eq!(result, 0xdead_beef);

        let result = hashlittle(b"Four score and seven years ago", 0);
        assert_eq!(result, 0x1777_0551);

        let result = hashlittle(b"Four score and seven years ago", 1);
        assert_eq!(result, 0xcd62_8161);
    }

    #[test]
    fn test_hashlittle2_basic() {
        // Test basic hashlittle2() functionality
        let mut pc = 0u32;
        let mut pb = 0u32;
        hashlittle2(b"", &mut pc, &mut pb);
        assert_eq!(pc, 0xdead_beef);
        assert_eq!(pb, 0xdead_beef);

        let mut pc = 0u32;
        let mut pb = 0u32;
        hashlittle2(b"Four score and seven years ago", &mut pc, &mut pb);
        assert_eq!(pc, 0x1777_0551);
        assert_eq!(pb, 0xce72_26e6);

        let mut pc = 1u32;
        let mut pb = 0u32;
        hashlittle2(b"Four score and seven years ago", &mut pc, &mut pb);
        assert_eq!(pc, 0xcd62_8161);
        assert_eq!(pb, 0x6cbe_a4b3);
    }

    #[test]
    fn test_hashlittle_lengths() {
        // Test different data lengths (edge cases at 12-byte boundaries)
        // Values verified against Python cascette_tools.crypto.jenkins implementation
        let test_cases = vec![
            (b"" as &[u8], 0xdead_beef),
            (b"a", 0x58d6_8708),
            (b"ab", 0xfbb3_a8df),
            (b"abc", 0x0e39_7631),
            (b"abcd", 0xb5f4_889c),
            (b"abcde", 0x026d_72de),
            (b"abcdef", 0xd6fa_502e),
            (b"abcdefg", 0xb11a_d4a5),
            (b"abcdefgh", 0x2995_c3be),
            (b"abcdefghi", 0xac65_72b4),
            (b"abcdefghij", 0x8bf7_d2ef),
            (b"abcdefghijk", 0x5f61_edf8),
            (b"abcdefghijkl", 0x4012_f87b),  // Exactly 12 bytes
            (b"abcdefghijklm", 0x9281_28f9), // 13 bytes - crosses boundary
        ];

        for (data, expected) in test_cases {
            let result = hashlittle(data, 0);
            assert_eq!(
                result,
                expected,
                "Hash mismatch for {:?} (len={}): got 0x{:08x}, expected 0x{:08x}",
                String::from_utf8_lossy(data),
                data.len(),
                result,
                expected
            );
        }
    }

    #[test]
    fn test_hashlittle_consistency() {
        // Verify hash is deterministic
        let data = b"test data for consistency check";
        let hash1 = hashlittle(data, 0);
        let hash2 = hashlittle(data, 0);
        assert_eq!(hash1, hash2);

        let hash3 = hashlittle(data, 42);
        let hash4 = hashlittle(data, 42);
        assert_eq!(hash3, hash4);

        // Different initval should produce different result
        assert_ne!(hash1, hash3);
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_hashlittle2_consistency() {
        // Verify hashlittle2 is deterministic
        let data = b"test data for consistency check";

        let mut pc_first = 0u32;
        let mut pb_first = 0u32;
        hashlittle2(data, &mut pc_first, &mut pb_first);

        let mut pc_second = 0u32;
        let mut pb_second = 0u32;
        hashlittle2(data, &mut pc_second, &mut pb_second);

        assert_eq!(pc_first, pc_second);
        assert_eq!(pb_first, pb_second);
    }

    #[test]
    fn test_hashlittle_vs_hashlittle2() {
        // Verify hashlittle() returns the same as hashlittle2's pc value
        let test_cases = vec![
            b"" as &[u8],
            b"a",
            b"test",
            b"Four score and seven years ago",
            b"abcdefghijklmnopqrstuvwxyz",
        ];

        for data in test_cases {
            let hash1 = hashlittle(data, 0);

            let mut pc = 0u32;
            let mut pb = 0u32;
            hashlittle2(data, &mut pc, &mut pb);

            assert_eq!(
                hash1,
                pc,
                "hashlittle mismatch with hashlittle2 for {:?}",
                String::from_utf8_lossy(data)
            );
        }
    }
}
