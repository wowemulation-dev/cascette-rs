mod error;
mod ioutils;
pub mod jenkins3;
pub mod wow_root;
pub mod utils;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
