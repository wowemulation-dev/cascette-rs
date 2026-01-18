//! Priority system and calculations for download manifests

use crate::download::entry::DownloadFileEntry;
use crate::download::header::DownloadHeader;
use std::cmp::Ordering;
use std::collections::HashMap;

/// Priority categories for download planning
///
/// These categories provide semantic meaning to priority values and enable
/// different download strategies based on content importance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PriorityCategory {
    /// Must download before game starts (priority < 0)
    Critical,
    /// Required for basic gameplay (priority = 0)
    Essential,
    /// Important for full experience (priority = 1-2)
    High,
    /// Standard content (priority = 3-5)
    Normal,
    /// Optional/deferred content (priority > 5)
    Low,
}

impl PriorityCategory {
    /// Convert priority value to category
    pub fn from_priority(priority: i8) -> Self {
        match priority {
            i8::MIN..=-1 => Self::Critical,
            0 => Self::Essential,
            1..=2 => Self::High,
            3..=5 => Self::Normal,
            _ => Self::Low,
        }
    }

    /// Get the priority range for this category
    pub fn priority_range(self) -> (i8, i8) {
        match self {
            Self::Critical => (i8::MIN, -1),
            Self::Essential => (0, 0),
            Self::High => (1, 2),
            Self::Normal => (3, 5),
            Self::Low => (6, i8::MAX),
        }
    }

    /// Get a human-readable description
    pub fn description(self) -> &'static str {
        match self {
            Self::Critical => "Must download before game can start",
            Self::Essential => "Required for basic gameplay",
            Self::High => "Important for full experience",
            Self::Normal => "Standard game content",
            Self::Low => "Optional or deferrable content",
        }
    }

    /// Get the suggested download weight (lower = higher priority)
    pub fn download_weight(self) -> u8 {
        match self {
            Self::Critical => 1,
            Self::Essential => 2,
            Self::High => 3,
            Self::Normal => 4,
            Self::Low => 5,
        }
    }

    /// Check if this category should block game launch
    pub fn blocks_launch(self) -> bool {
        matches!(self, Self::Critical | Self::Essential)
    }

    /// Check if this category can be downloaded while playing
    pub fn supports_streaming(self) -> bool {
        matches!(self, Self::Normal | Self::Low)
    }

    /// Get all categories in priority order (highest priority first)
    pub fn all_ordered() -> Vec<Self> {
        vec![
            Self::Critical,
            Self::Essential,
            Self::High,
            Self::Normal,
            Self::Low,
        ]
    }
}

impl std::fmt::Display for PriorityCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "Critical"),
            Self::Essential => write!(f, "Essential"),
            Self::High => write!(f, "High"),
            Self::Normal => write!(f, "Normal"),
            Self::Low => write!(f, "Low"),
        }
    }
}

/// Statistics for a specific priority category
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryStats {
    /// Number of files in this category
    pub file_count: usize,
    /// Total size of all files in this category
    pub total_size: u64,
    /// Percentage of total files
    pub percentage_of_files: f64,
    /// Percentage of total download size
    pub percentage_of_size: f64,
    /// Average file size in this category
    pub average_file_size: f64,
    /// Largest file size in this category
    pub max_file_size: u64,
    /// Smallest file size in this category
    pub min_file_size: u64,
}

impl CategoryStats {
    /// Create new category statistics
    pub fn new(
        file_count: usize,
        total_size: u64,
        total_files: usize,
        total_download_size: u64,
    ) -> Self {
        let percentage_of_files = if total_files > 0 {
            (file_count as f64 / total_files as f64) * 100.0
        } else {
            0.0
        };

        let percentage_of_size = if total_download_size > 0 {
            (total_size as f64 / total_download_size as f64) * 100.0
        } else {
            0.0
        };

        let average_file_size = if file_count > 0 {
            total_size as f64 / file_count as f64
        } else {
            0.0
        };

        Self {
            file_count,
            total_size,
            percentage_of_files,
            percentage_of_size,
            average_file_size,
            max_file_size: 0,
            min_file_size: u64::MAX,
        }
    }

