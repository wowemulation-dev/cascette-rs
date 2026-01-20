//! Cache access pattern analysis for NGDP game downloads
//!
//! This module provides cache access pattern analysis to understand
//! NGDP game download cache usage patterns.
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::cast_lossless)] // u32/u8 to u64 casts are safe
#![allow(clippy::single_match_else)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cast_precision_loss)] // Statistics/metrics calculations intentionally accept precision loss
#![allow(clippy::suboptimal_flops)] // Exponential moving average clarity is more important than FMA optimization

use std::sync::Arc;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Cache access pattern analyzer for intelligent prefetching
#[derive(Debug, Clone)]
pub struct CacheAccessAnalyzer {
    /// Track access frequency per key type
    access_patterns: Arc<Mutex<HashMap<String, AccessPattern>>>,
    /// Recent access history for trend analysis
    recent_accesses: Arc<Mutex<VecDeque<AccessEvent>>>,
    /// Configuration
    config: AnalyzerConfig,
}

#[derive(Debug, Clone)]
struct AccessPattern {
    /// Total access count
    count: u64,
    /// Last access time
    last_access: Instant,
    /// Average time between accesses
    avg_interval: Duration,
    /// Cache hit rate for this pattern
    hit_rate: f64,
    /// Size of content typically accessed
    avg_size: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AccessEvent {
    key_type: String,
    timestamp: Instant,
    was_hit: bool,
    size: usize,
}

/// Configuration for cache access pattern analysis
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Maximum access events to track
    pub max_history_size: usize,
    /// Minimum accesses to establish pattern
    pub min_pattern_accesses: u64,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            max_history_size: 10_000,
            min_pattern_accesses: 5,
        }
    }
}

impl CacheAccessAnalyzer {
    /// Create new access pattern analyzer
    pub fn new(config: AnalyzerConfig) -> Self {
        Self {
            access_patterns: Arc::new(Mutex::new(HashMap::new())),
            recent_accesses: Arc::new(Mutex::new(VecDeque::new())),
            config,
        }
    }

    /// Record cache access for pattern learning
    pub fn record_access(&self, key_type: &str, was_hit: bool, size: usize) {
        let now = Instant::now();

        // Update access patterns
        if let Ok(mut patterns) = self.access_patterns.lock() {
            let pattern = patterns
                .entry(key_type.to_string())
                .or_insert_with(|| AccessPattern {
                    count: 0,
                    last_access: now,
                    avg_interval: Duration::from_secs(60),
                    hit_rate: 0.0,
                    avg_size: 0,
                });

            pattern.count += 1;
            let interval = now.duration_since(pattern.last_access);
            pattern.avg_interval = (pattern.avg_interval + interval) / 2;
            pattern.last_access = now;

            // Update hit rate with exponential moving average
            let alpha = 0.1;
            pattern.hit_rate =
                pattern.hit_rate * (1.0 - alpha) + (if was_hit { 1.0 } else { 0.0 }) * alpha;

            // Update average size
            pattern.avg_size = usize::midpoint(pattern.avg_size, size);
        }

        // Record in recent access history
        if let Ok(mut accesses) = self.recent_accesses.lock() {
            accesses.push_back(AccessEvent {
                key_type: key_type.to_string(),
                timestamp: now,
                was_hit,
                size,
            });

            // Keep history size bounded
            while accesses.len() > self.config.max_history_size {
                accesses.pop_front();
            }
        }
    }

    /// Get access pattern statistics for analysis
    pub fn get_access_patterns(&self) -> Vec<(String, AccessPatternStats)> {
        let mut patterns = Vec::new();

        if let Ok(pattern_map) = self.access_patterns.lock() {
            for (key_type, pattern) in pattern_map.iter() {
                if pattern.count >= self.config.min_pattern_accesses {
                    patterns.push((
                        key_type.clone(),
                        AccessPatternStats {
                            access_count: pattern.count,
                            hit_rate: pattern.hit_rate,
                            avg_size: pattern.avg_size,
                            avg_interval: pattern.avg_interval,
                            last_access: pattern.last_access,
                        },
                    ));
                }
            }
        }

        patterns.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));
        patterns
    }
}

/// Statistics for a specific access pattern
#[derive(Debug, Clone)]
pub struct AccessPatternStats {
    /// Total access count
    pub access_count: u64,
    /// Cache hit rate for this pattern
    pub hit_rate: f64,
    /// Average size of content accessed
    pub avg_size: usize,
    /// Average time between accesses
    pub avg_interval: Duration,
    /// Last access time
    pub last_access: Instant,
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_access_analyzer() {
        let analyzer = CacheAccessAnalyzer::new(AnalyzerConfig::default());

        // Record some access patterns
        analyzer.record_access("manifest", false, 1000);
        analyzer.record_access("manifest", false, 1000);
        analyzer.record_access("manifest", true, 1000);
        analyzer.record_access("content", true, 5000);
        analyzer.record_access("content", true, 5000);
        analyzer.record_access("content", true, 5000);
        analyzer.record_access("content", true, 5000);
        analyzer.record_access("content", true, 5000);
        analyzer.record_access("content", true, 5000);

        let patterns = analyzer.get_access_patterns();
        assert!(!patterns.is_empty());

        // Should have content pattern with higher access count
        let content_pattern = patterns
            .iter()
            .find(|(key, _)| key == "content")
            .expect("should find content access pattern");
        assert!(content_pattern.1.access_count >= 5);
        assert!(content_pattern.1.hit_rate > 0.4); // Should reflect exponential moving average
    }
}
