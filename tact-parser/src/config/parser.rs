use crate::{Error, MD5_HEX_LENGTH, MD5_LENGTH, MaybePair, Md5, Result};
use std::{
    future::Future,
    io::{BufRead, ErrorKind},
};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
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
pub struct ConfigParser<T> {
    inner: T,
}

impl<T> ConfigParser<T> {
    pub fn new(inner: T) -> Self {
        ConfigParser { inner }
    }
}

impl<T: BufRead> ConfigParser<T> {
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

impl<T: AsyncBufRead + Unpin> ConfigParser<T> {
    /// Get the next element from the file, or return `None` at EOF.
    ///
    /// The returned values will be pointers within the provided `buf`.
    ///
    /// Comments and empty lines will be automatically skipped.
    ///
    /// **Note:** Unlike [`BufRead::read_line()`], this will automatically clear
    /// `buf` each time it is called.
    pub async fn anext<'a>(&mut self, buf: &'a mut String) -> Result<Option<(&'a str, &'a str)>> {
        loop {
            buf.clear();
            match self.inner.read_line(buf).await {
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

/// Internal trait for parsable configuration files
pub(crate) trait ConfigParsableInternal: Default + Send {
    fn handle_kv(o: &mut Self, k: &str, v: &str) -> Result<()>;
}

/// Trait for parsable configuration files
pub trait ConfigParsable: Sized + Send {
    /// Parse a configuration from a [`BufRead`].
    fn parse_config<T: BufRead>(f: T) -> Result<Self>;

    /// Parse a configuration from a [`AsyncBufRead`].
    fn aparse_config<T: AsyncBufRead + Unpin + Send>(
        f: T,
    ) -> impl Future<Output = Result<Self>> + Send;
}

impl<U: ConfigParsableInternal> ConfigParsable for U {
    fn parse_config<T: BufRead>(f: T) -> Result<Self> {
        let mut parser = ConfigParser::new(f);
        let mut o = Self::default();
        let mut buf = String::with_capacity(4096);

        while let Some((k, v)) = parser.next(&mut buf)? {
            Self::handle_kv(&mut o, k, v)?;
        }

        Ok(o)
    }

    async fn aparse_config<T: AsyncBufRead + Unpin + Send>(f: T) -> Result<Self> {
        let mut parser = ConfigParser::new(f);
        let mut o = Self::default();
        let mut buf = String::with_capacity(4096);

        while let Some((k, v)) = parser.anext(&mut buf).await? {
            Self::handle_kv(&mut o, k, v)?;
        }

        Ok(o)
    }
}

/// Parse a single base16-encoded MD5 checksum from a string.
pub fn parse_md5_string(v: &str) -> Result<Md5> {
    let mut m = [0; MD5_LENGTH];
    hex::decode_to_slice(v, &mut m).map_err(|_| Error::ConfigTypeMismatch)?;
    Ok(m)
}

const TWO_MD5_HEX_LENGTH: usize = MD5_HEX_LENGTH * 2 + 1;

/// Parse one or two MD5 checksums, which are separated by a space.
pub fn parse_md5_maybepair_string(v: &str) -> Result<MaybePair<Md5>> {
    let v = v.trim();
    match v.len() {
        MD5_HEX_LENGTH => {
            // Single entry
            let mut a = [0; MD5_LENGTH];
            hex::decode_to_slice(v, &mut a).map_err(|_| Error::ConfigTypeMismatch)?;
            Ok(MaybePair::Solo(a))
        }

        TWO_MD5_HEX_LENGTH => {
            // Two entries
            Ok(parse_md5_pair_string(v)?.into())
        }

        _ => Err(Error::ConfigTypeMismatch),
    }
}

/// Parse two MD5 checksums, which are separated by a space.
pub fn parse_md5_pair_string(v: &str) -> Result<(Md5, Md5)> {
    let v = v.trim().as_bytes();
    if v.len() != TWO_MD5_HEX_LENGTH {
        return Err(Error::ConfigTypeMismatch);
    }

    if !v[MD5_HEX_LENGTH].is_ascii_whitespace() {
        return Err(Error::ConfigTypeMismatch);
    }

    let mut a = [0; MD5_LENGTH];
    let mut b = [0; MD5_LENGTH];
    hex::decode_to_slice(&v[..MD5_HEX_LENGTH], &mut a).map_err(|_| Error::ConfigTypeMismatch)?;
    hex::decode_to_slice(&v[MD5_HEX_LENGTH + 1..], &mut b)
        .map_err(|_| Error::ConfigTypeMismatch)?;
    Ok((a, b))
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

/// Parse one or two u32s, which are separated by a space.
pub fn parse_u32_maybepair_string(v: &str) -> Result<MaybePair<u32>> {
    let v = v.trim_ascii();
    if v.is_empty() {
        return Err(Error::ConfigTypeMismatch);
    }

    let mut o = Vec::with_capacity(2);
    for e in v.split_ascii_whitespace() {
        if o.len() >= 2 {
            // Too many entries
            return Err(Error::ConfigTypeMismatch);
        }

        o.push(e.parse().map_err(|_| Error::ConfigTypeMismatch)?);
    }

    match o.len() {
        1 => Ok(MaybePair::Solo(o[0])),
        2 => Ok(MaybePair::Pair(o[0], o[1])),
        _ => Err(Error::ConfigTypeMismatch),
    }
}

/// Parse two u32s, which are separated by a space.
pub fn parse_u32_pair_string(v: &str) -> Result<(u32, u32)> {
    let v = v.trim_ascii();
    if v.is_empty() {
        return Err(Error::ConfigTypeMismatch);
    }

    let mut o = Vec::with_capacity(2);
    for e in v.split_ascii_whitespace() {
        if o.len() >= 2 {
            // Too many entries
            return Err(Error::ConfigTypeMismatch);
        }

        o.push(e.parse().map_err(|_| Error::ConfigTypeMismatch)?);
    }

    match o.len() {
        2 => Ok((o[0], o[1])),
        _ => Err(Error::ConfigTypeMismatch),
    }
}

/// Parse a space-separated list of u32s from a string.
pub fn parse_u32s_string(v: &str) -> Result<Vec<u32>> {
    let mut o = Vec::new();
    for e in v.split_ascii_whitespace() {
        o.push(e.parse().map_err(|_| Error::ConfigTypeMismatch)?);
    }

    Ok(o)
}

/// Parse a space-separated list of `MD5:u32` pairs from a string.
///
/// eg: `md5a:123 md5b:456` => `vec![(md5a, 123), (md5b, 456)]`
pub fn parse_md5_u32_pair_string(v: &str) -> Result<Vec<(Md5, u32)>> {
    let v = v.trim();
    let spaces = v.chars().filter(|b| b.is_ascii_whitespace()).count();

    let mut o = Vec::with_capacity(spaces);

    for e in v.split_ascii_whitespace() {
        let (vm, ve) = e.split_once(':').ok_or(Error::ConfigTypeMismatch)?;
        let mut m = [0; MD5_LENGTH];
        hex::decode_to_slice(vm, &mut m).map_err(|_| Error::ConfigTypeMismatch)?;
        let u: u32 = ve.parse().map_err(|_| Error::ConfigTypeMismatch)?;

        o.push((m, u));
    }

    Ok(o)
}
