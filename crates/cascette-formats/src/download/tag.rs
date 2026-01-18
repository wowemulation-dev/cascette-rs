//! Download manifest tag system using install tags

use crate::install::{InstallTag, TagType};

/// Download tag type alias
///
/// Download manifests use the same tag structure as install manifests,
/// but with different semantic meaning focused on streaming priorities.
pub type DownloadTag = InstallTag;

/// Tag operations specific to download manifests
impl DownloadTag {
    /// Check if this tag affects streaming download decisions
    ///
    /// These tags are typically used to determine which files should be
    /// downloaded for a specific platform, architecture, or locale.
    pub fn affects_streaming(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Platform
                | TagType::Architecture
                | TagType::Locale
                | TagType::Feature
                | TagType::Component
                | TagType::Region
        )
    }

    /// Check if this tag represents optional content
    ///
    /// Optional content can be downloaded later or skipped entirely
    /// without affecting core gameplay functionality.
    pub fn is_optional(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Option | TagType::Alternate | TagType::Expansion | TagType::Content
        )
    }

    /// Check if this tag represents required content
    ///
    /// Required content must be downloaded for the game to function properly.
    pub fn is_required(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Platform | TagType::Architecture | TagType::Component | TagType::Category
        )
    }

    /// Check if this tag represents platform-specific content
    pub fn is_platform_specific(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Platform | TagType::Architecture | TagType::Device
        )
    }

    /// Check if this tag represents locale-specific content
    pub fn is_locale_specific(&self) -> bool {
        matches!(self.tag_type, TagType::Locale | TagType::Region)
    }

    /// Check if this tag represents feature content
    pub fn is_feature_content(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Feature | TagType::Content | TagType::Expansion
        )
    }

    /// Get the download priority weight for this tag type
    ///
    /// Lower values indicate higher priority for download ordering.
    /// This helps prioritize platform-specific and required content.
    pub fn download_priority_weight(&self) -> u8 {
        match self.tag_type {
            TagType::Platform => 1,     // Highest priority - platform compatibility
            TagType::Architecture => 2, // Second highest - architecture compatibility
            TagType::Component => 3,    // Core components
            TagType::Category | TagType::Unknown => 4, // Content categories
            TagType::Locale => 5,       // Language content
            TagType::Region => 6,       // Regional content
            TagType::Feature => 7,      // Feature-specific content
            TagType::Version => 8,      // Version-specific content
            TagType::Device => 9,       // Device-specific content
            TagType::Mode => 10,        // Mode-specific content
            TagType::Branch => 11,      // Branch-specific content
            TagType::Optimization => 12, // Optimization variants
            TagType::Content => 13,     // General content
            TagType::Expansion => 14,   // Expansion content
            TagType::Alternate => 15,   // Alternate versions
            TagType::Option => 16,      // Lowest priority - optional content
        }
    }

    /// Check if this tag should be included in essential downloads
    ///
    /// Essential downloads are those required for basic game functionality.
    pub fn is_essential_for_download(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Platform | TagType::Architecture | TagType::Component | TagType::Category
        )
    }

    /// Check if this tag represents content that can be streamed later
    ///
    /// Streamable content can be downloaded while the game is running.
    pub fn is_streamable(&self) -> bool {
        matches!(
            self.tag_type,
            TagType::Content
                | TagType::Feature
                | TagType::Expansion
                | TagType::Option
                | TagType::Alternate
        )
    }

    /// Get a human-readable description of the tag's role in downloads
    pub fn download_description(&self) -> &'static str {
        match self.tag_type {
            TagType::Platform => "Platform compatibility files",
            TagType::Architecture => "Architecture-specific binaries",
            TagType::Locale => "Language and localization files",
            TagType::Category => "Core game content categories",
            TagType::Unknown => "Unknown category content",
            TagType::Component => "Essential game components",
            TagType::Version => "Version-specific content",
            TagType::Optimization => "Performance optimizations",
            TagType::Region => "Regional customizations",
            TagType::Device => "Device-specific optimizations",
            TagType::Mode => "Game mode content",
            TagType::Branch => "Development branch content",
            TagType::Content => "General game content",
            TagType::Feature => "Feature-specific content",
            TagType::Expansion => "Expansion pack content",
            TagType::Alternate => "Alternative content versions",
            TagType::Option => "Optional features and content",
        }
    }
}

/// Helper functions for tag filtering and analysis
impl DownloadTag {
    /// Create a platform filter for common platforms
    pub fn create_platform_filter() -> Vec<String> {
        vec![
            "Windows".to_string(),
            "Mac".to_string(),
            "Linux".to_string(),
        ]
    }

    /// Create an architecture filter for common architectures
    pub fn create_architecture_filter() -> Vec<String> {
        vec!["x86".to_string(), "x86_64".to_string(), "arm64".to_string()]
    }

