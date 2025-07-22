use crate::{Error, MaybePair, Md5, Result, config::parser::*};
use std::collections::BTreeMap;
use tracing::*;

/// [Build configuration][0] parser.
///
/// [0]: https://wowdev.wiki/TACT#Build_Config
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BuildConfig {
    pub root: Option<Md5>,

    pub install: Option<MaybePair<Md5>>,
    pub install_size: Option<MaybePair<u32>>,

    pub download: Option<MaybePair<Md5>>,
    pub download_size: Option<MaybePair<u32>>,

    pub size: Option<(Md5, Md5)>,
    pub size_size: Option<(u32, u32)>,

    pub encoding: Option<MaybePair<Md5>>,
    pub encoding_size: Option<MaybePair<u32>>,

    pub patch_index: Option<MaybePair<Md5>>,
    pub patch_index_size: Option<MaybePair<u32>>,

    pub patch: Option<Md5>,
    pub patch_size: Option<u32>,

    pub patch_config: Option<Md5>,

    pub build_name: Option<String>,
    pub build_uid: Option<String>,
    pub build_product: Option<String>,
    pub build_playbuild_installer: Option<String>,

    pub build_partial_priority: Option<Vec<(Md5, u32)>>,

    pub build_num: Option<u32>,
    pub build_attributes: Option<String>,
    pub build_branch: Option<String>,
    pub build_comments: Option<String>,
    pub build_creator: Option<String>,
    pub build_critical_patch_seqn: Option<u32>,
    // WC3: Maybe integer? "0"
    pub build_status: Option<String>,
    // WC3
    pub build_source_branch: Option<String>,
    // WC3: Looks like a SHA1 hash (ie: git/hg?)
    pub build_source_revision: Option<String>,
    // WC3
    pub build_data_branch: Option<String>,
    // WC3: Maybe integer?
    pub build_data_revision: Option<String>,
    pub build_token: Option<String>,

    pub vfs_root: Option<(Md5, Md5)>,
    pub vfs_root_size: Option<(u32, u32)>,

    /// VFS manifest.
    ///
    /// This uses the indexes from the original file, which normally start at 1.
    pub vfs: Option<BTreeMap<u16, (Md5, Md5)>>,

    /// VFS manifest size.
    ///
    /// This uses the indexes from the original file, which normally start at 1.
    pub vfs_size: Option<BTreeMap<u16, (u32, u32)>>,
}

impl ConfigParsableInternal for BuildConfig {
    fn handle_kv(o: &mut Self, k: &str, v: &str) -> Result<()> {
        let k = k.to_ascii_lowercase();
        match k.as_str() {
            "root" => {
                o.root = Some(parse_md5_string(v)?);
            }

            "install" => {
                o.install = Some(parse_md5_maybepair_string(v)?);
            }
            "install-size" => {
                o.install_size = Some(parse_u32_maybepair_string(v)?);
            }

            "download" => {
                o.download = Some(parse_md5_maybepair_string(v)?);
            }
            "download-size" => {
                o.download_size = Some(parse_u32_maybepair_string(v)?);
            }

            "size" => {
                o.size = Some(parse_md5_pair_string(v)?);
            }
            "size-size" => {
                o.size_size = Some(parse_u32_pair_string(v)?);
            }

            "encoding" => {
                o.encoding = Some(parse_md5_maybepair_string(v)?);
            }
            "encoding-size" => {
                o.encoding_size = Some(parse_u32_maybepair_string(v)?);
            }

            "patch-index" => {
                o.patch_index = Some(parse_md5_maybepair_string(v)?);
            }
            "patch-index-size" => {
                o.patch_index_size = Some(parse_u32_maybepair_string(v)?);
            }

            "patch" => {
                o.patch = Some(parse_md5_string(v)?);
            }
            "patch-size" => {
                o.patch_size = Some(v.parse().map_err(|_| Error::ConfigTypeMismatch)?);
            }
            "patch-config" => {
                o.patch_config = Some(parse_md5_string(v)?);
            }

            "build-attributes" => {
                o.build_attributes = Some(v.to_string());
            }
            "build-branch" => {
                o.build_branch = Some(v.to_string());
            }
            "build-comments" => {
                o.build_comments = Some(v.to_string());
            }
            "build-creator" => {
                o.build_creator = Some(v.to_string());
            }
            "build-critical-patch-seqn" => {
                o.build_critical_patch_seqn =
                    Some(v.parse().map_err(|_| Error::ConfigTypeMismatch)?);
            }
            "build-data-branch" => {
                o.build_data_branch = Some(v.to_string());
            }
            "build-data-revision" => {
                o.build_data_revision = Some(v.to_string());
            }
            "build-num" => {
                o.build_num = Some(v.parse().map_err(|_| Error::ConfigTypeMismatch)?);
            }
            "build-name" => {
                o.build_name = Some(v.to_string());
            }
            "build-source-branch" => {
                o.build_source_branch = Some(v.to_string());
            }
            "build-source-revision" => {
                o.build_source_revision = Some(v.to_string());
            }
            "build-status" => {
                o.build_status = Some(v.to_string());
            }
            "build-token" => {
                o.build_token = Some(v.to_string());
            }
            "build-uid" => {
                o.build_uid = Some(v.to_string());
            }
            "build-product" => {
                o.build_product = Some(v.to_string());
            }
            "build-playbuild-installer" => {
                o.build_playbuild_installer = Some(v.to_string());
            }
            "build-partial-priority" => {
                o.build_partial_priority = Some(parse_md5_u32_pair_string(v)?);
            }
            "vfs-root" => {
                o.vfs_root = Some(parse_md5_pair_string(v)?);
            }
            "vfs-root-size" => {
                o.vfs_root_size = Some(parse_u32_pair_string(v)?);
            }
            other => {
                // Handle `vfs-X` and `vfs-X-size`
                if !other.starts_with("vfs-") || other.len() <= 4 {
                    warn!("Unknown config key: {k:?}");
                    return Ok(());
                }

                let mut other = &other[4..];
                let is_size = other.ends_with("-size");
                if is_size {
                    other = &other[..other.len() - 5];
                }

                // Try to parse what's left as an integer
                let Ok(vfs_id) = other.parse::<u16>() else {
                    warn!("Unknown config key: {k:?}");
                    return Ok(());
                };

                if is_size {
                    let vfs_size = o.vfs_size.get_or_insert_default();
                    vfs_size.insert(vfs_id, parse_u32_pair_string(v)?);
                } else {
                    let vfs = o.vfs.get_or_insert_default();
                    vfs.insert(vfs_id, parse_md5_pair_string(v)?);
                }
            }
        }

        Ok(())
    }
}
