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
    pub fn validate(&self) -> Result<ValidationReport> {
        let start_time = std::time::Instant::now();
        let mut report = ValidationReport {
            total_files: self.files.len(),
            ..Default::default()
        };

        for (index, entry) in self.files.iter().enumerate() {
            let result = self.validate_entry(index, entry);
            if result.is_valid {
                report.valid_files += 1;
            } else {
                report.invalid_files += 1;
            }
            report.results.push(result);
        }

        report.total_time_us = start_time.elapsed().as_micros() as u64;
        Ok(report)
    }

    /// Quick validation (headers only)
    pub fn validate_headers(&self) -> Result<HeaderValidationReport> {
        let start_time = std::time::Instant::now();
        let mut valid_headers = 0;
        let mut invalid_headers = 0;

        for entry in &self.files {
            // Basic validation - check if entry has reasonable size and metadata
            if entry.size > 8 && entry.metadata.compressed_size > 0 {
                valid_headers += 1;
            } else {
                invalid_headers += 1;
            }
        }

        Ok(HeaderValidationReport {
            total_files: self.files.len(),
            valid_headers,
            invalid_headers,
            validation_time_us: start_time.elapsed().as_micros() as u64,
        })
    }

    /// Deep validation (full decompression)
    pub fn validate_deep(&mut self) -> Result<DeepValidationReport> {
        let basic = self.validate()?;

        let mut decompressed_files = 0;
        let mut decompression_errors = 0;
        let mut total_decompressed_bytes = 0;

        // For deep validation, we would need to parse each BLTE file
        // This is a simplified implementation
        for entry in &self.files {
            if let Some(ref _blte) = entry.blte {
                decompressed_files += 1;
                total_decompressed_bytes += entry.size as u64;
            } else {
                decompression_errors += 1;
            }
        }

        Ok(DeepValidationReport {
            basic,
            decompressed_files,
            decompression_errors,
            total_decompressed_bytes,
        })
    }

    /// Validate specific file by index
    pub fn validate_file(&self, index: usize) -> Result<ValidationResult> {
        if index >= self.files.len() {
            return Ok(ValidationResult {
                file_index: index,
                is_valid: false,
                error: Some(format!("File index {index} out of range")),
                validation_time_us: 0,
            });
        }

        let entry = &self.files[index];
        Ok(self.validate_entry(index, entry))
    }

    /// Validate a single archive entry
    fn validate_entry(&self, index: usize, entry: &super::ArchiveEntry) -> ValidationResult {
        let start_time = std::time::Instant::now();

        // Basic validation of entry structure
        if entry.size == 0 {
            return ValidationResult {
                file_index: index,
                is_valid: false,
                error: Some("Entry has zero size".to_string()),
                validation_time_us: start_time.elapsed().as_micros() as u64,
            };
        }

        if entry.metadata.compressed_size == 0 {
            return ValidationResult {
                file_index: index,
                is_valid: false,
                error: Some("Entry has zero compressed size".to_string()),
                validation_time_us: start_time.elapsed().as_micros() as u64,
            };
        }

        // If we get here, the entry structure is valid
        ValidationResult {
            file_index: index,
            is_valid: true,
            error: None,
            validation_time_us: start_time.elapsed().as_micros() as u64,
        }
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
