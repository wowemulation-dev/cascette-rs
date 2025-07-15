pub mod config;
pub mod encoding;
mod error;
mod ioutils;
pub mod jenkins3;
pub mod wow_root;
pub mod utils;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

const MD5_LENGTH: usize = 16;
pub type Md5 = [u8; MD5_LENGTH];
