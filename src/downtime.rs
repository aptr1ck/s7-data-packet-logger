use crate::constants::EVENT_TYPE_SPECIAL;
use crate::sql::{connect_to_db, query_packets};
use crate::event_data::SqlDataPacket;
use chrono::{Local, DateTime};

#[derive(Clone)]
pub struct DowntimeRecord {
    pub start: String,
    pub end: String,
    pub duration: i64,
}

// Pairs downtime start (41) and end (42) events and calculates duration
pub fn process_downtime_packets(packets: Vec<SqlDataPacket>) -> Vec<DowntimeRecord> {
    let mut downtime_records = Vec::new();
    let mut i = 0;
    
    while i < packets.len() {
        // Look for a start event (41)
        if packets[i].packet.plc_packet_code == 41 {
            let start_time = packets[i].timestamp.clone();
            
            // Look for the next end event (42)
            let mut end_time = None;
            for j in (i + 1)..packets.len() {
                if packets[j].packet.plc_packet_code == 42 {
                    end_time = Some(packets[j].timestamp.clone());
                    i = j; // Move to the end event
                    break;
                }
            }
            
            // If we found a matching end, calculate duration
            if let Some(end) = end_time {
                let duration = calculate_duration(&start_time, &end);
                downtime_records.push(DowntimeRecord {
                    start: start_time,
                    end,
                    duration,
                });
            }
        }
        i += 1;
    }
    
    downtime_records
}

/// Calculates the duration between two RFC3339 timestamps (returns seconds)
fn calculate_duration(start: &str, end: &str) -> i64 {
    match (
        DateTime::parse_from_rfc3339(start),
        DateTime::parse_from_rfc3339(end),
    ) {
        (Ok(start_dt), Ok(end_dt)) => {
            let duration = end_dt.signed_duration_since(start_dt);
            duration.num_seconds() + 60 // There is always 1 minute of downtime before we capture it.
        }
        _ => 0,
    }
}

pub fn downtime_retreive() -> (String, Result<Vec<SqlDataPacket>, rusqlite::Error>) {
    let now = Local::now();
    let date = now.format("%Y-%m-%d");
    
    let conn = connect_to_db().expect("Failed to connect to database");
    // 41 = downtime start
    // 42 = downtime end
    let sql_result = query_packets(&conn, &date.to_string(), &EVENT_TYPE_SPECIAL.to_string(), "41,42");
    let sql_query_str = if let Ok(packets) = &sql_result {
                packets.first()
                    .map(|p| format!("Query: {}", p.query))
                    .unwrap_or_else(|| String::from("Query: No results found"))
            } else {
                String::from("Query: Failed to retrieve query")
            };
    
    (sql_query_str, sql_result)
}



pub fn format_seconds_to_duration(mut seconds: i64) -> String {
    let hours = seconds / 3600;
    seconds %= 3600;
    let minutes = seconds / 60;
    let secs = seconds % 60;
    format!("{}h {}m {}s", hours, minutes, secs)
}