//! Internal utility functions

use std::io::{Error, Read};

/// Generic trait for reading integer types from a buffer.
#[allow(dead_code, reason = "big endian functions will be used later")]
pub trait ReadInt {
    /// Error type which can be returned on read failures.
    type Error;

    /// Read a `u8` from the buffer.
    fn read_u8(&mut self) -> Result<u8, Self::Error>;

    /// Read a little-endian `i32` from the buffer.
    fn read_i32le(&mut self) -> Result<i32, Self::Error>;

    /// Read a little-endian `u32` from the buffer.
    fn read_u32le(&mut self) -> Result<u32, Self::Error>;

    /// Read a little-endian `u64` from the buffer.
    fn read_u64le(&mut self) -> Result<u64, Self::Error>;

    /// Read a big-endian `u16` from the buffer.
    fn read_u16be(&mut self) -> Result<u16, Self::Error>;

    /// Read a big-endian `u32` from the buffer.
    fn read_u32be(&mut self) -> Result<u32, Self::Error>;

    /// Read a big-endian 40-bit unsigned integer from the buffer.
    fn read_u40be(&mut self) -> Result<u64, Self::Error>;
}

impl<T: Read> ReadInt for T {
    type Error = Error;

    fn read_u8(&mut self) -> Result<u8, Self::Error> {
        let mut b = [0; 1];
        self.read_exact(&mut b)?;
        Ok(b[0])
    }

    fn read_i32le(&mut self) -> Result<i32, Self::Error> {
        let mut b = [0; size_of::<i32>()];
        self.read_exact(&mut b)?;
        Ok(i32::from_le_bytes(b))
    }

    fn read_u32le(&mut self) -> Result<u32, Self::Error> {
        let mut b = [0; size_of::<u32>()];
        self.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_u64le(&mut self) -> Result<u64, Self::Error> {
        let mut b = [0; size_of::<u64>()];
        self.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    fn read_u16be(&mut self) -> Result<u16, Self::Error> {
        let mut b = [0; size_of::<u16>()];
        self.read_exact(&mut b)?;
        Ok(u16::from_be_bytes(b))
    }

    fn read_u32be(&mut self) -> Result<u32, Self::Error> {
        let mut b = [0; size_of::<u32>()];
        self.read_exact(&mut b)?;
        Ok(u32::from_be_bytes(b))
    }

    fn read_u40be(&mut self) -> Result<u64, Self::Error> {
        let mut b = [0; size_of::<u64>()];
        self.read_exact(&mut b[3..])?;
        Ok(u64::from_be_bytes(b))
    }
}
