//! Test utilities for format round-trip testing
//!
//! This module provides shared test utilities to reduce code duplication
//! across format test modules.

use crate::CascFormat;
use std::fmt::Debug;

/// Test round-trip serialization for a format instance
///
/// Verifies that a format can be serialized and deserialized back
/// to an equivalent value.
///
/// # Arguments
/// * `original` - The original format instance to test
///
/// # Returns
/// * `Ok(())` if round-trip succeeds and values match
/// * `Err` if serialization, deserialization, or comparison fails
pub fn test_round_trip<T>(original: &T) -> Result<(), Box<dyn std::error::Error>>
where
    T: CascFormat + PartialEq + Debug,
{
    // Build the binary representation
    let data = original.build()?;

    // Parse it back
    let parsed = T::parse(&data)?;

    // Verify they match
    if original != &parsed {
        return Err(format!(
            "Round-trip verification failed:\nOriginal: {:?}\nParsed: {:?}",
            original, parsed
        )
        .into());
    }

    Ok(())
}

/// Test round-trip with existing binary data
///
/// Verifies that binary data can be parsed, rebuilt, and reparsed
/// to produce equivalent results.
///
/// # Arguments
/// * `data` - The binary data to test
///
/// # Returns
/// * `Ok(())` if round-trip succeeds
/// * `Err` if any step fails
pub fn test_round_trip_with_data<T>(data: &[u8]) -> Result<(), Box<dyn std::error::Error>>
where
    T: CascFormat + PartialEq + Debug,
{
    // Parse the original data
    let parsed = T::parse(data)?;

    // Build it back
    let rebuilt = parsed.build()?;

    // Parse again
    let reparsed = T::parse(&rebuilt)?;

    // Verify they match
    if parsed != reparsed {
        return Err(format!(
            "Round-trip with data failed:\nParsed: {:?}\nReparsed: {:?}",
            parsed, reparsed
        )
        .into());
    }

    Ok(())
}

/// Test build-parse cycle for formats without PartialEq
///
/// Verifies that a format can be built and then parsed successfully.
/// This doesn't verify the parsed result equals the original, but ensures
/// the build-parse cycle works without errors.
///
/// # Arguments
/// * `instance` - The format instance to test
///
/// # Returns
/// * `Ok(())` if build-parse succeeds
/// * `Err` if either building or parsing fails
pub fn test_build_parse<T>(instance: &T) -> Result<(), Box<dyn std::error::Error>>
where
    T: CascFormat,
{
    // Build the binary representation
    let data = instance.build()?;

    // Parse it back (just verify it works)
    T::parse(&data)?;

    Ok(())
}

/// Test that parsing invalid data fails appropriately
///
/// # Arguments
/// * `invalid_data` - Data that should fail to parse
///
/// # Returns
/// * `Ok(())` if parsing fails as expected
/// * `Err` if parsing unexpectedly succeeds
pub fn test_invalid_data_rejected<T>(invalid_data: &[u8]) -> Result<(), Box<dyn std::error::Error>>
where
    T: CascFormat,
{
    match T::parse(invalid_data) {
        Ok(_) => Err("Expected parsing to fail for invalid data, but it succeeded".into()),
        Err(_) => Ok(()), // Expected failure
    }
}

/// Helper to assert round-trip works for a format
///
/// This is a convenience macro for use in tests
#[macro_export]
macro_rules! assert_round_trip {
    ($value:expr) => {
        $crate::test_utils::test_round_trip(&$value).expect("Round-trip should succeed")
    };
}

/// Helper to assert round-trip works with data
#[macro_export]
macro_rules! assert_round_trip_data {
    ($type:ty, $data:expr) => {
        $crate::test_utils::test_round_trip_with_data::<$type>($data)
            .expect("Round-trip with data should succeed")
    };
}

/// Helper to assert build-parse cycle works
#[macro_export]
macro_rules! assert_build_parse {
    ($value:expr) => {
        $crate::test_utils::test_build_parse(&$value).expect("Build-parse cycle should succeed")
    };
}

/// Helper to assert invalid data is rejected
#[macro_export]
macro_rules! assert_invalid_data_rejected {
    ($type:ty, $data:expr) => {
        $crate::test_utils::test_invalid_data_rejected::<$type>($data)
            .expect("Invalid data should be rejected")
    };
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    // Simple test structure to verify utilities work
    #[derive(Debug, PartialEq)]
    struct TestFormat {
        value: u32,
    }

    impl CascFormat for TestFormat {
        fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
            if data.len() != 4 {
                return Err("Invalid data length".into());
            }
            Ok(TestFormat {
                value: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            })
        }

        fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            Ok(self.value.to_le_bytes().to_vec())
        }
    }

    #[test]
    fn test_round_trip_utility() {
        let format = TestFormat { value: 42 };
        test_round_trip(&format).expect("Round-trip should succeed");
    }

    #[test]
    fn test_round_trip_with_data_utility() {
        let data = 42u32.to_le_bytes();
        test_round_trip_with_data::<TestFormat>(&data)
            .expect("Round-trip with data should succeed");
    }

    #[test]
    fn test_invalid_data_rejected_utility() {
        let invalid_data = vec![1, 2]; // Too short
        test_invalid_data_rejected::<TestFormat>(&invalid_data)
            .expect("Should reject invalid data");
    }

    #[test]
    fn test_round_trip_failure_detection() {
        // Test that failures are properly detected
        struct BadFormat;

        impl CascFormat for BadFormat {
            fn parse(_: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(BadFormat)
            }

            fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
                Err("Always fails".into())
            }
        }

        impl PartialEq for BadFormat {
            fn eq(&self, _: &Self) -> bool {
                true
            }
        }

        impl Debug for BadFormat {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "BadFormat")
            }
        }

        let format = BadFormat;
        assert!(test_round_trip(&format).is_err());
    }
}
