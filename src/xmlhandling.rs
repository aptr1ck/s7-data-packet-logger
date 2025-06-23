use serde::{Serialize, Deserialize,};
use quick_xml::de::{from_str,};
use quick_xml::se::{to_string,};
use std::fs;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ip_address: String,
    pub port: u16,
}

pub fn load_config(path: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    println!("Loading configuration from: {}", path);
    let xml = fs::read_to_string(path)?;
    let config: ServerConfig = from_str(&xml)?;
    Ok(config)
}

pub fn save_config(path: &str, config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let xml = to_string(config)?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(xml.as_bytes())?;
    Ok(())
}