    /// Create a locale filter for common locales
    pub fn create_locale_filter() -> Vec<String> {
        vec![
            "enUS".to_string(),
            "enGB".to_string(),
            "deDE".to_string(),
            "frFR".to_string(),
            "esES".to_string(),
            "zhCN".to_string(),
            "koKR".to_string(),
            "jaJP".to_string(),
        ]
    }

    /// Check if tag matches any of the provided filter values
    pub fn matches_filter(&self, filter: &[String]) -> bool {
        filter.contains(&self.name)
    }

    /// Check if tag matches platform requirements
    pub fn matches_platform(&self, platform: &str, architecture: &str) -> bool {
        match self.tag_type {
            TagType::Platform => self.name == platform,
            TagType::Architecture => self.name == architecture,
            _ => true, // Non-platform tags match by default
        }
    }

    /// Check if tag matches locale requirements
    pub fn matches_locale(&self, locale: &str, region: Option<&str>) -> bool {
        match self.tag_type {
            TagType::Locale => self.name == locale,
            TagType::Region => region.is_none_or(|r| self.name == r),
            _ => true, // Non-locale tags match by default
        }
    }
}

/// Tag analysis for download planning
pub struct TagAnalysis {
    /// Total number of tags
    pub total_tags: usize,
    /// Number of platform-specific tags
    pub platform_tags: usize,
    /// Number of locale-specific tags
    pub locale_tags: usize,
    /// Number of optional tags
    pub optional_tags: usize,
    /// Number of required tags
    pub required_tags: usize,
    /// Number of streamable tags
    pub streamable_tags: usize,
}

impl TagAnalysis {
    /// Analyze a collection of download tags
    pub fn analyze(tags: &[DownloadTag]) -> Self {
        let total_tags = tags.len();
        let mut platform_tags = 0;
        let mut locale_tags = 0;
        let mut optional_tags = 0;
        let mut required_tags = 0;
        let mut streamable_tags = 0;

        for tag in tags {
            if tag.is_platform_specific() {
                platform_tags += 1;
            }
            if tag.is_locale_specific() {
                locale_tags += 1;
            }
            if tag.is_optional() {
                optional_tags += 1;
            }
            if tag.is_required() {
                required_tags += 1;
            }
            if tag.is_streamable() {
                streamable_tags += 1;
            }
        }

        Self {
            total_tags,
            platform_tags,
            locale_tags,
            optional_tags,
            required_tags,
            streamable_tags,
        }
    }

    /// Calculate the percentage of optional content
    pub fn optional_percentage(&self) -> f64 {
        if self.total_tags == 0 {
            0.0
        } else {
            (self.optional_tags as f64 / self.total_tags as f64) * 100.0
        }
    }

    /// Calculate the percentage of platform-specific content
    pub fn platform_percentage(&self) -> f64 {
        if self.total_tags == 0 {
            0.0
        } else {
            (self.platform_tags as f64 / self.total_tags as f64) * 100.0
        }
    }

