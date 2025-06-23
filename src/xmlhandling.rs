use serde::Deserialize;
use quick_xml::de::from_str;
use std::fs;

#[derive(Debug, Deserialize)]
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