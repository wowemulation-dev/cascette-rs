//! Round-trip validation framework for binary formats
//!
//! This module provides validation traits and testing utilities to ensure that
//! parse(build(data)) == data for all binary formats in the cascette-client-storage crate.
//! It includes property-based testing, error scenario validation, and performance benchmarks.

use crate::{Result, StorageError};
use binrw::{BinRead, BinWrite};
use std::fmt::Debug;
use std::io::Cursor;

/// Core trait for validating binary format round-trip operations
pub trait BinaryFormatValidator: BinRead + BinWrite + Clone + Debug + PartialEq {
    /// Generate a valid test instance for round-trip validation
    fn generate_valid_instance() -> Self;

    /// Generate multiple test instances with edge cases
    fn generate_edge_cases() -> Vec<Self> {
        vec![Self::generate_valid_instance()]
    }

    /// Validate that serialized data has expected properties
    ///
    /// # Errors
    ///
    /// Returns error if data validation fails
    fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
        // Default implementation just checks non-empty data
        if data.is_empty() {
            return Err(StorageError::InvalidFormat(
                "Empty serialized data".to_string(),
            ));
        }
        Ok(())
    }

    /// Perform round-trip validation: serialize → deserialize → compare
    ///
    /// # Errors
    ///
    /// Returns error if round-trip validation fails
    fn validate_round_trip(&self) -> Result<()>
    where
        for<'a> <Self as BinWrite>::Args<'a>: Default,
        for<'a> <Self as BinRead>::Args<'a>: Default,
    {
        // Serialize using big-endian (NGDP default)
        let mut serialized_data = Vec::new();
        let mut cursor = Cursor::new(&mut serialized_data);

        self.write_be(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Serialization failed: {e}")))?;

        // Validate serialized data properties
        self.validate_serialized_data(&serialized_data)?;

        // Deserialize back
        let mut cursor = Cursor::new(&serialized_data[..]);
        let deserialized = Self::read_be(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Deserialization failed: {e}")))?;

        // Compare original and deserialized
        if *self != deserialized {
            return Err(StorageError::Verification(format!(
                "Round-trip validation failed: original != deserialized\nOriginal: {self:#?}\nDeserialized: {deserialized:#?}"
            )));
        }

        Ok(())
    }

    /// Validate that all edge cases pass round-trip tests
    ///
    /// # Errors
    ///
    /// Returns error if edge case validation fails
    fn validate_all_edge_cases() -> Result<()>
    where
        for<'a> <Self as BinWrite>::Args<'a>: Default,
        for<'a> <Self as BinRead>::Args<'a>: Default,
    {
        let edge_cases = Self::generate_edge_cases();
        for (i, case) in edge_cases.iter().enumerate() {
            case.validate_round_trip()
                .map_err(|e| StorageError::Verification(format!("Edge case {i} failed: {e}")))?;
        }
        Ok(())
    }

    /// Validate with corrupted data (should fail gracefully)
    ///
    /// # Errors
    ///
    /// Returns error if corruption handling validation fails
    fn validate_corruption_handling() -> Result<()>
    where
        for<'a> <Self as BinWrite>::Args<'a>: Default,
        for<'a> <Self as BinRead>::Args<'a>: Default,
    {
        let valid_instance = Self::generate_valid_instance();

        // Serialize valid data
        let mut serialized_data = Vec::new();
        let mut cursor = Cursor::new(&mut serialized_data);
        valid_instance
            .write_be(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Serialization failed: {e}")))?;

        if serialized_data.is_empty() {
            return Ok(()); // Skip if no data to corrupt
        }

        // Test truncated data
        let truncated = &serialized_data[..serialized_data.len().saturating_sub(1)];
        let mut cursor = Cursor::new(truncated);
        let result = Self::read_be(&mut cursor);
        if result.is_ok() {
            return Err(StorageError::Verification(
                "Truncated data should fail deserialization".to_string(),
            ));
        }

        // Test magic byte corruption (if applicable)
        if serialized_data.len() >= 4 {
            let mut corrupted = serialized_data.clone();
            corrupted[0] = !corrupted[0]; // Flip first byte
            let mut cursor = Cursor::new(&corrupted[..]);
            let _result = Self::read_be(&mut cursor);
            // Note: Some formats might still parse successfully with flipped bytes
            // This test ensures the parser doesn't panic or produce undefined behavior
        }

        Ok(())
    }
}

/// Validation statistics for performance tracking
#[derive(Debug, Clone, Default)]
pub struct ValidationStats {
    /// Total number of round-trip tests performed
    pub round_trip_count: usize,
    /// Total serialization time in nanoseconds
    pub total_serialize_time: u128,
    /// Total deserialization time in nanoseconds
    pub total_deserialize_time: u128,
    /// Average serialized size in bytes
    pub average_size: f64,
    /// Number of edge cases tested
    pub edge_cases_tested: usize,
    /// Number of corruption tests performed
    pub corruption_tests: usize,
}

impl ValidationStats {
    /// Calculate average serialization time
    pub fn average_serialize_time(&self) -> f64 {
        if self.round_trip_count == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                let serialize_time = self.total_serialize_time as f64;
                let count = self.round_trip_count as f64;
                serialize_time / count
            }
        }
    }

    /// Calculate average deserialization time
    pub fn average_deserialize_time(&self) -> f64 {
        if self.round_trip_count == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                let deserialize_time = self.total_deserialize_time as f64;
                let count = self.round_trip_count as f64;
                deserialize_time / count
            }
        }
    }

    /// Print validation report
    pub fn print_report(&self, format_name: &str) {
        println!("\n=== Validation Report: {format_name} ===");
        println!("Round-trip tests: {}", self.round_trip_count);
        println!("Edge cases tested: {}", self.edge_cases_tested);
        println!("Corruption tests: {}", self.corruption_tests);
        println!("Average size: {:.2} bytes", self.average_size);
        println!(
            "Average serialize time: {:.2} ns",
            self.average_serialize_time()
        );
        println!(
            "Average deserialize time: {:.2} ns",
            self.average_deserialize_time()
        );
    }
}

