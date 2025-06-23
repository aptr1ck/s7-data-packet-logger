#[windows_subsystem = "windows"]
mod sql;
mod event_data;
mod filehandling;
mod registryhandling;
mod utils;
mod ui;
mod xmlhandling;

use std::sync::{Arc, mpsc};
use tokio::sync::Notify;
use std::thread;
//use std::io::{Read, Write};
//use tokio::time::{timeout, Duration};
use tokio::net::{TcpListener};//, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
//use rusqlite::{params, Connection, Result};
//use serde_json;
use crate::sql::*;
use crate::event_data::*;
//use crate::registryhandling::*;
use crate::utils::*;
use crate::ui::*;
use crate::xmlhandling::*;

async fn run_server(
    shutdown_notify: Arc<Notify>, 
    ip_address: String, 
    port: String,
    tx: std::sync::mpsc::Sender<()>,
) -> std::io::Result<()> {
    let address = format!("{}:{}", ip_address, port); // TODO: Make this configurable
    let listener = TcpListener::bind(address).await?;
    if DEBUG { log(&format!("Server listening on {}:{}", ip_address, port)); }
  
    loop {
        let tx = tx.clone(); // Clone the sender for each connection
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut socket, addr)) => {
                        //let (mut socket, addr) = listener.accept().await?;
                        log(&format!("New connection from {}", addr));
                        // Initialize SQLite connection -- each connection needs its own database connection.
                        let conn = connect_to_db().expect("Failed to connect to database");
                        let shutdown_notify = shutdown_notify.clone(); // Clone for each task
                        tokio::spawn(async move {
                            let mut buffer = [0u8; 512];
                            loop {
                                tokio::select! {
                                    read_result = socket.read(&mut buffer) => {
                                        match read_result {
                                            Ok(0) => {
                                                log("Connection closed by client.");
                                                break;
                                            }
                                            Ok(size) => {
                                                log(&format!("Received {} bytes: {:?}", size, &buffer[..size]));
                                                // Notify the UI thread about new data
                                                let _ = tx.send(());
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
                                    _ = shutdown_notify.notified() => {
                                        if DEBUG { log("Shutdown signal received, closing connection."); }
                                        break;
                                    }
                                }
                            }
                            if DEBUG { log("Ending connection handler."); }
                        });
                    }
                    Err(e) => {
                        if DEBUG { log(&format!("Failed to accept connection: {}", e)); }
                        break;
                    }
                }
            }
            _ = shutdown_notify.notified() => {
                if DEBUG { log("Shutdown signal received, stopping server."); }
                break;
            }
        }
    }
    Ok(())  
}

#[tokio::main]
async fn main() {//-> std::io::Result<()> {
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_ui = shutdown_notify.clone();

    // Channel for server-to-UI notifications
    let (tx, rx) = mpsc::channel();

    // Spawn the WinAPI window in a separate thread
    thread::spawn(|| {
        main_window(Some(shutdown_notify_ui), rx); // Your function here
    });

    // Load configuration from XML file
    let config = load_config("config.xml")
        .expect("Failed to load configuration");
    let ip_address = config.ip_address;
    let port = config.port.to_string();
    let _ = run_server(shutdown_notify, ip_address, port, tx).await;
}
