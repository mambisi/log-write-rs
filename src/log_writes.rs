use std::path::Path;
use std::fs::{File, OpenOptions};
use std::io::{Read, Cursor, Seek, SeekFrom};
use bytes::{Bytes, Buf};
use crate::reader::Reader;
use anyhow::{Result, bail, anyhow, Error};
use crate::io;
use crate::util;
use std::cmp::min;
use std::os::unix::io::{AsRawFd, RawFd};

pub const LOG_FLUSH_FLAG: u64 = 1 << 0;
pub const LOG_FUA_FLAG: u64 = 1 << 1;
pub const LOG_DISCARD_FLAG: u64 = 1 << 2;
pub const LOG_MARK_FLAG: u64 = 1 << 3;
pub const LOG_METADATA_FLAG: u64 = 1 << 4;

pub const WRITE_LOG_VERSION: u64 = 1;
pub const WRITE_LOG_MAGIC: u64 = 0x6a736677736872;

#[derive(Debug, Copy, Clone)]
pub struct LogWriteSuper {
    pub magic: u64,
    pub version: u64,
    pub nr_entries: u64,
    pub sector_size: u32,
}

impl From<[u8; 32]> for LogWriteSuper {
    fn from(buf: [u8; 32]) -> Self {
        let mut rdr = Reader::from(buf.to_vec());
        let magic = rdr.read_u64_le();
        let version = rdr.read_u64_le();
        let nr_entries = rdr.read_u64_le();
        let _ = rdr.skip(4);
        let sector_size = rdr.read_u32_le();
        Self {
            magic,
            version,
            nr_entries,
            sector_size,
        }
    }
}

impl Default for LogWriteSuper {
    fn default() -> Self {
        Self {
            magic: 0,
            version: 0,
            nr_entries: 0,
            sector_size: 0,
        }
    }
}

pub struct FlagsToStrEntry {
    flags: u64,
    str: String,
}

macro_rules! log_flags_str_entry {
    ($f : ident, $s : expr ) => {
        FlagsToStrEntry {
            flags : $f,
            str : $s.to_string()
        }
    };
}

#[inline]
fn log_flags_table() -> Vec<FlagsToStrEntry> {
    vec![
        log_flags_str_entry!(LOG_FLUSH_FLAG, "FLUSH"),
        log_flags_str_entry!(LOG_FUA_FLAG, "FUA"),
        log_flags_str_entry!(LOG_DISCARD_FLAG, "DISCARD"),
        log_flags_str_entry!(LOG_MARK_FLAG, "MARK"),
        log_flags_str_entry!(LOG_METADATA_FLAG, "METADATA")
    ]
}


#[derive(Default)]
pub struct LogWriteEntry {
    pub sector: u64,
    pub nr_sectors: u64,
    pub flags: u64,
    pub data_len: u64,
}

impl From<Vec<u8>> for LogWriteEntry {
    fn from(buf: Vec<u8>) -> Self {
        let mut rdr = Reader::from(buf);
        let sector = rdr.read_u64_le();
        let nr_sectors = rdr.read_u64_le();
        let flags = rdr.read_u64_le();
        let data_len = rdr.read_u64_le();
        Self {
            sector,
            nr_sectors,
            flags,
            data_len,
        }
    }
}

pub const LOG_IGNORE_DISCARD: u64 = 1 << 0;
pub const LOG_DISCARD_NOT_SUPP: u64 = 1 << 1;
pub const LOG_FLAGS_BUF_SIZE: usize = 128;

pub struct Log {
    pub log_file: File,
    pub replay_file: File,
    pub flags: u64,
    pub nr_entries: u64,
    pub sector_size: u32,
    pub cur_entry: u64,
    pub max_zero_size: u64,
    pub cur_pos: u64,
}

pub trait MemSize {
    fn mem_size() -> usize;
}

pub fn entry_flags_to_str(flags: u64, buf: &mut String) {
    let mut flags = flags;
    let log_flags_table = log_flags_table();
    let mut empty = true;
    for i in log_flags_table {
        if (flags & i.flags) > 0 {
            if !empty {
                util::strncat(buf, "|".to_string(), LOG_FLAGS_BUF_SIZE);
            }
            empty = false;
            util::strncat(buf, i.str, LOG_FLAGS_BUF_SIZE);
            flags &= !i.flags;
        }
    }

    if flags > 0 {
        if !empty {
            util::strncat(buf, "|".to_string(), LOG_FLAGS_BUF_SIZE);
        }
        empty = false;
        let left_len = LOG_FLAGS_BUF_SIZE - min(buf.len(), LOG_FLAGS_BUF_SIZE);
        if left_len > 0 {
            println!("UNKNOWN.{}", flags)
        }
    }

    if empty {
        buf.clear();
        buf.push_str("None");
    }
}

impl MemSize for LogWriteEntry {
    fn mem_size() -> usize {
        std::mem::size_of_val(&LogWriteEntry::default())
    }
}


impl Log {

