//! Pattern-based file extraction with glob, regex, and key matching support

use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum PatternError {
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
    #[error("Invalid content key format: {0}")]
    InvalidContentKey(String),
    #[error("Invalid encoding key format: {0}")]
    InvalidEncodingKey(String),
    #[error("Pattern type could not be determined: {0}")]
    UnknownPattern(String),
}

/// Types of patterns we can match against
#[derive(Debug, Clone)]
pub enum PatternType {
    /// Glob pattern (e.g., "*.dbc", "interface/**/*.lua")
    Glob(String),
    /// Regular expression (e.g., r"/sound/.*\.ogg$/")
    Regex(Regex),
    /// 32-character hex content key
    ContentKey(String),
    /// 18-character hex encoding key
    EncodingKey(String),
    /// Exact file path match
    FilePath(String),
}

impl PartialEq for PatternType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PatternType::Glob(a), PatternType::Glob(b)) => a == b,
            (PatternType::Regex(a), PatternType::Regex(b)) => a.as_str() == b.as_str(),
            (PatternType::ContentKey(a), PatternType::ContentKey(b)) => a == b,
            (PatternType::EncodingKey(a), PatternType::EncodingKey(b)) => a == b,
            (PatternType::FilePath(a), PatternType::FilePath(b)) => a == b,
            _ => false,
        }
    }
}

/// Configuration for pattern matching behavior
#[derive(Debug, Clone)]
pub struct PatternConfig {
    /// Case-sensitive matching (default: false)
    pub case_sensitive: bool,
    /// Maximum number of files to match per pattern (default: unlimited)
    pub max_matches_per_pattern: Option<usize>,
    /// Whether to include directories in matches (default: false)
    pub include_directories: bool,
    /// File extensions to prioritize when multiple matches exist
    pub priority_extensions: Vec<String>,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            max_matches_per_pattern: None,
            include_directories: false,
            priority_extensions: vec!["dbc".to_string(), "db2".to_string(), "lua".to_string()],
        }
    }
}

/// A compiled pattern ready for matching
#[derive(Debug)]
pub struct CompiledPattern {
    pub pattern_type: PatternType,
    pub original: String,
    pub config: PatternConfig,
}

/// Results from pattern matching
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// The file path that matched
    pub file_path: String,
    /// The pattern that caused the match
    pub pattern: String,
    /// Additional metadata about the match
    pub metadata: MatchMetadata,
}

/// Additional information about a pattern match
#[derive(Debug, Clone, Default)]
pub struct MatchMetadata {
    /// File size if known
    pub file_size: Option<u64>,
    /// Content key if known
    pub content_key: Option<String>,
    /// Encoding key if known
    pub encoding_key: Option<String>,
    /// File type detected from extension
    pub file_type: Option<String>,
    /// Priority score (higher = more important)
    pub priority_score: u32,
}

/// Pattern extraction engine
pub struct PatternExtractor {
    config: PatternConfig,
    compiled_patterns: Vec<CompiledPattern>,
}

impl PatternExtractor {
    /// Create a new pattern extractor with default configuration
    pub fn new() -> Self {
        Self {
            config: PatternConfig::default(),
            compiled_patterns: Vec::new(),
        }
    }

    /// Create a pattern extractor with custom configuration
    pub fn with_config(config: PatternConfig) -> Self {
        Self {
            config,
            compiled_patterns: Vec::new(),
        }
    }

    /// Add a pattern to the extractor
    pub fn add_pattern(&mut self, pattern: &str) -> Result<(), PatternError> {
        let pattern_type = self.detect_pattern_type(pattern)?;
        let compiled = CompiledPattern {
            pattern_type,
            original: pattern.to_string(),
            config: self.config.clone(),
        };

        info!("Added pattern: {} -> {:?}", pattern, compiled.pattern_type);
        self.compiled_patterns.push(compiled);
        Ok(())
    }

    /// Add multiple patterns at once
    pub fn add_patterns(&mut self, patterns: &[String]) -> Result<(), PatternError> {
        for pattern in patterns {
            self.add_pattern(pattern)?;
        }
        Ok(())
    }

    /// Detect what type of pattern this is
    fn detect_pattern_type(&self, pattern: &str) -> Result<PatternType, PatternError> {
        // Check if it's a regex pattern (starts with / and ends with /)
        if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
            let regex_str = &pattern[1..pattern.len() - 1];
            let regex = if self.config.case_sensitive {
                Regex::new(regex_str)?
            } else {
                Regex::new(&format!("(?i){regex_str}"))?
            };
            return Ok(PatternType::Regex(regex));
        }

