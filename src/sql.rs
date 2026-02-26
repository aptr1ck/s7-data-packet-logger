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

pub fn query_packets(conn: &Connection, start_date: &str, end_date: &str, data_type: &str, plc_packet_code: &str) -> rusqlite::Result<Vec<SqlDataPacket>> {
    // build a safe IN(...) clause from comma‑separated PLC codes
    let codes_vec: Vec<u32> = plc_packet_code
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    let in_clause = if codes_vec.is_empty() {
        // no valid values -> make the expression always false
        String::from("IN (NULL)")
    } else {
        let codes_join = codes_vec.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        format!("IN({})", codes_join)
    };

    // We treat the input dates as **dates** (YYYY-MM-DD).  The timestamp
    // column stores a full RFC3339 string, so equality on that string will
    // almost never match when only a date is supplied.  Converting the
    // column to a date with `date(timestamp)` makes single‑day queries behave
    // properly.  An empty `end_date` means "no upper bound".

    if end_date.is_empty() {
        // open‑ended query (no upper limit)
        let sql = format!(
            "SELECT plc, timestamp, data_type, plc_packet_code, data \
             FROM event_data \
             WHERE date(timestamp, 'localtime') >= date(?1) \
               AND data_type = ?2 \
               AND plc_packet_code {}",
            in_clause
        );
        println!("Executing SQL Query: {}", sql);

        let mut stmt = conn.prepare(&sql)?;
        let packet_iter = stmt.query_map(params![start_date, data_type], move |row| {
            let plc: String = row.get(0)?;
            let timestamp: String = row.get(1)?;
            let data_type: u32 = row.get(2)?;
            let plc_packet_code: u32 = row.get(3)?;
            let data_json: String = row.get(4)?;
            let query = format!(
                "SELECT plc, timestamp, data_type, plc_packet_code, data FROM event_data \
                 WHERE date(timestamp, 'localtime') >= date('{}') AND data_type = {} AND plc_packet_code {}",
                start_date, data_type, in_clause
            );
            let data_vec: Vec<u32> = serde_json::from_str(&data_json).unwrap_or_default();
            println!("Results: {}", data_vec.len());

            Ok(SqlDataPacket {
                query,
                plc,
                timestamp,
                packet: EventDataPacket {
                    raw: vec![],
                    data_type,
                    plc_packet_code,
                    data: data_vec,
                },
            })
        })?;

        packet_iter.collect()
    } else {
        // bounded range query
        let sql = format!(
            "SELECT plc, timestamp, data_type, plc_packet_code, data \
             FROM event_data \
             WHERE date(timestamp, 'localtime') >= date(?1) \
               AND date(timestamp, 'localtime') <= date(?2) \
               AND data_type = ?3 \
               AND plc_packet_code {}",
            in_clause
        );
        println!("Executing SQL Query: {}", sql);

        let mut stmt = conn.prepare(&sql)?;
        let packet_iter = stmt.query_map(
            params![start_date, end_date, data_type],
            move |row| {
                let plc: String = row.get(0)?;
                let timestamp: String = row.get(1)?;
                let data_type: u32 = row.get(2)?;
                let plc_packet_code: u32 = row.get(3)?;
                let data_json: String = row.get(4)?;
                let query = format!(
                    "SELECT plc, timestamp, data_type, plc_packet_code, data FROM event_data \
                     WHERE date(timestamp, 'localtime') >= date('{}') AND date(timestamp, 'localtime') <= date('{}') \
                           AND data_type = {} AND plc_packet_code {}",
                    start_date, end_date, data_type, in_clause
                );
                let data_vec: Vec<u32> = serde_json::from_str(&data_json).unwrap_or_default();
                println!("Results: {}", data_vec.len());

                Ok(SqlDataPacket {
                    query,
                    plc,
                    timestamp,
                    packet: EventDataPacket {
                        raw: vec![],
                        data_type,
                        plc_packet_code,
                        data: data_vec,
                    },
                })
            },
        )?;

        packet_iter.collect()
    }
}