
use std::fs::OpenOptions;
use crate::log_writes::LogWriteSuper;


mod log_writes;
mod reader;
mod io;
mod util;

#[cfg(target_os = "linux")]
fn main() {

}