    /// Update min/max file sizes
    pub fn update_file_size_bounds(&mut self, file_size: u64) {
        self.max_file_size = self.max_file_size.max(file_size);
        self.min_file_size = self.min_file_size.min(file_size);
    }

    /// Finalize statistics (handle empty categories)
    pub fn finalize(&mut self) {
        if self.file_count == 0 {
            self.min_file_size = 0;
        }
    }

    /// Check if this category is significant (>1% of files or size)
    pub fn is_significant(&self) -> bool {
        self.percentage_of_files > 1.0 || self.percentage_of_size > 1.0
    }

    /// Get human-readable size string
    pub fn total_size_human_readable(&self) -> String {
        crate::download::entry::FileSize40::new(self.total_size).map_or_else(
            |_| format!("{} bytes", self.total_size),
            super::entry::FileSize40::to_human_readable,
        )
    }
}

/// Complete analysis of priority distribution in a download manifest
#[derive(Debug, Clone, PartialEq)]
pub struct PriorityAnalysis {
    /// Total number of files
    pub total_files: usize,
    /// Total download size
    pub total_size: u64,
    /// Priority range (min, max)
    pub priority_range: (i8, i8),
    /// Base priority adjustment applied
    pub base_priority_adjustment: i8,
    /// Statistics for each priority category
    pub categories: HashMap<PriorityCategory, CategoryStats>,
    /// Essential download size (Critical + Essential)
    pub essential_size: u64,
    /// Streamable content size (Normal + Low)
    pub streamable_size: u64,
}

impl PriorityAnalysis {
    /// Create a new priority analysis
    pub fn new(base_priority_adjustment: i8) -> Self {
        Self {
            total_files: 0,
            total_size: 0,
            priority_range: (i8::MAX, i8::MIN),
            base_priority_adjustment,
            categories: HashMap::new(),
            essential_size: 0,
            streamable_size: 0,
        }
    }

    /// Add an entry to the analysis
    pub fn add_entry(
        &mut self,
        category: PriorityCategory,
        file_size: u64,
        effective_priority: i8,
    ) {
        self.total_files += 1;
        self.total_size += file_size;

        // Update priority range
        self.priority_range.0 = self.priority_range.0.min(effective_priority);
        self.priority_range.1 = self.priority_range.1.max(effective_priority);

        // Update essential/streamable sizes
        if category.blocks_launch() {
            self.essential_size += file_size;
        } else if category.supports_streaming() {
            self.streamable_size += file_size;
        }

        // Update category stats
        let stats = self
            .categories
            .entry(category)
            .or_insert_with(|| CategoryStats::new(0, 0, 0, 0));
        stats.file_count += 1;
        stats.total_size += file_size;
        stats.update_file_size_bounds(file_size);
    }

    /// Finalize the analysis by calculating percentages
    pub fn finalize(&mut self) {
        // Handle empty analysis
        if self.total_files == 0 {
            self.priority_range = (0, 0);
            return;
        }

        // Update category percentages
        for stats in self.categories.values_mut() {
            stats.percentage_of_files = (stats.file_count as f64 / self.total_files as f64) * 100.0;
            stats.percentage_of_size = if self.total_size > 0 {
                (stats.total_size as f64 / self.total_size as f64) * 100.0
            } else {
                0.0
            };
            stats.average_file_size = if stats.file_count > 0 {
                stats.total_size as f64 / stats.file_count as f64
            } else {
                0.0
            };
            stats.finalize();
        }
    }

