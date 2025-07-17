use crate::{Error, Md5, Result, MD5_LENGTH};
use std::io::{BufRead, ErrorKind};
use tracing::*;

/// Parser for TACT configuration files.
///
/// ## Format
///
/// ```text
/// # Comment to be ignored
///
/// option-name = value
/// another-option = many words value
/// integer-option = 1234
///
///
/// ```
///
/// Files often include trailing newline characters.
pub struct ConfigParser<T: BufRead> {
    inner: T,
}

impl<T: BufRead> ConfigParser<T> {
    pub fn new(inner: T) -> Self {
        ConfigParser { inner }
    }

    /// Get the next element from the file, or return `None` at EOF.
    ///
    /// The returned values will be pointers within the provided `buf`.
    ///
    /// Comments and empty lines will be automatically skipped.
    ///
    /// **Note:** Unlike [`BufRead::read_line()`], this will automatically clear
    /// `buf` each time it is called.
    pub fn next<'a>(&mut self, buf: &'a mut String) -> Result<Option<(&'a str, &'a str)>> {
        loop {
            buf.clear();
            match self.inner.read_line(buf) {
                Ok(0) => return Ok(None),
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
                Ok(_) => (),
            }

            let line = buf.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((k, v)) = buf.split_once('=') else {
                warn!("Cannot parse configuration line: {line:?}");
                return Err(Error::ConfigSyntax);
            };

            return Ok(Some((k.trim(), v.trim())));
        }
    }
}


/// Parse a single base16-encoded MD5 checksum from a string.
pub fn parse_md5_string(v: &str) -> Result<Md5> {
    let mut m = [0; MD5_LENGTH];
    hex::decode_to_slice(v, &mut m).map_err(|_| Error::ConfigTypeMismatch)?;
    Ok(m)
}

/// Parse a space-separated list of base16-encoded MD5 checksums from a string.
pub fn parse_md5s_string(v: &str) -> Result<Vec<Md5>> {
    let mut o = Vec::with_capacity(v.len() / (MD5_LENGTH * 2 + 1));
    for e in v.split_ascii_whitespace() {
        let mut m = [0; MD5_LENGTH];
        hex::decode_to_slice(e, &mut m).map_err(|_| Error::ConfigTypeMismatch)?;
        o.push(m);
    }

    Ok(o)
}

/// Parse a space-separated list of u32s from a string.
pub fn parse_u32s_string(v: &str) -> Result<Vec<u32>> {
    let mut o = Vec::new();
    for e in v.split_ascii_whitespace() {
        o.push(e.parse().map_err(|_| Error::ConfigTypeMismatch)?);
    }

    Ok(o)
}
