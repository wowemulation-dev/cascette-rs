//! Random key and IV generation for creating encrypted CASC content.
//!
//! Agent.exe delegates random number generation to the Windows
//! `BCryptGenRandom` API. This module uses the platform-neutral
//! [`getrandom`] crate which maps to the same OS primitive on each
//! platform (`getrandom(2)` on Linux, `BCryptGenRandom` on Windows,
//! `SecRandomCopyBytes` on macOS).
//!
//! These functions are needed when creating custom encrypted CASC content
//! (modding). They are not required for decryption of existing content.

use crate::error::CryptoError;
use crate::keys::TactKey;

/// Generate a random 16-byte TACT encryption key with the given ID.
///
/// The returned key is suitable for use with [`crate::Salsa20Cipher`].
///
/// # Errors
///
/// Returns [`CryptoError::RandomGenerationFailed`] if the OS CSPRNG
/// is unavailable.
///
/// # Examples
///
/// ```
/// use cascette_crypto::generate::generate_tact_key;
///
/// let key = generate_tact_key(0xDEAD_BEEF_1234_5678).expect("RNG available");
/// assert_eq!(key.id, 0xDEAD_BEEF_1234_5678);
/// assert_eq!(key.key.len(), 16);
/// ```
pub fn generate_tact_key(id: u64) -> Result<TactKey, CryptoError> {
    let mut key = [0u8; 16];
    getrandom::fill(&mut key).map_err(|e| {
        CryptoError::RandomGenerationFailed(format!("failed to generate TACT key: {e}"))
    })?;
    Ok(TactKey::new(id, key))
}

/// Generate a random 8-byte Salsa20 IV.
///
/// The IV is used as the `iv` argument to [`crate::Salsa20Cipher::new`].
/// Each piece of encrypted content should use a unique IV.
///
/// # Errors
///
/// Returns [`CryptoError::RandomGenerationFailed`] if the OS CSPRNG
/// is unavailable.
///
/// # Examples
///
/// ```
/// use cascette_crypto::generate::generate_salsa20_iv;
/// use cascette_crypto::Salsa20Cipher;
///
/// let tact_key = [0u8; 16]; // In practice, load from keyring
/// let iv = generate_salsa20_iv().expect("RNG available");
/// let cipher = Salsa20Cipher::new(&tact_key, &iv, 0).expect("valid IV");
/// ```
pub fn generate_salsa20_iv() -> Result<[u8; 8], CryptoError> {
    let mut iv = [0u8; 8];
    getrandom::fill(&mut iv).map_err(|e| {
        CryptoError::RandomGenerationFailed(format!("failed to generate Salsa20 IV: {e}"))
    })?;
    Ok(iv)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn generate_tact_key_has_correct_id() {
        let key = generate_tact_key(0x1234_5678_9ABC_DEF0).expect("RNG available");
        assert_eq!(key.id, 0x1234_5678_9ABC_DEF0);
    }

    #[test]
    fn generate_tact_key_produces_16_bytes() {
        let key = generate_tact_key(0).expect("RNG available");
        // Probability of all zeros from a CSPRNG is negligible (2^-128).
        assert_ne!(key.key, [0u8; 16], "CSPRNG should not return all-zero key");
    }

    #[test]
    fn generate_tact_key_is_unique() {
        let a = generate_tact_key(1).expect("RNG available");
        let b = generate_tact_key(1).expect("RNG available");
        // Two independent calls should produce different keys.
        assert_ne!(a.key, b.key, "consecutive keys should differ");
    }

    #[test]
    fn generate_salsa20_iv_produces_8_bytes() {
        let iv = generate_salsa20_iv().expect("RNG available");
        // Probability of all zeros is negligible.
        assert_ne!(iv, [0u8; 8], "CSPRNG should not return all-zero IV");
    }

    #[test]
    fn generate_salsa20_iv_is_unique() {
        let a = generate_salsa20_iv().expect("RNG available");
        let b = generate_salsa20_iv().expect("RNG available");
        assert_ne!(a, b, "consecutive IVs should differ");
    }

    #[test]
    fn generated_iv_works_with_salsa20() {
        use crate::Salsa20Cipher;
        let key = generate_tact_key(0).expect("RNG available");
        let iv = generate_salsa20_iv().expect("RNG available");
        let result = Salsa20Cipher::new(&key.key, &iv, 0);
        assert!(
            result.is_ok(),
            "generated key+IV should be accepted by Salsa20"
        );
    }
}