    /// Get the percentage of essential content (must download before playing)
    pub fn essential_percentage(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            (self.essential_size as f64 / self.total_size as f64) * 100.0
        }
    }

    /// Get the percentage of streamable content (can download while playing)
    pub fn streamable_percentage(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            (self.streamable_size as f64 / self.total_size as f64) * 100.0
        }
    }

    /// Check if this manifest has good streaming characteristics
    pub fn has_good_streaming_potential(&self) -> bool {
        self.streamable_percentage() > 20.0 && self.essential_percentage() < 80.0
    }

    /// Get time-to-playable estimate (assuming download speed in MB/s)
    pub fn time_to_playable_seconds(&self, download_speed_mbps: f64) -> f64 {
        let essential_mb = self.essential_size as f64 / (1024.0 * 1024.0);
        essential_mb / download_speed_mbps
    }

    /// Get most significant categories (by file count or size)
    pub fn significant_categories(&self) -> Vec<(PriorityCategory, &CategoryStats)> {
        let mut categories: Vec<_> = self
            .categories
            .iter()
            .filter(|(_, stats)| stats.is_significant())
            .map(|(cat, stats)| (*cat, stats))
            .collect();

        // Sort by priority order
        categories.sort_by_key(|(cat, _)| cat.download_weight());
        categories
    }

    /// Get category with the most files
    pub fn largest_category_by_files(&self) -> Option<(PriorityCategory, &CategoryStats)> {
        self.categories
            .iter()
            .max_by_key(|(_, stats)| stats.file_count)
            .map(|(cat, stats)| (*cat, stats))
    }

    /// Get category with the most data
    pub fn largest_category_by_size(&self) -> Option<(PriorityCategory, &CategoryStats)> {
        self.categories
            .iter()
            .max_by_key(|(_, stats)| stats.total_size)
            .map(|(cat, stats)| (*cat, stats))
    }

    /// Generate a summary report
    pub fn summary_report(&self) -> String {
        use std::fmt::Write;
        let mut report = String::new();

        report.push_str("Download Manifest Priority Analysis\n");
        report.push_str("=====================================");
        writeln!(&mut report, "Total Files: {}", self.total_files)
            .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Total Size: {}",
            crate::download::entry::FileSize40::new(self.total_size).map_or_else(
                |_| format!("{} bytes", self.total_size),
                super::entry::FileSize40::to_human_readable
            )
        )
        .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Priority Range: {} to {}",
            self.priority_range.0, self.priority_range.1
        )
        .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Base Priority Adjustment: {}",
            self.base_priority_adjustment
        )
        .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Essential Content: {:.1}%",
            self.essential_percentage()
        )
        .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Streamable Content: {:.1}%",
            self.streamable_percentage()
        )
        .expect("Operation should succeed");
        writeln!(
            &mut report,
            "Good Streaming Potential: {}",
            if self.has_good_streaming_potential() {
                "Yes"
            } else {
                "No"
            }
        )
        .expect("Operation should succeed");

        report.push_str("\nCategory Breakdown:\n");
        for category in PriorityCategory::all_ordered() {
            if let Some(stats) = self.categories.get(&category) {
                if stats.file_count > 0 {
                    writeln!(
                        &mut report,
                        "  {}: {} files ({:.1}%), {} ({:.1}%)",
                        category,
                        stats.file_count,
                        stats.percentage_of_files,
                        stats.total_size_human_readable(),
                        stats.percentage_of_size
                    )
                    .expect("Operation should succeed");
                }
            }
        }

        report
    }
}

/// Analyze the priority distribution of entries in a download manifest
pub fn analyze_priorities(
    entries: &[DownloadFileEntry],
    header: &DownloadHeader,
) -> PriorityAnalysis {
    let mut analysis = PriorityAnalysis::new(header.base_priority());

    for entry in entries {
        let effective_priority = entry.effective_priority(header);
        let category = entry.priority_category(header);
        let file_size = entry.file_size.as_u64();

        analysis.add_entry(category, file_size, effective_priority);
    }

    analysis.finalize();
    analysis
}

/// Create a download plan ordered by priority
#[derive(Debug, Clone)]
pub struct DownloadPlan {
    /// Entries ordered by download priority (highest first)
    pub entries: Vec<(usize, PriorityCategory, i8)>, // (index, category, effective_priority)
    /// Total size of selected entries
    pub total_size: u64,
    /// Essential content size
    pub essential_size: u64,
    /// Category breakdown
    pub category_breakdown: HashMap<PriorityCategory, (usize, u64)>, // (count, size)
}

