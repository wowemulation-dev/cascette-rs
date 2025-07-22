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

use crate::{Error, Result};
use std::{
    collections::BTreeMap,
    io::{BufRead, ErrorKind},
};
use tokio::io::AsyncBufReadExt;
use tracing::warn;

/// Base listfile parser, which emits entries using an iterator-like interface.
///
/// If you're looking up many records, use [`ListfileNameResolver`] provides
/// [`BTreeMap`]-based lookups, which is faster than re-scanning the file.
pub struct ListfileParser<'a, T: 'a> {
    inner: &'a mut T,
}

impl<'a, T: 'a> ListfileParser<'a, T> {
    fn try_next_inner(buf: &str) -> Result<Option<(u32, String)>> {
        let line = buf.trim();
        if line.is_empty() {
            return Ok(None);
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

impl<'a, T: BufRead + 'a> ListfileParser<'a, T> {
    /// Create a new listfile parser.
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
    pub fn try_next(&mut self) -> Result<Option<(u32, String)>> {
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

            if let Some(entry) = Self::try_next_inner(&buf)? {
                return Ok(Some(entry));
            }
        }
    }
}

impl<'a, T: AsyncBufReadExt + Unpin + 'a> ListfileParser<'a, T> {
    /// Create a new asynchronous listfile parser.
    pub fn anew(inner: &'a mut T) -> Self {
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
    pub async fn atry_next(&mut self) -> Result<Option<(u32, String)>> {
        let mut buf = String::with_capacity(512);
        loop {
            buf.clear();
            match self.inner.read_line(&mut buf).await {
                Ok(0) => return Ok(None),
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
                Ok(_) => (),
            }

            if let Some(entry) = Self::try_next_inner(&buf)? {
                return Ok(Some(entry));
            }
        }
    }
}

/// [`BTreeMap`]-based File ID/path resolver.
///
/// When you only need to look up a single record, it's more efficient to use
/// [`ListfileParser`] directly.
pub struct ListfileNameResolver {
    path_to_fid: BTreeMap<String, u32>,
    fid_to_path: BTreeMap<u32, String>,
}

impl ListfileNameResolver {
    /// Create a ListfileNameResolver from a synchronous file handle.
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

    /// Create a ListfileNameResolver from an asynchronous file handle.
    pub async fn anew<T: AsyncBufReadExt + Unpin>(f: &mut T) -> Result<Self> {
        // Read in all the entries
        let mut path_to_fid = BTreeMap::new();
        let mut fid_to_path = BTreeMap::new();

        let mut parser = ListfileParser::anew(f);

        while let Some((fid, name)) = parser.atry_next().await? {
            path_to_fid.insert(name.clone(), fid);
            fid_to_path.insert(fid, name);
        }

        Ok(Self {
            path_to_fid,
            fid_to_path,
        })
    }

    /// Get the File ID of `path`.
    pub fn get_fid_from_path(&self, path: &str) -> Option<u32> {
        let path = listfile_normalise(path);
        self.path_to_fid.get(&path).copied()
    }

    /// Get the file path of `fid`.
    pub fn get_path_for_fid(&self, fid: u32) -> Option<&str> {
        self.fid_to_path.get(&fid).map(|s| s.as_str())
    }

    /// The number of entries in this resolver.
    pub fn len(&self) -> usize {
        self.path_to_fid.len()
    }

    /// `true` if there are no mapping entries.
    pub fn is_empty(&self) -> bool {
        self.path_to_fid.is_empty()
    }

    /// Iterate over all `(fid, path)` entries.
    ///
    /// Using [`ListfileParser`][] directly is generally more efficient than
    /// this.
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &String)> {
        self.fid_to_path.iter()
    }

    /// Provides direct access to the `fid` to `path` map.
    pub fn fid_to_path(&self) -> &BTreeMap<u32, String> {
        &self.fid_to_path
    }

    /// Provides direct access to the `path` to `fid` map.
    pub fn path_to_fid(&self) -> &BTreeMap<String, u32> {
        &self.path_to_fid
    }
}

/// Normalises a path in the listfile.
pub fn listfile_normalise(i: impl AsRef<str>) -> String {
    const OTHER_ALLOWED_CHARS: &str = "_-./";
    let i = i.as_ref();

    // Normalise directory separators, force lowercase, and remove
    // non-alphanumeric / _ / - characters.
    let i = i.replace('\\', "/").to_ascii_lowercase().replace(
        |c: char| !c.is_ascii_alphanumeric() && !OTHER_ALLOWED_CHARS.contains(c),
        "",
    );

    // Filter empty path segments, and remove trailing/leading dots
    let mut components = Vec::new();
    for p in i.split('/') {
        let p = p.trim_matches('.');
        if p.is_empty() {
            continue;
        }
        components.push(p);
    }

    components.join("/")
}
