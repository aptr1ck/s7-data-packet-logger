use rusqlite::{params, Connection, Result};
use serde_json;
use std::path::Path;
use crate::constants::DEBUG;
use crate::event_data::{EventDataPacket, SqlDataPacket};
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

pub fn store_packet(conn: &Connection, packet: &EventDataPacket, sender: &String) -> rusqlite::Result<()> {
    let timestamp = chrono::Local::now().to_rfc3339();
    let data_json = serde_json::to_string(&packet.data).unwrap();

    conn.execute(
        "INSERT INTO event_data (plc, timestamp, data_type, plc_packet_code, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sender, timestamp, packet.data_type, packet.plc_packet_code, data_json],
    )?;
    Ok(())
}

pub fn query_packets(conn: &Connection, date: &str, data_type: &str, plc_packet_code: &str) -> rusqlite::Result<Vec<SqlDataPacket>> {
    // Parse the comma-separated plc_packet_code into integers to build a safe IN(...) clause
    let codes_vec: Vec<u32> = plc_packet_code
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    let in_clause = if codes_vec.is_empty() {
        // No valid codes -> match nothing
        String::from("IN (NULL)")
    } else {
        let codes_join = codes_vec.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        format!("IN({})", codes_join)
    };

    let sql = format!(
        "SELECT plc, timestamp, data_type, plc_packet_code, data FROM event_data WHERE timestamp >= ?1 AND data_type = ?2 AND plc_packet_code {}",
        in_clause
    );
    println!("Executing SQL Query: {}", sql);

    // Keep readable copies for the row-mapper closure (avoid shadowing)
    let date_str = date.to_string();
    let data_type_param = data_type.to_string();
    let in_clause_clone = in_clause.clone();

    let mut stmt = conn.prepare(&sql)?;
    let packet_iter = stmt.query_map(params![date, data_type], move |row| {
        let plc: String = row.get(0)?;
        let timestamp: String = row.get(1)?;
        let data_type: u32 = row.get(2)?;
        let plc_packet_code: u32 = row.get(3)?;
        let data_json: String = row.get(4)?;
        let query = format!(
            "SELECT plc, timestamp, data_type, plc_packet_code, data FROM event_data WHERE timestamp >= '{}' AND data_type = {} AND plc_packet_code {}",
            date_str, data_type_param, in_clause_clone
        );
        let data_vec: Vec<u32> = serde_json::from_str(&data_json).unwrap_or_default();
        println!("Results: {}", data_vec.len());

        Ok(SqlDataPacket {
            query,
            plc,
            timestamp,
            packet: EventDataPacket {
                raw: vec![], // Raw bytes not stored in DB
                data_type,
                plc_packet_code,
                data: data_vec,
            },
        })
    })?;

    packet_iter.collect()
}