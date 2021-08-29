#[cfg(target_os = "linux")]
use std::io::Read;
use std::fs::File;
use anyhow::{Result, anyhow};
use std::io::{Cursor, Seek, SeekFrom};

pub struct Reader<IO : Read + Seek> {
    cursor : IO
}

impl From<Box<[u8]>> for Reader<Cursor<Vec<u8>>> {
    fn from(slice: Box<[u8]>) -> Self {
        Self {
            cursor: Cursor::new(slice.to_vec())
        }
    }
}

impl From<Vec<u8>> for Reader<Cursor<Vec<u8>>> {
    fn from(vec: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(vec)
        }
    }
}

pub const U16_MEM_LEN : usize = 2;
pub const I16_MEM_LEN : usize = 2;

pub const U32_MEM_LEN : usize = 4;
pub const I32_MEM_LEN : usize = 4;

pub const U64_MEM_LEN : usize = 8;
pub const I64_MEM_LEN : usize = 8;


impl<IO : Read + Seek> Reader<IO> {
    pub fn read_u16_le(&mut self) -> u16 {
        let mut raw_bytes = [0_u8; U16_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        u16::from_le_bytes(raw_bytes)
    }

    pub fn read_i16_le(&mut self) -> i16 {
        let mut raw_bytes = [0_u8; U16_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        i16::from_le_bytes(raw_bytes)
    }

    pub fn read_u32_le(&mut self) -> u32 {
        let mut raw_bytes = [0_u8; U32_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        u32::from_le_bytes(raw_bytes)
    }

    pub fn read_i32_le(&mut self) -> i32 {
        let mut raw_bytes = [0_u8; I32_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        i32::from_le_bytes(raw_bytes)
    }

    pub fn read_u64_le(&mut self) -> u64 {
        let mut raw_bytes = [0_u8; U64_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        u64::from_le_bytes(raw_bytes)
    }

    pub fn read_i64_le(&mut self) -> i64 {
        let mut raw_bytes = [0_u8; I64_MEM_LEN];
        self.cursor.read_exact(&mut raw_bytes);
        i64::from_le_bytes(raw_bytes)
    }

    pub fn skip(&mut self, n_bytes : i64) -> Result<()> {
        let _ = self.cursor.seek(SeekFrom::Current(n_bytes))?;
        Ok(())
    }
}
