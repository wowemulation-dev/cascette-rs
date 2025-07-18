//! [`wowdev/wow-listfile`][0] parser
//!
//! Community / verified listfile parser.
//!
//! The [root file][crate::wow_root] in current retail versions of the game have
//! less than 10% of filename hashes available.
//!
//! Also, with hashes, it is not possible to walk a directory tree.
//!
//! Instead, one can use the [community listfile][0] to get a list of file names
//! and their file IDs.
//!
//! # Note
//!
//! Community listfile names are generally invented by the community, as it is
//! often impossible to recover the original filename. As such, they are not
//! guaranteed to be stable, as someone may come up with a better name.
//!
//! [0]: https://github.com/wowdev/wow-listfile/

use tracing::warn;

use crate::{Error, Result};
use std::{
    collections::BTreeMap,
    io::{BufRead, ErrorKind},
    path::{Path, PathBuf},
};

/// Base listfile parser, which emits entries using an iterator-like interface.
/// 
/// If you're looking up many records, use [`ListfileNameResolver`] provides
/// [`BTreeMap`]-based lookups, which is faster than re-scanning the file.
pub struct ListfileParser<'a, T: BufRead + 'a> {
    inner: &'a mut T,
}

impl<'a, T: BufRead + 'a> ListfileParser<'a, T> {
    pub fn new(inner: &'a mut T) -> Self {
        Self { inner }
    }

    /// Get the next element from the file, or return `None` at EOF.
    ///
    /// Comments and empty lines will be automatically skipped.
    ///
    /// # Errors
    ///
    /// On encountering an error, the next call to [`next()`][Self::next] will
    /// read the next line of input.
    pub fn try_next(&mut self) -> Result<Option<(u32, PathBuf)>> {
        let mut buf = String::with_capacity(512);
        loop {
            buf.clear();
            match self.inner.read_line(&mut buf) {
                Ok(0) => return Ok(None),
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
                Ok(_) => (),
            }

            let line = buf.trim();
            if line.is_empty() {
                continue;
            }

            let Some((k, v)) = buf.split_once(';') else {
                warn!("Cannot parse configuration line: {line:?}");
                return Err(Error::ListfileSyntax);
            };

            let k = k.trim().parse().map_err(|_| Error::InvalidListfileID)?;
            // Ignore extra fields
            let v = v.split(';').next().unwrap().trim();
            let v = listfile_normalise(v);

            return Ok(Some((k, v)));
        }
    }
}

/// [`BTreeMap`]-based File ID/path resolver.
/// 
/// When you only need to look up a single record, it's more efficient to use
/// [`ListfileParser`] directly.
pub struct ListfileNameResolver {
    path_to_fid: BTreeMap<PathBuf, u32>,
    fid_to_path: BTreeMap<u32, PathBuf>,
}

impl ListfileNameResolver {
    pub fn new<T: BufRead>(f: &mut T) -> Result<Self> {
        // Read in all the entries
        let mut path_to_fid = BTreeMap::new();
        let mut fid_to_path = BTreeMap::new();

        let mut parser = ListfileParser::new(f);

        while let Some((fid, name)) = parser.try_next()? {
            path_to_fid.insert(name.clone(), fid);
            fid_to_path.insert(fid, name);
        }

        Ok(Self {
            path_to_fid,
            fid_to_path,
        })
    }

    /// Get the File ID of `path`.
    ///
    /// # Note
    ///
    /// `path` must be in lowercase.
    pub fn get_fid_from_path(&self, path: impl AsRef<Path>) -> Option<u32> {
        let mut path = path.as_ref().to_path_buf();
        path.as_mut_os_string().make_ascii_lowercase();
        self.path_to_fid.get(&path).copied()
    }

    /// Get the file path of `fid`.
    pub fn get_path_for_fid(&self, fid: u32) -> Option<&Path> {
        self.fid_to_path.get(&fid).map(|p| p.as_path())
    }

    pub fn len(&self) -> usize {
        self.path_to_fid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.path_to_fid.is_empty()
    }
}

/// Normalise a file path in a listfile.
///
/// All paths converted to lowercase.
pub fn listfile_normalise(i: &str) -> PathBuf {
    let mut o = PathBuf::new();
    for segment in i.split(&['/', '\\'][..]) {
        o.push(segment.to_ascii_lowercase())
    }
    o
}