impl DownloadPlan {
    /// Create a download plan from manifest entries
    pub fn create(
        entries: &[DownloadFileEntry],
        header: &DownloadHeader,
        max_priority: Option<i8>,
        required_categories: Option<&[PriorityCategory]>,
    ) -> Self {
        let mut plan_entries = Vec::new();
        let mut total_size = 0;
        let mut essential_size = 0;
        let mut category_breakdown = HashMap::new();

        for (index, entry) in entries.iter().enumerate() {
            let effective_priority = entry.effective_priority(header);
            let category = entry.priority_category(header);

            // Apply priority filter
            if let Some(max_pri) = max_priority {
                if effective_priority > max_pri {
                    continue;
                }
            }

            // Apply category filter
            if let Some(required_cats) = required_categories {
                if !required_cats.contains(&category) {
                    continue;
                }
            }

            let file_size = entry.file_size.as_u64();
            plan_entries.push((index, category, effective_priority));
            total_size += file_size;

            if category.blocks_launch() {
                essential_size += file_size;
            }

            let (count, size) = category_breakdown.entry(category).or_insert((0, 0));
            *count += 1;
            *size += file_size;
        }

        // Sort by effective priority (lower values = higher priority)
        plan_entries.sort_by(|a, b| {
            match a.2.cmp(&b.2) {
                Ordering::Equal => a.0.cmp(&b.0), // Secondary sort by index for stability
                other => other,
            }
        });

        Self {
            entries: plan_entries,
            total_size,
            essential_size,
            category_breakdown,
        }
    }

    /// Get entries for essential content only
    pub fn essential_only(entries: &[DownloadFileEntry], header: &DownloadHeader) -> Self {
        Self::create(
            entries,
            header,
            Some(0), // Priority <= 0
            None,
        )
    }

    /// Get entries for critical content only
    pub fn critical_only(entries: &[DownloadFileEntry], header: &DownloadHeader) -> Self {
        Self::create(
            entries,
            header,
            Some(-1), // Priority < 0
            None,
        )
    }

    /// Get entries by category
    pub fn by_categories(
        entries: &[DownloadFileEntry],
        header: &DownloadHeader,
        categories: &[PriorityCategory],
    ) -> Self {
        Self::create(entries, header, None, Some(categories))
    }

    /// Get the percentage of essential content
    pub fn essential_percentage(&self) -> f64 {
        if self.total_size == 0 {
            0.0
        } else {
            (self.essential_size as f64 / self.total_size as f64) * 100.0
        }
    }

