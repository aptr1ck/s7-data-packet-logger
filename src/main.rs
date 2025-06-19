mod sql;
mod event_data;
mod utils;

use std::io::{Read, Write};
use tokio::time::{timeout, Duration};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use rusqlite::{params, Connection, Result};
use serde_json;
use crate::sql::*;
use crate::event_data::*;
use crate::utils::*;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = "0.0.0.0:10200"; // TODO: Make this configurable
    let listener = TcpListener::bind(address).await?;
    if DEBUG { log(&format!("Server listening on {}", address)); }

    loop {
        let (mut socket, addr) = listener.accept().await?;
        log(&format!("New connection from {}", addr));

        // Initialize SQLite connection
        let conn = connect_to_db().expect("Failed to connect to database");

        tokio::spawn(async move {
            let mut buffer = [0u8; 512];

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        log("Connection closed by client.");
                        break;
                    }
                    Ok(size) => {
                        log(&format!("Received {} bytes: {:?}", size, &buffer[..size]));

                        // Deserialize the event data packet
                        if let Some(packet) = parse_event_data_packet(&buffer[..size]) {
                            log(&format!("Parsed packet: event_code={}, plc_packet_code={}, data={:?}",
                                         packet.event_code, packet.plc_packet_code, packet.data));
                            // Check for system packet that we should not store.
                            if !system_packet(&packet) {
                                // Put the data into the database
                                let _ = store_packet(&conn, &packet); // TODO: Handle the response properly.
                            }
                        } else {
                            log("Failed to parse event data packet.");
                        }

                        // Send ACK or echo back
                        if let Err(e) = socket.write_all(b"ACK").await {
                            if DEBUG { log(&format!("Failed to send ACK: {}", e)); }
                            break;
                        } else {
                            if DEBUG { log("ACK sent to client."); }
                        }
                    }
                    Err(e) => {
                        if DEBUG { log(&format!("Read error: {}", e)); }
                        break;
                    }
                }
            }

            if DEBUG { log("Ending connection handler."); }
        });
    }
}