    /// Check if the manifest has good streaming potential
    pub fn has_good_streaming_potential(&self) -> bool {
        self.streamable_tags > 0 && self.optional_percentage() > 10.0
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn create_test_tag(name: &str, tag_type: TagType) -> DownloadTag {
        DownloadTag {
            name: name.to_string(),
            tag_type,
            bit_mask: vec![0u8; 1],
        }
    }

    #[test]
    fn test_streaming_classification() {
        let platform_tag = create_test_tag("Windows", TagType::Platform);
        assert!(platform_tag.affects_streaming());
        assert!(!platform_tag.is_optional());
        assert!(platform_tag.is_required());
        assert!(platform_tag.is_platform_specific());
        assert!(platform_tag.is_essential_for_download());
        assert!(!platform_tag.is_streamable());

        let optional_tag = create_test_tag("Cinematics", TagType::Option);
        assert!(!optional_tag.affects_streaming());
        assert!(optional_tag.is_optional());
        assert!(!optional_tag.is_required());
        assert!(!optional_tag.is_platform_specific());
        assert!(!optional_tag.is_essential_for_download());
        assert!(optional_tag.is_streamable());

        let locale_tag = create_test_tag("enUS", TagType::Locale);
        assert!(locale_tag.affects_streaming());
        assert!(!locale_tag.is_optional());
        assert!(!locale_tag.is_required());
        assert!(!locale_tag.is_platform_specific());
        assert!(locale_tag.is_locale_specific());
        assert!(!locale_tag.is_essential_for_download());
        assert!(!locale_tag.is_streamable());
    }

    #[test]
    fn test_download_priority_weights() {
        let platform_tag = create_test_tag("Windows", TagType::Platform);
        let optional_tag = create_test_tag("Extras", TagType::Option);
        let feature_tag = create_test_tag("Graphics", TagType::Feature);

        assert_eq!(platform_tag.download_priority_weight(), 1);
        assert_eq!(optional_tag.download_priority_weight(), 16);
        assert_eq!(feature_tag.download_priority_weight(), 7);

        // Platform should have higher priority (lower weight) than optional
        assert!(platform_tag.download_priority_weight() < optional_tag.download_priority_weight());
        assert!(feature_tag.download_priority_weight() < optional_tag.download_priority_weight());
    }

    #[test]
    fn test_download_descriptions() {
        let tags = vec![
            (TagType::Platform, "Platform compatibility files"),
            (TagType::Architecture, "Architecture-specific binaries"),
            (TagType::Locale, "Language and localization files"),
            (TagType::Option, "Optional features and content"),
        ];

        for (tag_type, expected_desc) in tags {
            let tag = create_test_tag("test", tag_type);
            assert_eq!(tag.download_description(), expected_desc);
        }
    }

    #[test]
    fn test_filter_creation() {
        let platform_filter = DownloadTag::create_platform_filter();
        assert!(platform_filter.contains(&"Windows".to_string()));
        assert!(platform_filter.contains(&"Mac".to_string()));
        assert!(platform_filter.contains(&"Linux".to_string()));

        let arch_filter = DownloadTag::create_architecture_filter();
        assert!(arch_filter.contains(&"x86_64".to_string()));
        assert!(arch_filter.contains(&"arm64".to_string()));

        let locale_filter = DownloadTag::create_locale_filter();
        assert!(locale_filter.contains(&"enUS".to_string()));
        assert!(locale_filter.contains(&"deDE".to_string()));
    }

    #[test]
    fn test_filter_matching() {
        let tag = create_test_tag("Windows", TagType::Platform);
        let filter = vec!["Windows".to_string(), "Mac".to_string()];

        assert!(tag.matches_filter(&filter));

        let other_filter = vec!["Linux".to_string()];
        assert!(!tag.matches_filter(&other_filter));
    }

    #[test]
    fn test_platform_matching() {
        let platform_tag = create_test_tag("Windows", TagType::Platform);
        let arch_tag = create_test_tag("x86_64", TagType::Architecture);
        let other_tag = create_test_tag("Base", TagType::Category);

        assert!(platform_tag.matches_platform("Windows", "x86_64"));
        assert!(!platform_tag.matches_platform("Mac", "x86_64"));

        assert!(arch_tag.matches_platform("Windows", "x86_64"));
        assert!(!arch_tag.matches_platform("Windows", "arm64"));

        assert!(other_tag.matches_platform("Windows", "x86_64"));
        assert!(other_tag.matches_platform("Mac", "arm64"));
    }

    #[test]
    fn test_locale_matching() {
        let locale_tag = create_test_tag("enUS", TagType::Locale);
        let region_tag = create_test_tag("US", TagType::Region);
        let other_tag = create_test_tag("Base", TagType::Category);

        assert!(locale_tag.matches_locale("enUS", Some("US")));
        assert!(!locale_tag.matches_locale("deDE", Some("US")));

        assert!(region_tag.matches_locale("enUS", Some("US")));
        assert!(!region_tag.matches_locale("enUS", Some("EU")));
        assert!(region_tag.matches_locale("enUS", None)); // No region requirement

        assert!(other_tag.matches_locale("enUS", Some("US")));
        assert!(other_tag.matches_locale("deDE", Some("EU")));
    }

    #[test]
    fn test_tag_analysis() {
        let tags = vec![
            create_test_tag("Windows", TagType::Platform),
            create_test_tag("x86_64", TagType::Architecture),
            create_test_tag("enUS", TagType::Locale),
            create_test_tag("Base", TagType::Component),
            create_test_tag("Extras", TagType::Option),
            create_test_tag("Cinematics", TagType::Content),
        ];

        let analysis = TagAnalysis::analyze(&tags);

        assert_eq!(analysis.total_tags, 6);
        assert_eq!(analysis.platform_tags, 2); // Platform + Architecture
        assert_eq!(analysis.locale_tags, 1); // Locale
        assert_eq!(analysis.optional_tags, 2); // Option + Content (both are optional)
        assert_eq!(analysis.required_tags, 3); // Platform + Architecture + Component
        assert_eq!(analysis.streamable_tags, 2); // Option + Content

        assert!((analysis.optional_percentage() - 33.33).abs() < 0.1); // 2 out of 6 tags = 33.33%
        assert!((analysis.platform_percentage() - 33.33).abs() < 0.1);
        assert!(analysis.has_good_streaming_potential());
    }

    #[test]
    fn test_empty_tag_analysis() {
        let tags: Vec<DownloadTag> = vec![];
        let analysis = TagAnalysis::analyze(&tags);

        assert_eq!(analysis.total_tags, 0);
        assert_eq!(analysis.platform_tags, 0);
        assert_eq!(analysis.locale_tags, 0);
        assert_eq!(analysis.optional_tags, 0);
        assert_eq!(analysis.required_tags, 0);
        assert_eq!(analysis.streamable_tags, 0);

        assert!((analysis.optional_percentage() - 0.0).abs() < f64::EPSILON);
        assert!((analysis.platform_percentage() - 0.0).abs() < f64::EPSILON);
        assert!(!analysis.has_good_streaming_potential());
    }
}
