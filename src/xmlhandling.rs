use base64::{Engine as _, engine::general_purpose, engine::general_purpose::STANDARD};
use std::fs::File;
use std::fs;
use std::io::Write;
use quick_xml::se::Serializer;
use serde::{Serialize, Deserialize, Deserializer};
use crate::comms::{ServerConfig, SERVER_CONFIG, };
use quick_xml::de::{from_str,};

pub fn load_config(path: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    println!("Loading configuration from: {}", path);
    let xml = fs::read_to_string(path)?;
    let server_config: ServerConfig = from_str(&xml)?;
    Ok(server_config)
}

pub unsafe fn save_config(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = String::new();
    let mut serializer = Serializer::new(&mut buffer);
    serializer.indent(' ', 4); // 4 spaces for indentation

    SERVER_CONFIG.serialize(serializer)?;

    let mut file = File::create(path)?;
    file.write_all(buffer.as_bytes())?;
    println!("{:?}",buffer);
    Ok(())
}

pub fn serialize_bytes_as_base64<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let encoded = general_purpose::STANDARD.encode(bytes);
    serializer.serialize_str(&encoded)
}

pub fn deserialize_bytes_from_base64<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
where
    D: Deserializer<'de>,
{
    let s = <String as Deserialize>::deserialize(deserializer)?;
    let decoded = STANDARD.decode(s)
        .map_err(serde::de::Error::custom)?;
    
    if decoded.len() != 32 {
        return Err(serde::de::Error::custom("Invalid byte array length"));
    }
    
    let mut array = [0u8; 32];
    array.copy_from_slice(&decoded);
    Ok(array)
}