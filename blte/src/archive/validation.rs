//! Archive validation and integrity checking

use super::BLTEArchive;
use crate::Result;

/// Validation result for individual files
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// File index in archive
    pub file_index: usize,
    /// Whether the file is valid
    pub is_valid: bool,
    /// Validation error message if invalid
    pub error: Option<String>,
    /// Time taken to validate (in microseconds)
    pub validation_time_us: u64,
}

/// Complete validation report for archive
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    /// Total files validated
    pub total_files: usize,
    /// Number of valid files
    pub valid_files: usize,
    /// Number of invalid files
    pub invalid_files: usize,
    /// Individual file results
    pub results: Vec<ValidationResult>,
    /// Total validation time
    pub total_time_us: u64,
}

/// Header-only validation report (fast)
#[derive(Debug, Clone)]
pub struct HeaderValidationReport {
    /// Total files checked
    pub total_files: usize,
    /// Files with valid headers
    pub valid_headers: usize,
    /// Files with invalid headers
    pub invalid_headers: usize,
    /// Validation time
    pub validation_time_us: u64,
}

/// Deep validation report (includes decompression)
#[derive(Debug, Clone)]
pub struct DeepValidationReport {
    /// Basic validation report
    pub basic: ValidationReport,
    /// Files successfully decompressed
    pub decompressed_files: usize,
    /// Files with decompression errors
    pub decompression_errors: usize,
    /// Total decompressed bytes
    pub total_decompressed_bytes: u64,
}

impl BLTEArchive {
    /// Validate all BLTE files in archive
    pub fn validate(&mut self) -> Result<ValidationReport> {
        // TODO: Implement archive validation
        todo!("Archive validation not yet implemented")
    }

    /// Quick validation (headers only)
    pub fn validate_headers(&self) -> Result<HeaderValidationReport> {
        // TODO: Implement header validation
        todo!("Header validation not yet implemented")
    }

    /// Deep validation (full decompression)
    pub fn validate_deep(&mut self) -> Result<DeepValidationReport> {
        // TODO: Implement deep validation
        todo!("Deep validation not yet implemented")
    }

    /// Validate specific file by index
    pub fn validate_file(&mut self, _index: usize) -> Result<ValidationResult> {
        // TODO: Implement single file validation
        todo!("Single file validation not yet implemented")
    }
}

impl ValidationReport {
    /// Check if all files are valid
    pub fn is_all_valid(&self) -> bool {
        self.invalid_files == 0 && self.total_files > 0
    }

    /// Get validation success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.valid_files as f64 / self.total_files as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_report_creation() {
        let report = ValidationReport::default();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.valid_files, 0);
        assert_eq!(report.invalid_files, 0);
        assert!(report.results.is_empty());
    }

    #[test]
    fn test_success_rate_calculation() {
        let report = ValidationReport {
            total_files: 10,
            valid_files: 8,
            invalid_files: 2,
            ..Default::default()
        };

        assert_eq!(report.success_rate(), 80.0);
        assert!(!report.is_all_valid());
    }
}