    /// Get estimated download time in seconds
    pub fn estimated_download_time_seconds(&self, download_speed_mbps: f64) -> f64 {
        let total_mb = self.total_size as f64 / (1024.0 * 1024.0);
        total_mb / download_speed_mbps
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::download::entry::DownloadFileEntry;
    use crate::download::header::DownloadHeader;
    use cascette_crypto::EncodingKey;

    fn create_test_entry(priority: i8, size: u64) -> DownloadFileEntry {
        let ekey = EncodingKey::from_bytes([0u8; 16]);
        DownloadFileEntry::new(ekey, size, priority).expect("Operation should succeed")
    }

    #[test]
    fn test_priority_categories() {
        assert_eq!(
            PriorityCategory::from_priority(-5),
            PriorityCategory::Critical
        );
        assert_eq!(
            PriorityCategory::from_priority(0),
            PriorityCategory::Essential
        );
        assert_eq!(PriorityCategory::from_priority(1), PriorityCategory::High);
        assert_eq!(PriorityCategory::from_priority(2), PriorityCategory::High);
        assert_eq!(PriorityCategory::from_priority(3), PriorityCategory::Normal);
        assert_eq!(PriorityCategory::from_priority(5), PriorityCategory::Normal);
        assert_eq!(PriorityCategory::from_priority(10), PriorityCategory::Low);
    }

    #[test]
    fn test_category_properties() {
        assert!(PriorityCategory::Critical.blocks_launch());
        assert!(PriorityCategory::Essential.blocks_launch());
        assert!(!PriorityCategory::High.blocks_launch());

        assert!(!PriorityCategory::Critical.supports_streaming());
        assert!(!PriorityCategory::Essential.supports_streaming());
        assert!(!PriorityCategory::High.supports_streaming());
        assert!(PriorityCategory::Normal.supports_streaming());
        assert!(PriorityCategory::Low.supports_streaming());
    }

    #[test]
    fn test_category_ordering() {
        let categories = PriorityCategory::all_ordered();
        assert_eq!(categories[0], PriorityCategory::Critical);
        assert_eq!(categories[1], PriorityCategory::Essential);
        assert_eq!(categories[4], PriorityCategory::Low);

        // Check download weights are in order
        for i in 1..categories.len() {
            assert!(categories[i - 1].download_weight() < categories[i].download_weight());
        }
    }

    #[test]
    fn test_category_stats() {
        let mut stats = CategoryStats::new(10, 1024, 100, 10240);
        assert_eq!(stats.file_count, 10);
        assert_eq!(stats.total_size, 1024);
        assert!((stats.percentage_of_files - 10.0).abs() < f64::EPSILON); // 10/100 * 100
        assert!((stats.percentage_of_size - 10.0).abs() < f64::EPSILON); // 1024/10240 * 100
        assert!((stats.average_file_size - 102.4).abs() < f64::EPSILON); // 1024/10

        stats.update_file_size_bounds(512);
        stats.update_file_size_bounds(2048);
        stats.finalize();

        assert_eq!(stats.max_file_size, 2048);
        assert_eq!(stats.min_file_size, 512);
        assert!(stats.is_significant());
    }

    #[test]
    fn test_priority_analysis() {
        let entries = vec![
            create_test_entry(-2, 1000), // Critical
            create_test_entry(0, 2000),  // Essential
            create_test_entry(1, 3000),  // High
            create_test_entry(4, 4000),  // Normal
            create_test_entry(10, 5000), // Low
        ];

        let header = DownloadHeader::new_v1(entries.len() as u32, 0, false);
        let analysis = analyze_priorities(&entries, &header);

        assert_eq!(analysis.total_files, 5);
        assert_eq!(analysis.total_size, 15000);
        assert_eq!(analysis.priority_range, (-2, 10));
        assert_eq!(analysis.base_priority_adjustment, 0);

        // Essential size = Critical + Essential = 1000 + 2000 = 3000
        assert_eq!(analysis.essential_size, 3000);
        assert!((analysis.essential_percentage() - 20.0).abs() < f64::EPSILON);

        // Streamable size = Normal + Low = 4000 + 5000 = 9000
        assert_eq!(analysis.streamable_size, 9000);
        assert!((analysis.streamable_percentage() - 60.0).abs() < f64::EPSILON);

        assert!(analysis.has_good_streaming_potential());

        // Check categories
        assert_eq!(analysis.categories.len(), 5);

        let critical_stats = analysis
            .categories
            .get(&PriorityCategory::Critical)
            .expect("Operation should succeed");
        assert_eq!(critical_stats.file_count, 1);
        assert_eq!(critical_stats.total_size, 1000);

        let low_stats = analysis
            .categories
            .get(&PriorityCategory::Low)
            .expect("Operation should succeed");
        assert_eq!(low_stats.file_count, 1);
        assert_eq!(low_stats.total_size, 5000);
    }

    #[test]
    fn test_priority_analysis_with_base_adjustment() {
        let entries = vec![
            create_test_entry(3, 1000),  // Effective: 3 - (-2) = 5 (Normal)
            create_test_entry(-2, 2000), // Effective: -2 - (-2) = 0 (Essential)
        ];

        let header = DownloadHeader::new_v3(entries.len() as u32, 0, false, 0, -2);
        let analysis = analyze_priorities(&entries, &header);

        assert_eq!(analysis.base_priority_adjustment, -2);
        assert_eq!(analysis.priority_range, (0, 5));

        // First entry should be Normal category (effective priority 5)
        let normal_stats = analysis
            .categories
            .get(&PriorityCategory::Normal)
            .expect("Operation should succeed");
        assert_eq!(normal_stats.file_count, 1);
        assert_eq!(normal_stats.total_size, 1000);

        // Second entry should be Essential category (effective priority 0)
        let essential_stats = analysis
            .categories
            .get(&PriorityCategory::Essential)
            .expect("Operation should succeed");
        assert_eq!(essential_stats.file_count, 1);
        assert_eq!(essential_stats.total_size, 2000);
    }

    #[test]
    fn test_download_plan_creation() {
        let entries = vec![
            create_test_entry(5, 1000),  // Low priority
            create_test_entry(-1, 2000), // Critical
            create_test_entry(0, 3000),  // Essential
            create_test_entry(2, 4000),  // High
        ];

        let header = DownloadHeader::new_v1(entries.len() as u32, 0, false);
        let plan = DownloadPlan::create(&entries, &header, None, None);

        assert_eq!(plan.entries.len(), 4);
        assert_eq!(plan.total_size, 10000);
        assert_eq!(plan.essential_size, 5000); // Critical + Essential = 2000 + 3000

        // Check ordering (should be sorted by priority: -1, 0, 2, 5)
        assert_eq!(plan.entries[0].2, -1); // Critical first
        assert_eq!(plan.entries[1].2, 0); // Essential second
        assert_eq!(plan.entries[2].2, 2); // High third
        assert_eq!(plan.entries[3].2, 5); // Low last
    }

    #[test]
    fn test_download_plan_filtering() {
        let entries = vec![
            create_test_entry(-2, 1000), // Critical
            create_test_entry(0, 2000),  // Essential
            create_test_entry(3, 3000),  // Normal
            create_test_entry(10, 4000), // Low
        ];

        let header = DownloadHeader::new_v1(entries.len() as u32, 0, false);

        // Essential only (priority <= 0)
        let essential_plan = DownloadPlan::essential_only(&entries, &header);
        assert_eq!(essential_plan.entries.len(), 2);
        assert_eq!(essential_plan.total_size, 3000);

        // Critical only (priority < 0)
        let critical_plan = DownloadPlan::critical_only(&entries, &header);
        assert_eq!(critical_plan.entries.len(), 1);
        assert_eq!(critical_plan.total_size, 1000);

        // By categories
        let streaming_plan = DownloadPlan::by_categories(
            &entries,
            &header,
            &[PriorityCategory::Normal, PriorityCategory::Low],
        );
        assert_eq!(streaming_plan.entries.len(), 2);
        assert_eq!(streaming_plan.total_size, 7000);
    }

    #[test]
    fn test_time_calculations() {
        let entries = vec![create_test_entry(0, 100 * 1024 * 1024)]; // 100MB
        let header = DownloadHeader::new_v1(1, 0, false);
        let analysis = analyze_priorities(&entries, &header);

        // At 10 MB/s download speed
        let time_seconds = analysis.time_to_playable_seconds(10.0);
        assert!((time_seconds - 10.0).abs() < f64::EPSILON); // 100MB / 10MB/s = 10 seconds

        let plan = DownloadPlan::essential_only(&entries, &header);
        let plan_time = plan.estimated_download_time_seconds(10.0);
        assert!((plan_time - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summary_report() {
        let entries = vec![
            create_test_entry(-1, 1000), // Critical
            create_test_entry(0, 2000),  // Essential
            create_test_entry(10, 3000), // Low (changed from 5 to 10 to be in Low category)
        ];

        let header = DownloadHeader::new_v1(3, 0, false);
        let analysis = analyze_priorities(&entries, &header);

        let report = analysis.summary_report();
        assert!(report.contains("Total Files: 3"));
        assert!(report.contains("Priority Range: -1 to 10"));
        assert!(report.contains("Critical:"));
        assert!(report.contains("Essential:"));
        assert!(report.contains("Low:"));
    }
}
