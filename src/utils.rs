use chrono::Local;

pub const DEBUG: bool = true; // Set to true to enable debug logging

pub fn log(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] {}", timestamp, message);
}