        // Check if it's a content key (32 hex characters)
        if pattern.len() == 32 && pattern.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(PatternType::ContentKey(pattern.to_lowercase()));
        }

        // Check if it's an encoding key (18 hex characters)
        if pattern.len() == 18 && pattern.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(PatternType::EncodingKey(pattern.to_lowercase()));
        }

        // Check if it contains glob characters
        if pattern.contains('*')
            || pattern.contains('?')
            || pattern.contains('[')
            || pattern.contains('{')
        {
            return Ok(PatternType::Glob(pattern.to_string()));
        }

        // Default to file path
        Ok(PatternType::FilePath(pattern.to_string()))
    }

    /// Match patterns against a list of file paths
    pub fn match_files(&self, file_paths: &[String]) -> Vec<PatternMatch> {
        let mut matches = Vec::new();
        let mut seen_files = HashSet::new();

        info!(
            "Matching {} patterns against {} files",
            self.compiled_patterns.len(),
            file_paths.len()
        );

        for compiled_pattern in &self.compiled_patterns {
            let pattern_matches = self.match_pattern(compiled_pattern, file_paths);

            debug!(
                "Pattern '{}' matched {} files",
                compiled_pattern.original,
                pattern_matches.len()
            );

            // Apply limits and deduplication
            let mut added_for_pattern = 0;
            for mut pattern_match in pattern_matches {
                if seen_files.contains(&pattern_match.file_path) {
                    continue;
                }

                // Apply per-pattern limit
                if let Some(limit) = compiled_pattern.config.max_matches_per_pattern {
                    if added_for_pattern >= limit {
                        debug!(
                            "Reached limit of {} matches for pattern '{}'",
                            limit, compiled_pattern.original
                        );
                        break;
                    }
                }

                // Calculate priority score
                pattern_match.metadata.priority_score = self.calculate_priority(&pattern_match);

                seen_files.insert(pattern_match.file_path.clone());
                matches.push(pattern_match);
                added_for_pattern += 1;
            }
        }

        // Sort by priority score (descending)
        matches.sort_by(|a, b| b.metadata.priority_score.cmp(&a.metadata.priority_score));

        info!("Total matches found: {}", matches.len());
        matches
    }

    /// Match a single compiled pattern against file paths
    fn match_pattern(
        &self,
        compiled_pattern: &CompiledPattern,
        file_paths: &[String],
    ) -> Vec<PatternMatch> {
        match &compiled_pattern.pattern_type {
            PatternType::Glob(glob_pattern) => {
                self.match_glob_pattern(glob_pattern, file_paths, &compiled_pattern.original)
            }
            PatternType::Regex(regex) => {
                self.match_regex_pattern(regex, file_paths, &compiled_pattern.original)
            }
            PatternType::ContentKey(ckey) => {
                self.match_content_key(ckey, &compiled_pattern.original)
            }
            PatternType::EncodingKey(ekey) => {
                self.match_encoding_key(ekey, &compiled_pattern.original)
            }
            PatternType::FilePath(path) => {
                self.match_file_path(path, file_paths, &compiled_pattern.original)
            }
        }
    }

    /// Match glob patterns like "*.dbc" or "interface/**/*.lua"
    fn match_glob_pattern(
        &self,
        glob_pattern: &str,
        file_paths: &[String],
        original: &str,
    ) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        // Convert glob to regex
        let regex_pattern = self.glob_to_regex(glob_pattern);
        let regex = match Regex::new(&regex_pattern) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Failed to compile glob pattern '{}' to regex: {}",
                    glob_pattern, e
                );
                return matches;
            }
        };

        for file_path in file_paths {
            let test_path = if self.config.case_sensitive {
                file_path.clone()
            } else {
                file_path.to_lowercase()
            };

            if regex.is_match(&test_path) {
                matches.push(PatternMatch {
                    file_path: file_path.clone(),
                    pattern: original.to_string(),
                    metadata: self.create_metadata_for_file(file_path),
                });
            }
        }

        matches
    }

    /// Match regex patterns
    fn match_regex_pattern(
        &self,
        regex: &Regex,
        file_paths: &[String],
        original: &str,
    ) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        for file_path in file_paths {
            if regex.is_match(file_path) {
                matches.push(PatternMatch {
                    file_path: file_path.clone(),
                    pattern: original.to_string(),
                    metadata: self.create_metadata_for_file(file_path),
                });
            }
        }

        matches
    }

    /// Match content keys (would need manifest integration)
    fn match_content_key(&self, _ckey: &str, original: &str) -> Vec<PatternMatch> {
        // For now, create a placeholder match
        // In full implementation, would resolve via encoding/root files
        vec![PatternMatch {
            file_path: format!("content_key_{_ckey}.data"),
            pattern: original.to_string(),
            metadata: MatchMetadata {
                content_key: Some(_ckey.to_string()),
                priority_score: 100, // High priority for direct keys
                ..Default::default()
            },
        }]
    }

    /// Match encoding keys (would need manifest integration)
    fn match_encoding_key(&self, _ekey: &str, original: &str) -> Vec<PatternMatch> {
        // For now, create a placeholder match
        // In full implementation, would resolve via encoding file
        vec![PatternMatch {
            file_path: format!("encoding_key_{_ekey}.data"),
            pattern: original.to_string(),
            metadata: MatchMetadata {
                encoding_key: Some(_ekey.to_string()),
                priority_score: 90, // High priority for direct keys
                ..Default::default()
            },
        }]
    }

    /// Match exact file paths
    fn match_file_path(
        &self,
        target_path: &str,
        file_paths: &[String],
        original: &str,
    ) -> Vec<PatternMatch> {
        let mut matches = Vec::new();

        let normalized_target = self.normalize_path(target_path);

        for file_path in file_paths {
            let normalized_file = self.normalize_path(file_path);

            if normalized_target == normalized_file {
                matches.push(PatternMatch {
                    file_path: file_path.clone(),
                    pattern: original.to_string(),
                    metadata: self.create_metadata_for_file(file_path),
                });
            }
        }

        matches
    }

    /// Convert glob pattern to regex
    fn glob_to_regex(&self, glob: &str) -> String {
        let mut regex = String::new();
        let mut chars = glob.chars().peekable();

        regex.push('^');

        while let Some(ch) = chars.next() {
            match ch {
                '*' => {
                    if chars.peek() == Some(&'*') {
                        chars.next(); // consume second *
                        if chars.peek() == Some(&'/') {
                            chars.next(); // consume /
                            regex.push_str("(?:[^/]+/)*"); // match any number of path segments
                        } else {
                            regex.push_str(".*"); // match everything
                        }
                    } else {
                        regex.push_str("[^/]*"); // match everything except path separator
                    }
                }
                '?' => regex.push_str("[^/]"),
                '[' => {
                    regex.push('[');
                    // Copy character class
                    for ch in chars.by_ref() {
                        regex.push(ch);
                        if ch == ']' {
                            break;
                        }
                    }
                }
                '{' => {
                    // Convert {a,b,c} to (a|b|c)
                    regex.push('(');
                    for ch in chars.by_ref() {
                        if ch == '}' {
                            break;
                        } else if ch == ',' {
                            regex.push('|');
                        } else {
                            if "^$()[]{}|+.\\".contains(ch) {
                                regex.push('\\');
                            }
                            regex.push(ch);
                        }
                    }
                    regex.push(')');
                }
                // Escape regex special characters
                ch if "^$()[]{}|+.\\".contains(ch) => {
                    regex.push('\\');
                    regex.push(ch);
                }
                ch => regex.push(ch),
            }
        }

        regex.push('$');

        if !self.config.case_sensitive {
            format!("(?i){regex}")
        } else {
            regex
        }
    }

    /// Normalize path for comparison
    fn normalize_path(&self, path: &str) -> String {
        let mut normalized = path.replace('\\', "/");
        if !self.config.case_sensitive {
            normalized = normalized.to_lowercase();
        }
        normalized
    }

    /// Create metadata for a file path
    fn create_metadata_for_file(&self, file_path: &str) -> MatchMetadata {
        let file_type = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());

        MatchMetadata {
            file_type,
            ..Default::default()
        }
    }

    /// Calculate priority score for a match
    fn calculate_priority(&self, pattern_match: &PatternMatch) -> u32 {
        let mut score = 10; // Base score

        // Boost priority for certain file extensions
        if let Some(file_type) = &pattern_match.metadata.file_type {
            if self.config.priority_extensions.contains(file_type) {
                score += 50;
            }

            // Additional boosts for specific file types
            score += match file_type.as_str() {
                "dbc" | "db2" => 40, // Database files
                "lua" | "xml" => 30, // Interface files
                "ogg" | "mp3" => 20, // Audio files
                "blp" | "tga" => 20, // Image files
                "m2" | "wmo" => 25,  // 3D models
                _ => 0,
            };
        }

        // Boost for direct key matches
        if pattern_match.metadata.content_key.is_some() {
            score += 100;
        }
        if pattern_match.metadata.encoding_key.is_some() {
            score += 90;
        }

        score
    }

    /// Get statistics about the compiled patterns
    pub fn get_stats(&self) -> PatternStats {
        let mut stats = PatternStats::default();

        for pattern in &self.compiled_patterns {
            match &pattern.pattern_type {
                PatternType::Glob(_) => stats.glob_patterns += 1,
                PatternType::Regex(_) => stats.regex_patterns += 1,
                PatternType::ContentKey(_) => stats.content_keys += 1,
                PatternType::EncodingKey(_) => stats.encoding_keys += 1,
                PatternType::FilePath(_) => stats.file_paths += 1,
            }
        }

        stats.total_patterns = self.compiled_patterns.len();
        stats
    }
}

