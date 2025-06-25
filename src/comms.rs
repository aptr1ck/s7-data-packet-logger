use std::sync::{Arc, mpsc};
use serde::{Serialize, Deserialize};
use tokio::sync::Notify;
use tokio::time::{timeout, Duration};
use tokio::net::{TcpListener};//, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::sql::*;
use crate::event_data::*;
use crate::utils::*;

#[derive(Clone, Copy)]
pub struct ServerStatus {
    pub new_data: bool,
    pub is_connected: bool,
    pub is_alive: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ip_address: String,
    pub port: u16,
}

pub async fn run_server(
    shutdown_notify: Arc<Notify>, 
    ip_address: String, 
    port: String,
    tx: std::sync::mpsc::Sender<ServerStatus>,
) -> std::io::Result<()> {
    let mut server_status = ServerStatus {
        new_data: false,
        is_connected: false,
        is_alive: false,
    };
    let address = format!("{}:{}", ip_address, port);
    let listener = loop {
        match TcpListener::bind(&address).await {
            Ok(l) => break l, // break with the listener
            Err(e) => {
                log(&format!("Failed to bind to {}: {}. Retrying in 10 seconds...", address, e));
                server_status.new_data = true; // Notify UI about the retry
                tokio::time::sleep(Duration::from_secs(10)).await;
                // TODO: Receive shutdown signal here to break the loop if needed
                // TODO: Updating the log with new data doesn't work.
            }
        }
    };
    // We don't get here until the listener is successfully connected.
    if DEBUG { log(&format!("Server listening on {}:{}", ip_address, port)); }
  
    loop {
        let mut server_status = server_status; // Clone the server status for each connection
        let tx = tx.clone(); // Clone the sender for each connection
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut socket, addr)) => {
                        //let (mut socket, addr) = listener.accept().await?;
                        log(&format!("New connection from {}", addr));
                        server_status.is_connected = true;
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
                                                server_status.is_connected = false;
                                                break;
                                            }
                                            Ok(size) => {
                                                log(&format!("Received {} bytes: {:?}", size, &buffer[..size]));
                                                server_status.is_alive = true;
                                                server_status.new_data = true;
                                                // Notify the UI thread about new data
                                                let _ = tx.send(server_status.clone());
                                                // Deserialize the event data packet
                                                if let Some(packet) = parse_event_data_packet(&buffer[..size]) {
                                                    log(&format!("Parsed packet: event_code={}, plc_packet_code={}, data={:?}",
                                                                packet.data_type, packet.plc_packet_code, packet.data));
                                                    // Check for system packet that we should not store.
                                                    if !is_system_packet(&packet) {
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
                                    _ = tokio::time::sleep(Duration::from_secs(30)) => {
                                        if DEBUG { log("No data recevied for 30 seconds.") };
                                        server_status.is_alive = false;
                                        // Notify the UI thread about the status change
                                        server_status.new_data = true;
                                        let _ = tx.send(server_status.clone());
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