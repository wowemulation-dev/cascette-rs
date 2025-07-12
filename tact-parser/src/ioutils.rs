//! Internal utility functions

use std::io::{Read, Result};

pub trait ReadInt {
    fn read_u8(&mut self) -> Result<u8>;
    // fn read_i16le(&mut self) -> Result<i16>;
    // fn read_u16le(&mut self) -> Result<u16>;
    fn read_i32le(&mut self) -> Result<i32>;
    fn read_u32le(&mut self) -> Result<u32>;
    fn read_u64le(&mut self) -> Result<u64>;
    
    fn read_u16be(&mut self) -> Result<u16>;
    fn read_u32be(&mut self) -> Result<u32>;
    fn read_u40be(&mut self) -> Result<u64>;
}

impl<T: Read> ReadInt for T {
    fn read_u64le(&mut self) -> Result<u64> {
        let mut b = [0; size_of::<u64>()];
        self.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    fn read_u40be(&mut self) -> Result<u64> {
        let mut b = [0; size_of::<u64>()];
        self.read_exact(&mut b[3..])?;
        Ok(u64::from_be_bytes(b))
    }

    fn read_u32be(&mut self) -> Result<u32> {
        let mut b = [0; size_of::<u32>()];
        self.read_exact(&mut b)?;
        Ok(u32::from_be_bytes(b))
    }

    fn read_u32le(&mut self) -> Result<u32> {
        let mut b = [0; size_of::<u32>()];
        self.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_i32le(&mut self) -> Result<i32> {
        let mut b = [0; size_of::<i32>()];
        self.read_exact(&mut b)?;
        Ok(i32::from_le_bytes(b))
    }

    fn read_u16be(&mut self) -> Result<u16> {
        let mut b = [0; size_of::<u16>()];
        self.read_exact(&mut b)?;
        Ok(u16::from_be_bytes(b))
    }

    fn read_u8(&mut self) -> Result<u8> {
        let mut b = [0; 1];
        self.read_exact(&mut b)?;
        Ok(b[0])
    }
}