/// Validation runner for a specific binary format
pub struct FormatValidator<T: BinaryFormatValidator> {
    /// Validation statistics
    pub stats: ValidationStats,
    /// Format name for reporting
    pub format_name: String,
    /// Phantom data for type parameter
    _phantom: std::marker::PhantomData<T>,
}

impl<T: BinaryFormatValidator> FormatValidator<T> {
    /// Create a new format validator
    pub fn new(format_name: impl Into<String>) -> Self {
        Self {
            stats: ValidationStats::default(),
            format_name: format_name.into(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Run the validation suite
    ///
    /// # Errors
    ///
    /// Returns error if any validation test fails
    pub fn run_comprehensive_validation(&mut self) -> Result<()>
    where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        println!("Running comprehensive validation for {}", self.format_name);

        // Test basic round-trip
        self.test_basic_round_trip()?;

        // Test all edge cases
        self.test_edge_cases()?;

        // Test corruption handling
        self.test_corruption_handling()?;

        // Print final report
        self.stats.print_report(&self.format_name);

        Ok(())
    }

    /// Test basic round-trip validation
    fn test_basic_round_trip(&mut self) -> Result<()>
    where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        let instance = T::generate_valid_instance();

        let start = std::time::Instant::now();

        // Serialize
        let serialize_start = std::time::Instant::now();
        let mut serialized_data = Vec::new();
        let mut cursor = Cursor::new(&mut serialized_data);
        instance
            .write_be(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Serialization failed: {e}")))?;
        let serialize_time = serialize_start.elapsed().as_nanos();

        // Deserialize
        let deserialize_start = std::time::Instant::now();
        let mut cursor = Cursor::new(&serialized_data[..]);
        let deserialized = T::read_be(&mut cursor)
            .map_err(|e| StorageError::InvalidFormat(format!("Deserialization failed: {e}")))?;
        let deserialize_time = deserialize_start.elapsed().as_nanos();

        // Validate
        if instance != deserialized {
            return Err(StorageError::Verification(
                "Basic round-trip validation failed".to_string(),
            ));
        }

        // Update statistics
        self.stats.round_trip_count += 1;
        self.stats.total_serialize_time += serialize_time;
        self.stats.total_deserialize_time += deserialize_time;
        self.stats.average_size = if self.stats.round_trip_count == 1 {
            #[allow(clippy::cast_precision_loss)]
            let size = serialized_data.len() as f64;
            size
        } else {
            #[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]
            {
                let count_f64 = self.stats.round_trip_count as f64;
                let size_f64 = serialized_data.len() as f64;
                (self.stats.average_size * (count_f64 - 1.0) + size_f64) / count_f64
            }
        };

        println!(
            "  - Basic round-trip validation passed ({:.2} ms)",
            start.elapsed().as_secs_f64() * 1000.0
        );
        Ok(())
    }

    /// Test all edge cases
    fn test_edge_cases(&mut self) -> Result<()>
    where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        let edge_cases = T::generate_edge_cases();
        self.stats.edge_cases_tested = edge_cases.len();

        for (i, case) in edge_cases.iter().enumerate() {
            case.validate_round_trip()
                .map_err(|e| StorageError::Verification(format!("Edge case {i} failed: {e}")))?;

            self.stats.round_trip_count += 1;
        }

        println!("  - {} edge cases validated", edge_cases.len());
        Ok(())
    }

    /// Test corruption handling
    fn test_corruption_handling(&mut self) -> Result<()>
    where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        T::validate_corruption_handling()?;
        self.stats.corruption_tests += 1;

        println!("  - Corruption handling validated");
        Ok(())
    }
}

/// Property-based testing utilities
#[cfg(test)]
pub mod proptest_utils {
    use super::*;
    use proptest::prelude::*;

    /// Generate property-based tests for a binary format
    pub fn prop_test_round_trip<T>() -> impl Strategy<Value = T>
    where
        T: BinaryFormatValidator + Arbitrary + 'static,
    {
        any::<T>()
    }

