//! Build manifest collection type.
//!
//! `BuildManifests` aggregates all parsed manifest files needed for installation.
//! It is the integration point for `cascette-maintenance`'s preservation set.

use cascette_formats::config::{BuildConfig, CdnConfig};
use cascette_formats::download::DownloadManifest;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_formats::patch_index::PatchIndex;
use cascette_formats::root::RootFile;
use cascette_formats::size::SizeManifest;

/// All parsed manifests for a build.
///
/// After resolution, this struct contains everything needed to classify
/// artifacts and drive downloads. It is also exposed publicly so that
/// `cascette-maintenance` can iterate manifest entries for preservation.
#[allow(missing_debug_implementations)]
pub struct BuildManifests {
    /// Parsed build configuration.
    pub build_config: BuildConfig,

    /// Parsed CDN configuration.
    pub cdn_config: CdnConfig,

    /// Parsed encoding file (content key -> encoding key mapping).
    pub encoding: EncodingFile,

    /// Parsed root file (FDID -> content key mapping).
    pub root: RootFile,

    /// Parsed install manifest (files to install with tags).
    pub install: InstallManifest,

    /// Parsed download manifest (files with priority and archive location).
    pub download: DownloadManifest,

    /// Parsed size manifest (encoding key -> estimated size, with tags).
    /// Used as canonical tag source for download manifest filtering.
    /// `None` for older builds that lack a size reference in the build config.
    pub size: Option<SizeManifest>,

    /// Parsed patch index (source/target file pairs for delta patching).
    /// `None` when the build config has no `patch-index` entry (common for
    /// fresh installs and older builds like WoW Classic 1.13.x).
    pub patch_index: Option<PatchIndex>,
}
