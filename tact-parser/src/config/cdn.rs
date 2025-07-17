use crate::{Error, Md5, Result, config::parser::*};
use std::io::BufRead;
use tracing::*;

/// CDN configuration parser.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CdnConfig {
    /// C-Keys of all archives.
    pub archives: Option<Vec<Md5>>,

    pub archives_index_size: Option<Vec<u32>>,

    /// C-Key of the combined index file.
    pub archive_group: Option<Md5>,

    /// C-Keys of patch archives.
    pub patch_archives: Option<Vec<Md5>>,
    pub patch_archives_index_size: Option<Vec<u32>>,
    pub patch_archive_group: Option<Md5>,

    pub file_index: Option<Md5>,
    pub file_index_size: Option<u32>,
    pub patch_file_index: Option<Md5>,
    pub patch_file_index_size: Option<u32>,

    /// List of build configs this CDN config supports
    pub builds: Option<Vec<String>>,
}

impl CdnConfig {
    /// Get an iterator over both `archives` and `archives_index_size`, if both fields were provided.
    pub fn archives_with_index_size(&self) -> Option<impl Iterator<Item = (&Md5, u32)>> {
        if let (Some(archives), Some(archives_index_size)) =
            (&self.archives, &self.archives_index_size)
        {
            Some(archives.iter().zip(archives_index_size.iter().copied()))
        } else {
            None
        }
    }

    /// Get an iterator over both `patch_archives` and `patch_archives_index_size`, if both fields were provided.
    pub fn patch_archives_with_index_size(&self) -> Option<impl Iterator<Item = (&Md5, u32)>> {
        if let (Some(patch_archives), Some(patch_archives_index_size)) =
            (&self.patch_archives, &self.patch_archives_index_size)
        {
            Some(
                patch_archives
                    .iter()
                    .zip(patch_archives_index_size.iter().copied()),
            )
        } else {
            None
        }
    }

    pub fn parse_config<T: BufRead>(f: T) -> Result<Self> {
        Self::parse_config_inner(&mut ConfigParser::new(f))
    }

    fn parse_config_inner<T: BufRead>(parser: &mut ConfigParser<T>) -> Result<Self> {
        let mut o = CdnConfig::default();
        let mut buf = String::with_capacity(4096);

        while let Some((k, v)) = parser.next(&mut buf)? {
            let k = k.to_ascii_lowercase();
            match k.as_str() {
                "archives" => {
                    o.archives = Some(parse_md5s_string(v)?);
                }
                "archives-index-size" => {
                    o.archives_index_size = Some(parse_u32s_string(v)?);
                }
                "archive-group" => {
                    o.archive_group = Some(parse_md5_string(v)?);
                }
                "patch-archives" => {
                    o.patch_archives = Some(parse_md5s_string(v)?);
                }
                "patch-archives-index-size" => {
                    o.patch_archives_index_size = Some(parse_u32s_string(v)?);
                }
                "patch-archive-group" => {
                    o.patch_archive_group = Some(parse_md5_string(v)?);
                }
                "file-index" => {
                    o.file_index = Some(parse_md5_string(v)?);
                }
                "file-index-size" => {
                    o.file_index_size = Some(v.parse().map_err(|_| Error::ConfigTypeMismatch)?);
                }
                "patch-file-index" => {
                    o.patch_file_index = Some(parse_md5_string(v)?);
                }
                "patch-file-index-size" => {
                    o.patch_file_index_size =
                        Some(v.parse().map_err(|_| Error::ConfigTypeMismatch)?);
                }
                "builds" => {
                    o.builds = Some(v.split_ascii_whitespace().map(String::from).collect());
                }
                _ => {
                    warn!("Unknown config key: {k:?}");
                }
            }
        }

        Ok(o)
    }
}
