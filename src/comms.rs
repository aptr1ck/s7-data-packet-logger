use std::sync::{Arc};
use chrono::Local;
use serde::{Serialize, Deserialize};
use tokio::sync::Notify;
use tokio::time::{Duration};
use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::constants::DEBUG;
use crate::sql::*;
use crate::event_data::*;
use crate::utils::*;
use crate::xmlhandling::load_config;
use once_cell::sync::Lazy;

#[derive(Clone, Debug)]
pub struct ServerStatus {
    pub server: Vec<ServerStatusInfo>,
}
impl ServerStatus {
    pub fn new() -> Self {
        ServerStatus {
            server: vec![ServerStatusInfo {
                idx: 0,
                new_data: false,
                is_connected: false,
                is_alive: false,
                last_packet_time: 0,
            }],
        }
    }
}
pub static mut SERVER_STATUS: Lazy<ServerStatus> = Lazy::new(|| {
    ServerStatus{
        server: vec![ServerStatusInfo {
            idx: 0,
            new_data: false,
            is_connected: false,
            is_alive: false,
            last_packet_time: Local::now().timestamp_millis() as u64,
        }]
    }
});

#[derive(Clone, Copy, Debug)]
pub struct ServerStatusInfo {
    pub idx: usize,
    pub new_data: bool,
    pub is_connected: bool,
    pub is_alive: bool,
    pub last_packet_time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(rename = "Server")]
    pub server: Vec<ServerEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerEntry {
    pub name: String,
    pub ip_address: String,
    pub port: u16,
}

pub static mut SERVER_CONFIG: Lazy<ServerConfig> = Lazy::new(|| {
    load_config("config.xml").unwrap_or_else(|_| ServerConfig {
        server: vec![
            ServerEntry {
                name: "Default Server".to_string(),
                ip_address: "0.0.0.0".to_string(),
                port: 102,
            }
        ]
    })
});

pub async fn run_server(
    shutdown_notify: Arc<Notify>, 
    server_number: usize,
    //ip_address: String, 
    //port: String,
    tx: std::sync::mpsc::Sender<ServerStatusInfo>,
) -> std::io::Result<()> {
        let mut prev_status_len = unsafe { SERVER_STATUS.server.len() };
        log(&format!("Initial server status length: {}", prev_status_len));
    // Get the server status info for this server number, or create a new one if it doesn't exist.
    let mut server_status = unsafe { SERVER_STATUS.server.get(server_number)
        .cloned()
        .unwrap_or(ServerStatusInfo {
            idx: server_number,
            new_data: false,
            is_connected: false,
            is_alive: false,
            last_packet_time: 0 as u64,
        }) };
    // Create a new one if it doesn't exist
    unsafe {
        if server_number >= SERVER_STATUS.server.len() {
            SERVER_STATUS.server.push(server_status.clone());
        }
    }
    log(&format!("server_status: {:?}", unsafe{&SERVER_STATUS}));
    let config = match unsafe { SERVER_CONFIG.server.get(server_number) } {
        Some(cfg) => cfg,
        None => {
            log(&format!("ERROR: No server config for index {}", server_number));
            return Ok(());
        }
    };
    let ip_address = config.ip_address.to_string();
    let port = config.port.to_string();
    let address = format!("{}:{}", ip_address, port);
    log(&format!("Attempting to bind to {}", address)); 
    let listener = loop {
        let current_status_len = unsafe { SERVER_STATUS.server.len() };
        log(&format!("Current server status length: {}", current_status_len));
        match TcpListener::bind(&address).await {
            Ok(l) => break l, // break with the listener
            Err(e) => {
                log(&format!("Failed to bind to {}: {}. Retrying in 10 seconds...", address, e));
                server_status.new_data = true; // Notify UI about the retry
                tokio::time::sleep(Duration::from_secs(10)).await;
                // TODO: Receive shutdown signal here to break the loop if needed?
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
                                                server_status.last_packet_time = Local::now().timestamp_millis() as u64;
                                                // Notify the UI thread about new data
                                                let _ = tx.send(server_status.clone());
                                                // Deserialize the event data packet
                                                if let Some(packet) = parse_event_data_packet(&buffer[..size]) {
                                                    log(&format!("Parsed packet: data_type={}, plc_packet_code={}, data={:?}",
                                                                packet.data_type, packet.plc_packet_code, packet.data));
                                                    // Check for system packet that we should not store.
                                                    if !is_system_packet(&packet) {
                                                        // Put the data into the database
                                                        let result = store_packet(&conn, &packet); // TODO: Handle the response properly.
                                                        if result.is_err() {
                                                            if DEBUG { log(&format!("Error storing packet in database: {:?}", result)); }
                                                            // TODO: Close the connection when we have SQL INSERT errors.
                                                        }
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

                                                // UPDATE: Update global state
                                                unsafe {
                                                    if server_number < SERVER_STATUS.server.len() {
                                                        SERVER_STATUS.server[server_number] = server_status;
                                                    }
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