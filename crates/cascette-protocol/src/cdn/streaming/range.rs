//! HTTP range request support with optimization and coalescing
//!
//! Provides efficient handling of HTTP range requests including automatic coalescing
//! of adjacent ranges, validation, and optimization strategies for CDN streaming.

use std::fmt;

use super::{config::StreamingConfig, error::StreamingError};

/// HTTP range specification for partial content requests
///
/// Represents a byte range request as defined by RFC 7233.
/// Supports both inclusive ranges (start-end) and suffix ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpRange {
    /// Start byte offset (inclusive)
    pub start: u64,
    /// End byte offset (inclusive)
    pub end: u64,
}

impl HttpRange {
    /// Create a new range from start and end offsets (both inclusive)
    ///
    /// # Arguments
    /// * `start` - Starting byte offset (inclusive)
    /// * `end` - Ending byte offset (inclusive)
    ///
    /// # Returns
    /// A new HttpRange instance
    ///
    /// # Panics
    /// Panics if start > end
    pub fn new(start: u64, end: u64) -> Self {
        assert!(start <= end, "Range start must be <= end");
        Self { start, end }
    }

    /// Create a new range from offset and length
    ///
    /// # Arguments
    /// * `offset` - Starting byte offset
    /// * `length` - Number of bytes to request
    ///
    /// # Returns
    /// A new HttpRange instance
    ///
    /// # Panics
    /// Panics if length is 0 or if offset + length would overflow
    #[allow(clippy::panic)]
    pub fn from_offset_length(offset: u64, length: u64) -> Self {
        assert!(length > 0, "Range length must be > 0");
        let Some(end) = offset.checked_add(length - 1) else {
            panic!("Range offset + length overflow");
        };
        Self { start: offset, end }
    }

    /// Get the length of this range in bytes
    pub fn length(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Check if this range contains the given offset
    pub fn contains(&self, offset: u64) -> bool {
        offset >= self.start && offset <= self.end
    }

    /// Check if this range overlaps with another range
    pub fn overlaps(&self, other: &Self) -> bool {
        self.start <= other.end && self.end >= other.start
    }

    /// Check if this range is adjacent to another range
    ///
    /// Two ranges are adjacent if they can be merged without creating a gap.
    pub fn is_adjacent(&self, other: &Self) -> bool {
        // Adjacent if one ends where the other starts (allowing 1-byte gap)
        self.end + 1 == other.start || other.end + 1 == self.start
    }

    /// Get the gap size between this range and another
    ///
    /// Returns 0 if the ranges overlap or are adjacent.
    pub fn gap_to(&self, other: &Self) -> u64 {
        if self.overlaps(other) || self.is_adjacent(other) {
            return 0;
        }

        if self.end < other.start {
            other.start - self.end - 1
        } else {
            self.start - other.end - 1
        }
    }

    /// Merge this range with another overlapping or adjacent range
    ///
    /// # Arguments
    /// * `other` - The range to merge with
    ///
    /// # Returns
    /// A new range covering both input ranges
    ///
    /// # Panics
    /// Panics if the ranges don't overlap and aren't adjacent
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        assert!(
            self.overlaps(other) || self.is_adjacent(other),
            "Cannot merge non-overlapping, non-adjacent ranges"
        );

        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Convert this range to an HTTP Range header value
    ///
    /// # Returns
    /// A string suitable for use in an HTTP Range header
    pub fn to_header_value(&self) -> String {
        format!("bytes={}-{}", self.start, self.end)
    }

    /// Split this range into smaller chunks
    ///
    /// # Arguments
    /// * `max_chunk_size` - Maximum size for each chunk
    ///
    /// # Returns
    /// A vector of ranges, each no larger than max_chunk_size
    pub fn split(&self, max_chunk_size: u64) -> Vec<Self> {
        let mut chunks = Vec::new();
        let mut current_start = self.start;

        while current_start <= self.end {
            let chunk_end = (current_start + max_chunk_size - 1).min(self.end);
            chunks.push(Self::new(current_start, chunk_end));
            current_start = chunk_end + 1;
        }

        chunks
    }

    /// Validate this range against content length
    ///
    /// # Arguments
    /// * `content_length` - Total size of the content
    ///
    /// # Returns
    /// Ok(()) if the range is valid, or an error if invalid
    pub fn validate(&self, content_length: u64) -> Result<(), StreamingError> {
        if self.start >= content_length {
            return Err(StreamingError::InvalidRange {
                reason: format!(
                    "Range start {} >= content length {}",
                    self.start, content_length
                ),
            });
        }

        if self.end >= content_length {
            return Err(StreamingError::InvalidRange {
                reason: format!(
                    "Range end {} >= content length {}",
                    self.end, content_length
                ),
            });
        }

        Ok(())
    }
}

impl fmt::Display for HttpRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bytes {}-{} ({} bytes)",
            self.start,
            self.end,
            self.length()
        )
    }
}

