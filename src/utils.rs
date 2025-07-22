use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::constants::DEBUG;

pub fn log(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    // Log to console
    if DEBUG { println!("[{}] {}", timestamp, message); }
    // Append to log file
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("log.txt")
    {
        let _ = file.write_all(format!("[{}] {}\n", timestamp, message).as_bytes());
    } else {
        eprintln!("Failed to open log file.");
    }
}

pub fn widestring(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