    fn discard_range(&mut self, start : u64, len : u64) -> i32 {
        let range : [u64;2] = [start, len];
        let ret = unsafe {
            ioctls::blkdiscard(self.replay_file.as_raw_fd(), &range)
        };
        if ret < 0 {
            println!("replay device doesn't support discard, switching to writing zeros");
            self.flags |= LOG_DISCARD_NOT_SUPP;
        }
        return 0
    }
    fn zero_range(&mut self, start : u64, len : u64) -> i32 {
        let mut start = start as usize;
        let mut len = len as usize;
        let mut ret : usize = 0;
        let mut bufsize : usize = len;
        if self.max_zero_size < len as u64{
            println!("discard len {} larger than max {}", len, self.max_zero_size);
            return 0;
        }

        let mut buf : Vec<u8> = Vec::with_capacity(len);
        if buf.capacity() != len as usize {
            eprintln!("Couldn't allocate zero buffer");
            return -1;
        }

        buf.fill(0);

        while len > 0 {
            ret = match io::pwrite(&self.replay_file, buf.as_slice(), start as  i64){
                Ok(ret) => {
                    ret
                }
                Err(error) => {
                    eprintln!("Error zeroing file {}", error);
                    return -1
                }
            };
            if ret != bufsize {
                eprintln!("Error zeroing file");
                return -1;
            }
            len -= ret;
            start += ret;
        }
        return 0;
    }

    fn discard(&mut self, entry: &LogWriteEntry) -> Result<()> {
        let mut start = entry.sector * self.sector_size as u64;
        let mut size = entry.nr_sectors * self.sector_size as u64;
        let max_chunk: u64 = 1 * 1024 * 1024 * 1024;

        if (self.flags & LOG_DISCARD_FLAG) != 0 {
            return Ok(());
        }

        while size > 0 {
            let len = min(max_chunk, size);
            let ret : i32;
            if (self.flags & LOG_DISCARD_NOT_SUPP) <= 0 {
                ret = self.discard_range(start, len)
            }
            if (self.flags & LOG_DISCARD_NOT_SUPP) > 0 {
                ret = self.zero_range(start, len)
            }

            if ret > 0 {
                bail!("Discard error")
            }
        }
        Ok(())
    }

    pub fn open<P: AsRef<Path>>(log_file_path: P, replay_file_path: P) -> Result<Self> {
        let mut log_file = OpenOptions::new().read(true).write(false).open(log_file_path)?;
        let replay_file = OpenOptions::new().write(true).read(false).open(replay_file_path)?;

        let mut buf = [0_u8; 32];
        io::read(&replay_file, &mut buf)?;
        let log_super = LogWriteSuper::from(buf);

        if log_super.magic == WRITE_LOG_MAGIC {
            bail!("Magic doesn't match")
        }

        // Seek to first log entry
        let _ = log_file.seek(SeekFrom::Current(std::mem::size_of_val(&log_super) as i64)).map_err(|error| {
            anyhow!("Error seeking to first entry: {}", error)
        })?;

        Ok(Self {
            log_file,
            replay_file,
            flags: 0,
            nr_entries: log_super.nr_entries,
            sector_size: log_super.sector_size,
            cur_entry: 0,
            max_zero_size: 128 * 1024 * 1024,
            cur_pos: 0,
        })
    }

    pub fn replay_next_entry(&mut self, read_data: bool) -> Result<Option<LogWriteEntry>> {
        let read_size = if read_data {
            self.sector_size as usize
        } else {
            std::mem::size_of_val(&LogWriteEntry::default())
        };

        let mut raw_log_entry = vec![0_u8; read_size];

        if self.cur_entry >= self.nr_entries {
            return Ok(None);
        }

        let mut ret = io::read(&self.log_file, &mut raw_log_entry)?;
        if ret != read_size as usize {
            bail!("Error reading entry: {}", ret)
        }
        let entry = LogWriteEntry::from(raw_log_entry);
        self.cur_entry += 1;

        let size = (entry.nr_sectors * self.sector_size as u64) as usize;
        if read_size < self.sector_size as usize {
            self.log_file.seek(SeekFrom::Current(LogWriteEntry::mem_size() as i64))?;
        }

        let mut flag_buf = String::new();
        let flags = entry.flags;
        entry_flags_to_str(flags, &mut flag_buf);

        println!("replaying {}: sector {}, size {}, flags {}({})", self.cur_entry - 1, entry.sector, size, flags, flag_buf);

        if size > 0 {
            return Ok(Some(entry));
        }

        if (flags & LOG_DISCARD_FLAG) > 0 {
            self.discard(&entry);
            return Ok(Some(entry))
        }

        let mut buf: Vec<u8> = Vec::with_capacity(size);
        if buf.capacity() != size {
            bail!("Error allocating buffer {} entry {}", size, self.cur_entry - 1);
        }

        ret = io::read(&self.log_file, &mut buf)?;
        if ret != size as usize {
            bail!("Error reading data: {}", ret)
        }

        let offset = entry.sector * self.sector_size;
        ret = io::pwrite(&self.replay_file, buf.as_slice(), offset as i64)?;
        drop(buf);
        if ret != size as usize {
            bail!("Error reading data: {}", ret)
        }
        Ok(Some(entry))
    }
}

#[cfg(test)]
mod tests {
    use crate::log_writes::{LogWriteSuper, log_flags_table};
    use std::fs::OpenOptions;
    use std::io::Read;

    #[test]
    fn test_rust_struct_size() {}
}