impl Default for PatternExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about compiled patterns
#[derive(Debug, Default)]
pub struct PatternStats {
    pub total_patterns: usize,
    pub glob_patterns: usize,
    pub regex_patterns: usize,
    pub content_keys: usize,
    pub encoding_keys: usize,
    pub file_paths: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_detection() {
        let extractor = PatternExtractor::new();

        // Test glob patterns
        assert!(matches!(
            extractor.detect_pattern_type("*.dbc").unwrap(),
            PatternType::Glob(_)
        ));
        assert!(matches!(
            extractor.detect_pattern_type("interface/**/*.lua").unwrap(),
            PatternType::Glob(_)
        ));

        // Test regex patterns
        assert!(matches!(
            extractor.detect_pattern_type("/sound/.*\\.ogg$/").unwrap(),
            PatternType::Regex(_)
        ));

        // Test content key (32 hex characters)
        assert!(matches!(
            extractor
                .detect_pattern_type("0123456789abcdef0123456789abcdef")
                .unwrap(),
            PatternType::ContentKey(_)
        ));

        // Test encoding key (18 hex characters)
        assert!(matches!(
            extractor.detect_pattern_type("0123456789abcdef01").unwrap(),
            PatternType::EncodingKey(_)
        ));

        // Test file path
        assert!(matches!(
            extractor
                .detect_pattern_type("world/maps/azeroth/azeroth.wdt")
                .unwrap(),
            PatternType::FilePath(_)
        ));
    }

