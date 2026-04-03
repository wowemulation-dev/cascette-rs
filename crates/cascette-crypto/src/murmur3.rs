//! MurmurHash3 fmix64 finalizer
//!
//! Agent.exe uses the fmix64 mixer from MurmurHash3 to distribute TACT key IDs
//! across its keyring hash map buckets. This module provides that single function.
//!
//! The fmix64 function is the finalizer stage of MurmurHash3_x64_128. It mixes
//! all bits of a 64-bit input to produce a well-distributed 64-bit output.

/// MurmurHash3 fmix64 bit mixer.
///
/// Applies the standard fmix64 finalizer from Austin Appleby's MurmurHash3.
/// This is used by Agent.exe to hash TACT key IDs for its keyring hash map.
///
/// # Examples
///
/// ```
/// use cascette_crypto::murmur3_fmix64;
///
/// let hash = murmur3_fmix64(0x1234_5678_9ABC_DEF0);
/// assert_ne!(hash, 0x1234_5678_9ABC_DEF0); // input bits are fully mixed
/// ```
pub fn murmur3_fmix64(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    k ^= k >> 33;
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmix64_zero_is_zero() {
        // 0 is a fixed point of fmix64: 0 ^ (0 >> 33) = 0, 0 * c = 0, etc.
        assert_eq!(murmur3_fmix64(0), 0);
    }

    #[test]
    fn fmix64_one() {
        // Verified against reference Python implementation:
        // def fmix64(k):
        //     k ^= k >> 33; k = (k * 0xff51afd7ed558ccd) & 0xffffffffffffffff
        //     k ^= k >> 33; k = (k * 0xc4ceb9fe1a85ec53) & 0xffffffffffffffff
        //     k ^= k >> 33; return k
        assert_eq!(murmur3_fmix64(1), 0xb456_bcfc_34c2_cb2c);
    }

    #[test]
    fn fmix64_deterministic_with_tact_key_ids() {
        // Real TACT key IDs should produce consistent output
        let id_a = 0xD31E_348B_3014_87DE_u64;
        let id_b = 0xB765_78E2_0033_1A3B_u64;

        let hash_a1 = murmur3_fmix64(id_a);
        let hash_a2 = murmur3_fmix64(id_a);
        let hash_b = murmur3_fmix64(id_b);

        assert_eq!(hash_a1, hash_a2, "fmix64 must be deterministic");
        assert_ne!(
            hash_a1, hash_b,
            "different inputs should produce different outputs"
        );
    }

    #[test]
    fn fmix64_avalanche() {
        // Flipping a single input bit should change at least 16 output bits.
        // This validates the avalanche property of the mixer.
        let base = 0xDEAD_BEEF_CAFE_BABE_u64;
        let base_hash = murmur3_fmix64(base);

        for bit in 0..64 {
            let flipped = base ^ (1u64 << bit);
            let flipped_hash = murmur3_fmix64(flipped);
            let diff_bits = (base_hash ^ flipped_hash).count_ones();
            assert!(
                diff_bits >= 16,
                "flipping bit {bit} changed only {diff_bits} output bits (expected >= 16)"
            );
        }
    }
}