/// Range coalescing engine for optimizing HTTP requests
///
/// Automatically combines small, nearby ranges into larger requests to reduce
/// HTTP overhead while respecting server limitations and configuration constraints.
#[derive(Debug)]
pub struct RangeCoalescer {
    config: StreamingConfig,
}

impl RangeCoalescer {
    /// Create a new range coalescer with the given configuration
    pub fn new(config: StreamingConfig) -> Self {
        Self { config }
    }

    /// Coalesce a set of ranges into an optimized request pattern
    ///
    /// # Arguments
    /// * `ranges` - Input ranges to coalesce
    ///
    /// # Returns
    /// Optimized ranges that cover the same data with fewer HTTP requests
    ///
    /// # Errors
    /// Returns error if coalescing fails due to configuration constraints
    pub fn coalesce(&self, mut ranges: Vec<HttpRange>) -> Result<Vec<HttpRange>, StreamingError> {
        if ranges.is_empty() {
            return Ok(ranges);
        }

        // Sort ranges by start position
        ranges.sort_by_key(|r| r.start);

        // Remove duplicates and merge overlapping ranges
        let mut merged: Vec<HttpRange> = Vec::new();
        for range in ranges {
            if let Some(last) = merged.last_mut() {
                if last.overlaps(&range) || last.is_adjacent(&range) {
                    *last = last.merge(&range);
                } else {
                    merged.push(range);
                }
            } else {
                merged.push(range);
            }
        }

        // Coalesce nearby ranges if the gap is small
        let mut coalesced: Vec<HttpRange> = Vec::new();
        for range in merged {
            if let Some(last) = coalesced.last_mut() {
                let gap = last.gap_to(&range);
                if gap <= self.config.range_coalesce_threshold {
                    *last = HttpRange::new(last.start, range.end);
                } else {
                    coalesced.push(range);
                }
            } else {
                coalesced.push(range);
            }
        }

        // Split large ranges if they exceed the maximum size
        let mut final_ranges = Vec::new();
        for range in coalesced {
            if range.length() > self.config.max_range_size {
                final_ranges.extend(range.split(self.config.max_range_size));
            } else {
                final_ranges.push(range);
            }
        }

        // Check if we've exceeded the maximum number of ranges
        if final_ranges.len() > self.config.max_ranges_per_request {
            return Err(StreamingError::RangeCoalescingFailed {
                reason: format!(
                    "Coalescing resulted in {} ranges, but maximum is {}",
                    final_ranges.len(),
                    self.config.max_ranges_per_request
                ),
            });
        }

        Ok(final_ranges)
    }

    /// Estimate the efficiency gain from coalescing
    ///
    /// # Arguments
    /// * `original` - Original ranges before coalescing
    /// * `coalesced` - Coalesced ranges after optimization
    ///
    /// # Returns
    /// A tuple of (request_reduction_ratio, byte_efficiency_ratio)
    /// where both ratios are between 0.0 and 1.0
    pub fn efficiency_gain(&self, original: &[HttpRange], coalesced: &[HttpRange]) -> (f64, f64) {
        if original.is_empty() || coalesced.is_empty() {
            return (0.0, 0.0);
        }

        // Request reduction: how much did we reduce the number of requests?
        #[allow(clippy::cast_precision_loss)]
        let request_reduction = 1.0 - (coalesced.len() as f64 / original.len() as f64);

        // Byte efficiency: what percentage of requested bytes are actually needed?
        let original_bytes: u64 = original.iter().map(HttpRange::length).sum();
        let coalesced_bytes: u64 = coalesced.iter().map(HttpRange::length).sum();

        let byte_efficiency = if coalesced_bytes > 0 {
            #[allow(clippy::cast_precision_loss)]
            {
                original_bytes as f64 / coalesced_bytes as f64
            }
        } else {
            0.0
        };

        (request_reduction, byte_efficiency)
    }
}

/// Multi-range HTTP request builder
///
/// Constructs HTTP requests with multiple ranges according to RFC 7233.
/// Handles the complexity of multipart range requests and response parsing.
#[derive(Debug)]
pub struct MultiRangeRequest {
    ranges: Vec<HttpRange>,
    url: String,
}

impl MultiRangeRequest {
    /// Create a new multi-range request
    ///
    /// # Arguments
    /// * `url` - Target URL for the request
    /// * `ranges` - Ranges to request
    ///
    /// # Returns
    /// A new MultiRangeRequest instance
    pub fn new(url: String, ranges: Vec<HttpRange>) -> Self {
        Self { ranges, url }
    }

    /// Get the HTTP Range header value for this request
    ///
    /// # Returns
    /// A Range header value supporting multiple ranges
    pub fn to_header_value(&self) -> String {
        if self.ranges.len() == 1 {
            self.ranges[0].to_header_value()
        } else {
            let range_specs: Vec<String> = self
                .ranges
                .iter()
                .map(|r| format!("{}-{}", r.start, r.end))
                .collect();
            format!("bytes={}", range_specs.join(","))
        }
    }