    #[test]
    fn test_glob_matching() {
        let mut extractor = PatternExtractor::new();
        extractor.add_pattern("*.dbc").unwrap();

        let files = vec![
            "achievement.dbc".to_string(),
            "spell.dbc".to_string(),
            "item.db2".to_string(),
            "interface/framexml/uiparent.lua".to_string(),
        ];

        let matches = extractor.match_files(&files);
        assert_eq!(matches.len(), 2); // Only .dbc files should match

        assert!(matches.iter().any(|m| m.file_path == "achievement.dbc"));
        assert!(matches.iter().any(|m| m.file_path == "spell.dbc"));
    }

    #[test]
    fn test_regex_matching() {
        let mut extractor = PatternExtractor::new();
        extractor.add_pattern("/.*\\.lua$/").unwrap();

        let files = vec![
            "interface/framexml/uiparent.lua".to_string(),
            "scripts/addon.lua".to_string(),
            "spell.dbc".to_string(),
        ];

        let matches = extractor.match_files(&files);
        assert_eq!(matches.len(), 2); // Only .lua files should match
    }

    #[test]
    fn test_glob_to_regex_conversion() {
        let extractor = PatternExtractor::new();

        assert_eq!(extractor.glob_to_regex("*.dbc"), "(?i)^[^/]*\\.dbc$");
        assert_eq!(extractor.glob_to_regex("test?.txt"), "(?i)^test[^/]\\.txt$");
        assert_eq!(
            extractor.glob_to_regex("**/*.lua"),
            "(?i)^(?:[^/]+/)*[^/]*\\.lua$"
        );
    }

    #[test]
    fn test_priority_calculation() {
        let extractor = PatternExtractor::new();

        let dbc_match = PatternMatch {
            file_path: "spell.dbc".to_string(),
            pattern: "*.dbc".to_string(),
            metadata: MatchMetadata {
                file_type: Some("dbc".to_string()),
                ..Default::default()
            },
        };

        let score = extractor.calculate_priority(&dbc_match);
        assert!(score > 50); // Should have high priority for .dbc files
    }
}
