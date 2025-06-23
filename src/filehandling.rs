use std::fs;
use std::io::{self};

pub fn file_tail(path: &str, num_lines: usize) -> Result<String, io::Error> {
    if let Ok(content) = fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(num_lines);        
        let mut tail_lines: Vec<&str> = lines[start..].to_vec();
        tail_lines.reverse();
        Ok(tail_lines.join("\r\n"))
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "Failed to read file."))
    }
}