    /// Validate round-trip property for any generated instance
    ///
    /// # Errors
    ///
    /// Returns error if serialization, deserialization, or validation fails.
    pub fn validate_round_trip_property<T: BinaryFormatValidator>(instance: &T) -> Result<()>
    where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        instance.validate_round_trip()
    }
}

/// Batch validation runner for multiple formats
pub struct BatchValidator {
    /// Individual format validators
    validators: Vec<Box<dyn Fn() -> Result<()>>>,
    /// Validation results
    results: Vec<(String, Result<()>)>,
}

impl BatchValidator {
    /// Create a new batch validator
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
            results: Vec::new(),
        }
    }

    /// Add a format validator to the batch
    pub fn add_validator<T: BinaryFormatValidator + 'static>(
        &mut self,
        format_name: impl Into<String>,
    ) where
        for<'a> <T as BinWrite>::Args<'a>: Default,
        for<'a> <T as BinRead>::Args<'a>: Default,
    {
        let name = format_name.into();
        let name_clone = name;

        let validator = Box::new(move || {
            let mut format_validator = FormatValidator::<T>::new(name_clone.clone());
            format_validator.run_comprehensive_validation()
        });

        self.validators.push(validator);
    }

    /// Run all validators and collect results
    ///
    /// # Errors
    ///
    /// Returns error if any validation fails
    pub fn run_all(&mut self) -> Result<()> {
        self.results.clear();
        let mut all_passed = true;

        println!("\nRunning batch validation for all binary formats...\n");

        for (i, validator) in self.validators.iter().enumerate() {
            let result = validator();
            let format_name = format!("Format #{}", i + 1);

            match &result {
                Ok(()) => println!("[PASS] {format_name} validation passed"),
                Err(e) => {
                    println!("[FAIL] {format_name} validation failed: {e}");
                    all_passed = false;
                }
            }

            self.results.push((format_name, result));
        }

        if all_passed {
            println!("\nAll binary format validations passed!");
            Ok(())
        } else {
            Err(StorageError::Verification(
                "One or more binary format validations failed".to_string(),
            ))
        }
    }

    /// Print validation summary
    pub fn print_summary(&self) {
        println!("\n=== Validation Summary ===");
        let total = self.results.len();
        let passed = self.results.iter().filter(|(_, r)| r.is_ok()).count();
        let failed = total - passed;

        println!("Total formats tested: {total}");
        println!("Passed: {passed}");
        println!("Failed: {failed}");

        if failed > 0 {
            println!("\nFailed formats:");
            for (name, result) in &self.results {
                if let Err(e) = result {
                    println!("  - {name}: {e}");
                }
            }
        }
    }
}

impl Default for BatchValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Test the validation framework itself
    #[derive(Debug, Clone, PartialEq)]
    struct TestFormat {
        value: u32,
    }

    impl BinRead for TestFormat {
        type Args<'a> = ();

        fn read_options<R: std::io::Read + std::io::Seek>(
            reader: &mut R,
            _endian: binrw::Endian,
            _args: Self::Args<'_>,
        ) -> binrw::BinResult<Self> {
            let value = binrw::BinRead::read_be(reader)?;
            Ok(Self { value })
        }
    }

    impl BinWrite for TestFormat {
        type Args<'a> = ();

        fn write_options<W: std::io::Write + std::io::Seek>(
            &self,
            writer: &mut W,
            _endian: binrw::Endian,
            _args: Self::Args<'_>,
        ) -> binrw::BinResult<()> {
            self.value.write_be(writer)
        }
    }

    impl BinaryFormatValidator for TestFormat {
        fn generate_valid_instance() -> Self {
            Self { value: 0x1234_5678 }
        }

        fn generate_edge_cases() -> Vec<Self> {
            vec![
                Self { value: 0 },
                Self { value: u32::MAX },
                Self { value: 0x1234_5678 },
            ]
        }

        fn validate_serialized_data(&self, data: &[u8]) -> Result<()> {
            if data.len() != 4 {
                return Err(StorageError::InvalidFormat(format!(
                    "Expected 4 bytes, got {}",
                    data.len()
                )));
            }
            Ok(())
        }
    }

    #[test]
    fn test_validation_framework() {
        let mut validator = FormatValidator::<TestFormat>::new("TestFormat");
        validator
            .run_comprehensive_validation()
            .expect("Comprehensive validation should succeed");

        assert!(validator.stats.round_trip_count > 0);
        assert!(validator.stats.edge_cases_tested > 0);
        assert!(validator.stats.corruption_tests > 0);
    }

    #[test]
    fn test_batch_validator() {
        let mut batch = BatchValidator::new();
        batch.add_validator::<TestFormat>("TestFormat");

        batch.run_all().expect("Batch validation should succeed");
        assert_eq!(batch.results.len(), 1);
        assert!(batch.results[0].1.is_ok());
    }

    #[test]
    fn test_round_trip_trait() {
        let instance = TestFormat::generate_valid_instance();
        instance
            .validate_round_trip()
            .expect("Round trip validation should succeed");

        TestFormat::validate_all_edge_cases().expect("Edge case validation should succeed");
        TestFormat::validate_corruption_handling()
            .expect("Corruption handling validation should succeed");
    }
}
