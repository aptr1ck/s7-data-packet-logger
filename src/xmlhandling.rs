use std::fs::File;
use std::fs;
use std::io::Write;
use quick_xml::se::Serializer;
use serde::Serialize;
use crate::comms::ServerConfig;
use quick_xml::de::{from_str,};

pub fn load_config(path: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    println!("Loading configuration from: {}", path);
    let xml = fs::read_to_string(path)?;
    let ServerConfig: ServerConfig = from_str(&xml)?;
    Ok(ServerConfig)
}

pub fn save_config(path: &str, config: &ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = String::new();
    let mut serializer = Serializer::new(&mut buffer);
    serializer.indent(' ', 4); // 4 spaces for indentation

    config.serialize(serializer)?;

    let mut file = File::create(path)?;
    file.write_all(buffer.as_bytes())?;
    Ok(())
}