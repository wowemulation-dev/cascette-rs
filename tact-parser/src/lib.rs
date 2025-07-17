pub mod archive;
pub mod blte;
pub mod config;
pub mod encoding;
mod error;
mod ioutils;
pub mod jenkins3;
pub mod utils;
pub mod wow_root;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

const MD5_LENGTH: usize = 16;
pub type Md5 = [u8; MD5_LENGTH];
const MD5_HEX_LENGTH: usize = MD5_LENGTH * 2;

/// An array which can have either 1 or 2 items of the same type.
#[derive(Debug, PartialEq, Eq)]
pub enum MaybePair<T> {
    Solo(T),
    Pair(T, T),
}

impl<T> MaybePair<T> {
    /// Get a reference to the first item.
    pub fn first(&self) -> &T {
        match self {
            Self::Solo(a) => a,
            Self::Pair(a, _) => a,
        }
    }

    /// Get a reference to the second item, if it exists.
    pub fn second(&self) -> Option<&T> {
        match self {
            Self::Pair(_, b) => Some(b),
            _ => None,
        }
    }
}

impl<T> From<T> for MaybePair<T> {
    fn from(value: T) -> Self {
        Self::Solo(value)
    }
}

impl<T> From<(T, T)> for MaybePair<T> {
    fn from((a, b): (T, T)) -> Self {
        Self::Pair(a, b)
    }
}
