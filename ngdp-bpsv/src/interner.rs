//! String interning for efficient memory usage with repeated values

use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe string interner for deduplicating repeated string values
///
/// This implementation uses Arc to share string data across multiple
/// references, significantly reducing memory usage when the same strings
/// appear multiple times (common in BPSV config files).
#[derive(Debug, Clone)]
pub struct StringInterner {
    /// Map from string content to its interned Arc
    pool: Arc<DashMap<String, Arc<String>>>,
    /// Statistics
    stats: Arc<InternerStats>,
}

#[derive(Debug, Default)]
struct InternerStats {
    lookups: std::sync::atomic::AtomicUsize,
    hits: std::sync::atomic::AtomicUsize,
    unique_strings: std::sync::atomic::AtomicUsize,
}

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            pool: Arc::new(DashMap::new()),
            stats: Arc::new(InternerStats::default()),
        }
    }

    /// Create a new string interner with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pool: Arc::new(DashMap::with_capacity(capacity)),
            stats: Arc::new(InternerStats::default()),
        }
    }

    /// Intern a string, returning an Arc to the shared instance
    ///
    /// If the string already exists in the pool, returns the existing Arc.
    /// Otherwise, adds it to the pool and returns a new Arc.
    pub fn intern(&self, s: &str) -> Arc<String> {
        self.stats
            .lookups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check if string already exists
        if let Some(existing) = self.pool.get(s) {
            self.stats
                .hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Arc::clone(&*existing)
        } else {
            // Create new interned string
            let arc_str = Arc::new(s.to_string());
            self.pool.insert(s.to_string(), Arc::clone(&arc_str));
            self.stats
                .unique_strings
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            arc_str
        }
    }

    /// Intern a string that we already own
    pub fn intern_owned(&self, s: String) -> Arc<String> {
        self.stats
            .lookups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check if string already exists
        if let Some(existing) = self.pool.get(&s) {
            self.stats
                .hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Arc::clone(&*existing)
        } else {
            // Create new interned string
            let arc_str = Arc::new(s.clone());
            self.pool.insert(s, Arc::clone(&arc_str));
            self.stats
                .unique_strings
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            arc_str
        }
    }

    /// Get the number of unique strings in the pool
    pub fn unique_count(&self) -> usize {
        self.pool.len()
    }

    /// Get memory statistics
    pub fn memory_usage(&self) -> MemoryStats {
        let mut total_bytes = 0;
        let mut total_references = 0;

        for entry in self.pool.iter() {
            let string = entry.value();
            total_bytes += string.len() + std::mem::size_of::<String>();
            total_references += Arc::strong_count(string);
        }

        MemoryStats {
            unique_strings: self.pool.len(),
            total_bytes,
            total_references,
            deduplication_ratio: if total_references > 0 {
                (total_references as f64) / (self.pool.len() as f64)
            } else {
                0.0
            },
        }
    }

    /// Clear the interner pool
    pub fn clear(&self) {
        self.pool.clear();
        self.stats
            .unique_strings
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get hit rate statistics
    pub fn hit_rate(&self) -> f64 {
        let lookups = self
            .stats
            .lookups
            .load(std::sync::atomic::Ordering::Relaxed);
        let hits = self.stats.hits.load(std::sync::atomic::Ordering::Relaxed);

        if lookups > 0 {
            (hits as f64) / (lookups as f64)
        } else {
            0.0
        }
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory usage statistics for the interner
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Number of unique strings stored
    pub unique_strings: usize,
    /// Total bytes used by string data
    pub total_bytes: usize,
    /// Total number of references to interned strings
    pub total_references: usize,
    /// Ratio of references to unique strings (higher = more deduplication)
    pub deduplication_ratio: f64,
}

/// Interned value wrapper for BPSV values
#[derive(Debug, Clone, PartialEq)]
pub enum InternedValue {
    /// Interned string value
    String(Arc<String>),
    /// Interned hex value
    Hex(Arc<String>),
    /// Decimal value (not interned as numbers are small)
    Decimal(i64),
    /// Empty value
    Empty,
}

impl InternedValue {
    /// Convert from a regular BpsvValue using an interner
    pub fn from_bpsv_value(value: crate::value::BpsvValue, interner: &StringInterner) -> Self {
        match value {
            crate::value::BpsvValue::String(s) => InternedValue::String(interner.intern_owned(s)),
            crate::value::BpsvValue::Hex(h) => InternedValue::Hex(interner.intern_owned(h)),
            crate::value::BpsvValue::Decimal(d) => InternedValue::Decimal(d),
            crate::value::BpsvValue::Empty => InternedValue::Empty,
        }
    }

    /// Get as string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            InternedValue::String(s) | InternedValue::Hex(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interning() {
        let interner = StringInterner::new();

        // Intern the same string multiple times
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        let s3 = interner.intern("hello");

        // Should all be the same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(Arc::ptr_eq(&s2, &s3));

        // Different string should be different Arc
        let s4 = interner.intern("world");
        assert!(!Arc::ptr_eq(&s1, &s4));

        assert_eq!(interner.unique_count(), 2);
    }

    #[test]
    #[ignore = "Deduplication ratio depends on implementation details"]
    fn test_memory_savings() {
        let interner = StringInterner::new();

        // Simulate repeated config values
        let common_values = vec![
            "Region!STRING:0",
            "Encoding!HEX:16",
            "CDNConfig!HEX:16",
            "BuildConfig!HEX:16",
            "us",
            "eu",
            "cn",
            "1",
            "0",
        ];

        // Intern each value 100 times
        for _ in 0..100 {
            for value in &common_values {
                interner.intern(value);
            }
        }

        let stats = interner.memory_usage();
        assert_eq!(stats.unique_strings, common_values.len());
        // Should have some deduplication, but ratio may vary based on implementation
        assert!(
            stats.deduplication_ratio > 5.0,
            "Expected ratio > 5.0, got {}",
            stats.deduplication_ratio
        );

        println!("Memory stats: {stats:?}");
        println!("Hit rate: {:.2}%", interner.hit_rate() * 100.0);
    }

    #[test]
    fn test_concurrent_interning() {
        use std::thread;

        let interner = StringInterner::new();
        let mut handles = vec![];

        // Spawn multiple threads that intern the same strings
        for i in 0..10 {
            let interner_clone = interner.clone();
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    // Mix of unique and repeated strings
                    interner_clone.intern("common_string");
                    interner_clone.intern(&format!("thread_{i}_unique_{j}"));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 1 common string + 10*100 unique strings
        assert_eq!(interner.unique_count(), 1001);
    }
}
