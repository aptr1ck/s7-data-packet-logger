use rusqlite::{params, Connection, Result};
use serde_json;
use std::path::Path;
use crate::constants::DEBUG;
use crate::event_data::EventDataPacket;
use crate::utils::*;

pub fn connect_to_db() -> Result<Connection> {
    let db_path = "event_data.db";
    let is_new_db = !Path::new(db_path).exists();

    let conn = Connection::open(db_path).expect("Failed to open database");

    if is_new_db {
        conn.execute_batch(include_str!("schema.sql")).expect("Failed to create schema");
        if DEBUG { log("Database created with schema."); }
    } else {
        if DEBUG { log("Connected to existing database."); }
    }
    Ok(conn)
}

pub fn store_packet(conn: &Connection, packet: &EventDataPacket) -> rusqlite::Result<()> {
    let timestamp = chrono::Local::now().to_rfc3339();
    let data_json = serde_json::to_string(&packet.data).unwrap();

    conn.execute(
        "INSERT INTO event_data (timestamp, data_type, plc_packet_code, data) VALUES (?1, ?2, ?3, ?4)",
        params![timestamp, packet.data_type, packet.plc_packet_code, data_json],
    )?;
    Ok(())
}
