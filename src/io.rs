use std::fs::File;
use anyhow::{Result, anyhow};
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(target_os = "linux")]
pub fn read(file : &File, buf : &mut [u8]) -> Result<usize>{
    nix::unistd::read(file.as_raw_fd(), buf).map_err(|e| {
        anyhow!("IO error {}", e)
    })
}
#[cfg(target_os = "linux")]
pub fn read_at(file : &File, buf : &mut [u8], offset : i64) -> Result<usize>{
    nix::sys::uio::pread(file.as_raw_fd(), buf,offset).map_err(|e| {
        anyhow!("IO error {}", e)
    })

}

#[cfg(target_os = "linux")]
pub fn pwrite(file : &File, buf : &[u8], offset : i64) -> Result<usize>{
    nix::sys::uio::pwrite(file.as_raw_fd(), buf,offset).map_err(|e| {
        anyhow!("IO error {}", e)
    })

}