//! Content categorization for CASC files.
//!
//! Classifies files by extension and path prefix into categories that
//! determine compression and encryption behavior.

use std::path::Path;

/// Content category for a file in the CASC archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentCategory {
    /// Executable binaries and DLLs.
    Executable,
    /// Audio files (mp3, ogg, wav).
    Audio,
    /// Texture and model files (blp, m2, wmo, skin).
    Graphics,
    /// UI layout, Lua scripts, XML, fonts, and icons.
    Interface,
    /// Database files (DB2, DBC) and configuration.
    Data,
    /// Map, WDT, WDL, ADT terrain data.
    WorldData,
    /// Unrecognized file type.
    Unknown,
}

impl ContentCategory {
    /// Classify a file path into a content category.
    ///
    /// Uses the file extension as the primary signal, with a path prefix
    /// check for `interface/` paths that would otherwise be classified by
    /// extension alone.
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_ascii_lowercase();

        // Interface prefix overrides extension-based classification for
        // files under the interface/ directory tree (icons, layouts, Lua).
        if lower.starts_with("interface/") {
            return Self::Interface;
        }

        let ext = Path::new(&lower)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "exe" | "dll" | "so" | "dylib" => Self::Executable,
            "mp3" | "ogg" | "wav" | "flac" => Self::Audio,
            "blp" | "m2" | "wmo" | "skin" | "anim" | "phys" | "bone" | "tex" => Self::Graphics,
            "lua" | "xml" | "toc" | "ttf" | "otf" => Self::Interface,
            "db2" | "dbc" | "csv" | "txt" | "cfg" | "wtf" | "tbl" | "sbt" => Self::Data,
            "adt" | "wdt" | "wdl" | "bls" | "tex2" => Self::WorldData,
            _ => Self::Unknown,
        }
    }

    /// Whether files of this category are likely encrypted in CASC.
    ///
    /// Executables and data tables are frequently encrypted.
    /// Audio and world data are rarely encrypted.
    pub fn is_likely_encrypted(self) -> bool {
        matches!(self, Self::Executable | Self::Data)
    }

    /// Whether files of this category benefit from compression.
    ///
    /// Audio files are already compressed (mp3, ogg) and gain little
    /// from additional compression. All other categories benefit.
    pub fn benefits_from_compression(self) -> bool {
        !matches!(self, Self::Audio)
    }
}

/// Metadata about a file's content derived from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentInfo {
    /// Content category.
    pub category: ContentCategory,
    /// File extension (lowercase, without dot), empty if none.
    pub extension: String,
    /// File name without directory.
    pub filename: String,
    /// Directory portion of the path.
    pub directory: String,
}

impl ContentInfo {
    /// Build content info from a file path.
    pub fn from_path(path: &str) -> Self {
        let category = ContentCategory::from_path(path);

        let extension = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let filename = path
            .rsplit_once('/')
            .map_or(path, |(_, name)| name)
            .to_string();

        let directory = path
            .rsplit_once('/')
            .map_or(String::new(), |(dir, _)| dir.to_string());

        Self {
            category,
            extension,
            filename,
            directory,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_category_from_extension() {
        assert_eq!(
            ContentCategory::from_path("wow.exe"),
            ContentCategory::Executable
        );
        assert_eq!(
            ContentCategory::from_path("sound/music/zone.mp3"),
            ContentCategory::Audio
        );
        assert_eq!(
            ContentCategory::from_path("creature/human/humanmale.m2"),
            ContentCategory::Graphics
        );
        assert_eq!(
            ContentCategory::from_path("scripts/addon.lua"),
            ContentCategory::Interface
        );
        assert_eq!(
            ContentCategory::from_path("dbfilesclient/spell.db2"),
            ContentCategory::Data
        );
        assert_eq!(
            ContentCategory::from_path("world/maps/azeroth/azeroth_32_32.adt"),
            ContentCategory::WorldData
        );
        assert_eq!(
            ContentCategory::from_path("unknown/file.xyz"),
            ContentCategory::Unknown
        );
    }

    #[test]
    fn test_interface_prefix_override() {
        // A .blp file under interface/ should be Interface, not Graphics
        assert_eq!(
            ContentCategory::from_path("interface/icons/inv_misc_questionmark.blp"),
            ContentCategory::Interface
        );
        assert_eq!(
            ContentCategory::from_path("Interface/FrameXML/UIParent.lua"),
            ContentCategory::Interface
        );
    }

    #[test]
    fn test_likely_encrypted() {
        assert!(ContentCategory::Executable.is_likely_encrypted());
        assert!(ContentCategory::Data.is_likely_encrypted());
        assert!(!ContentCategory::Audio.is_likely_encrypted());
        assert!(!ContentCategory::Graphics.is_likely_encrypted());
        assert!(!ContentCategory::WorldData.is_likely_encrypted());
    }

    #[test]
    fn test_benefits_from_compression() {
        assert!(!ContentCategory::Audio.benefits_from_compression());
        assert!(ContentCategory::Executable.benefits_from_compression());
        assert!(ContentCategory::Graphics.benefits_from_compression());
        assert!(ContentCategory::Data.benefits_from_compression());
        assert!(ContentCategory::WorldData.benefits_from_compression());
    }

    #[test]
    fn test_content_info_from_path() {
        let info = ContentInfo::from_path("world/maps/azeroth/azeroth.wmo");
        assert_eq!(info.category, ContentCategory::Graphics);
        assert_eq!(info.extension, "wmo");
        assert_eq!(info.filename, "azeroth.wmo");
        assert_eq!(info.directory, "world/maps/azeroth");
    }

    #[test]
    fn test_content_info_no_directory() {
        let info = ContentInfo::from_path("readme.txt");
        assert_eq!(info.category, ContentCategory::Data);
        assert_eq!(info.filename, "readme.txt");
        assert_eq!(info.directory, "");
    }

    #[test]
    fn test_content_info_no_extension() {
        let info = ContentInfo::from_path("data/somefile");
        assert_eq!(info.category, ContentCategory::Unknown);
        assert_eq!(info.extension, "");
        assert_eq!(info.filename, "somefile");
    }
}