    /// Get the total number of bytes that will be requested
    pub fn total_bytes(&self) -> u64 {
        self.ranges.iter().map(HttpRange::length).sum()
    }

    /// Get the URL for this request
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the ranges for this request
    pub fn ranges(&self) -> &[HttpRange] {
        &self.ranges
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;

    #[test]
    fn test_http_range_creation() {
        let range = HttpRange::new(0, 1023);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 1023);
        assert_eq!(range.length(), 1024);

        let range2 = HttpRange::from_offset_length(1024, 2048);
        assert_eq!(range2.start, 1024);
        assert_eq!(range2.end, 3071);
        assert_eq!(range2.length(), 2048);
    }

    #[test]
    fn test_range_overlap_and_adjacency() {
        let range1 = HttpRange::new(0, 100);
        let range2 = HttpRange::new(50, 150); // Overlapping
        let range3 = HttpRange::new(101, 200); // Adjacent
        let range4 = HttpRange::new(300, 400); // Separate

        assert!(range1.overlaps(&range2));
        assert!(range1.is_adjacent(&range3));
        assert!(!range1.overlaps(&range4));
        assert!(!range1.is_adjacent(&range4));

        assert_eq!(range1.gap_to(&range4), 199);
        assert_eq!(range1.gap_to(&range3), 0); // Adjacent
    }

    #[test]
    fn test_range_merging() {
        let range1 = HttpRange::new(0, 100);
        let range2 = HttpRange::new(50, 150);
        let merged = range1.merge(&range2);

        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 150);
    }

    #[test]
    fn test_range_splitting() {
        let range = HttpRange::new(0, 10000);
        let chunks = range.split(3000);

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], HttpRange::new(0, 2999));
        assert_eq!(chunks[1], HttpRange::new(3000, 5999));
        assert_eq!(chunks[2], HttpRange::new(6000, 8999));
        assert_eq!(chunks[3], HttpRange::new(9000, 10000));
    }

    #[test]
    fn test_range_validation() {
        let range = HttpRange::new(0, 1023);

        assert!(range.validate(2048).is_ok());
        assert!(range.validate(1024).is_ok());
        assert!(range.validate(1023).is_err()); // End == content length
        assert!(range.validate(500).is_err()); // Range exceeds content
    }

    #[test]
    fn test_range_header_formatting() {
        let range = HttpRange::new(100, 199);
        assert_eq!(range.to_header_value(), "bytes=100-199");
    }

    #[test]
    fn test_range_coalescing() {
        let config = StreamingConfig::default();
        let coalescer = RangeCoalescer::new(config);

        let ranges = vec![
            HttpRange::new(0, 100),
            HttpRange::new(200, 300),         // Small gap, should coalesce
            HttpRange::new(100_000, 110_000), // Large gap (99KB > 65KB threshold), separate request
        ];

        let result = coalescer
            .coalesce(ranges)
            .expect("Operation should succeed");

        // Should merge first two ranges (gap of 99 bytes < 64KB threshold), keep third separate
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].start, 0);
        assert_eq!(result[0].end, 300);
        assert_eq!(result[1], HttpRange::new(100_000, 110_000));
    }

    #[test]
    fn test_multi_range_request() {
        let ranges = vec![HttpRange::new(0, 100), HttpRange::new(200, 300)];

        let request = MultiRangeRequest::new("http://example.com/file".to_string(), ranges);

        assert_eq!(request.url(), "http://example.com/file");
        assert_eq!(request.ranges().len(), 2);
        assert_eq!(request.total_bytes(), 202);
        assert_eq!(request.to_header_value(), "bytes=0-100,200-300");
    }

    #[test]
    fn test_efficiency_calculation() {
        let config = StreamingConfig::default();
        let coalescer = RangeCoalescer::new(config);

        let original = vec![
            HttpRange::new(0, 100),
            HttpRange::new(200, 300),
            HttpRange::new(400, 500),
        ];

        let optimized_ranges = vec![
            HttpRange::new(0, 500), // Merged all ranges
        ];

        let (request_reduction, byte_efficiency) =
            coalescer.efficiency_gain(&original, &optimized_ranges);

        assert!((request_reduction - 0.6667).abs() < 0.001); // 3->1 requests = 66.67% reduction
        assert!((byte_efficiency - 0.603).abs() < 0.01); // 303/501 bytes efficiency
    }

    #[test]
    #[should_panic(expected = "Range start must be <= end")]
    fn test_invalid_range_creation() {
        HttpRange::new(100, 50);
    }

    #[test]
    #[should_panic(expected = "Range length must be > 0")]
    fn test_zero_length_range() {
        HttpRange::from_offset_length(100, 0);
    }
}
