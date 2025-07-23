use tokio::task::JoinHandle;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use chrono::Local;
use serde::{Serialize, Deserialize};
use tokio::sync::{/*mpsc,*/ Notify};
use tokio::time::{Duration};
use tokio::net::{TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::constants::DEBUG;
use crate::sql::*;
use crate::event_data::*;
use crate::utils::*;
use crate::xmlhandling::load_config;
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub enum ServerCommand {
    Start(usize),
    Stop(usize),
    StopAll,
}

#[derive(Clone, Copy, Debug)]
pub struct ServerStatusInfo {
    pub idx: usize,
    pub new_data: bool,
    pub is_running: bool,
    pub is_connected: bool,
    pub is_alive: bool,
    pub last_packet_time: u64,
    pub peer_ip: [u8; 16], // Can hold IPv4 or IPv6, convert to/from string as needed
}

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
                is_running: false,
                is_connected: false,
                is_alive: false,
                last_packet_time: 0,
                peer_ip: [0; 16],
            }],
        }
    }

    pub fn set_ipv4(&mut self, i: usize, ip: std::net::Ipv4Addr) {
        // Store IPv4 in the first 4 bytes, rest zeros
        let octets = ip.octets();
        self.server[i].peer_ip = [0; 16];
        self.server[i].peer_ip[0..4].copy_from_slice(&octets);
        log(&format!("Set IPv4 for server {}: {:?}", i, self.server[i].peer_ip));
    }
    
    pub fn set_ipv6(&mut self, i: usize, ip: std::net::Ipv6Addr) {
        self.server[i].peer_ip = ip.octets();
    }

    pub fn get_ip_string(&self, i: usize) -> String {
        // Check if it's IPv4 (first 4 bytes non-zero, rest zero)
        log(&format!("Getting IP string for server {}: {:?}", i, self.server[i].peer_ip));
        if self.server[i].peer_ip[4..].iter().all(|&x| x == 0) {
            let ipv4 = std::net::Ipv4Addr::new(
                self.server[i].peer_ip[0], 
                self.server[i].peer_ip[1], 
                self.server[i].peer_ip[2], 
                self.server[i].peer_ip[3]
            );
            if ipv4.is_unspecified() {
                log(&format!("Server {} has unspecified IPv4 address, returning x.x.x.x", i));
                "x.x.x.x".to_string()
            } else {
                log(&format!("Server {} IPv4 address: {}", i, ipv4));
                ipv4.to_string()
            }
        } else {
            // IPv6
            let ipv6 = std::net::Ipv6Addr::from(self.server[i].peer_ip);
            ipv6.to_string()
        }
    }

    pub fn set_ip_from_string(&mut self, i: usize, ip_str: &str) -> Result<(), std::net::AddrParseError> {
        log(&format!("Setting IP for server {}: {}", i, ip_str));
        if let Ok(ipv4) = ip_str.parse::<std::net::Ipv4Addr>() {
            self.set_ipv4(i,ipv4);
            Ok(())
        } else if let Ok(ipv6) = ip_str.parse::<std::net::Ipv6Addr>() {
            self.set_ipv6(i, ipv6);
            Ok(())
        } else {
            Err("Invalid IP address".parse::<std::net::Ipv4Addr>().unwrap_err())
        }
    }
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
    pub autostart: bool,
}

pub static mut SERVER_CONFIG: Lazy<ServerConfig> = Lazy::new(|| {
    load_config("config.xml").unwrap_or_else(|_| ServerConfig {
        server: vec![
            ServerEntry {
                name: "Default Server".to_string(),
                ip_address: "0.0.0.0".to_string(),
                port: 2000,
                autostart: false,
            }
        ]
    })
});

//#[derive(Copy, Debug)]
pub struct ServerManager {
    pub handles: HashMap<usize, JoinHandle<()>>,
    pub shutdown_notify: Arc<Notify>,
    pub tx: std::sync::mpsc::Sender<ServerStatusInfo>,
    pub command_rx: tokio::sync::mpsc::UnboundedReceiver<ServerCommand>,
    pub server_status: ServerStatus,
}

impl ServerManager {
    pub fn new(
        tx: std::sync::mpsc::Sender<ServerStatusInfo>, 
        shutdown_notify: Arc<Notify>
    ) -> (Self, tokio::sync::mpsc::UnboundedSender<ServerCommand>) {
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        
        // Initialize with proper number of servers from config
        let server_count = unsafe { SERVER_CONFIG.server.len() };
        let mut server_status = ServerStatus::new();
        server_status.server.clear(); // Remove the default entry
        
        // Create status entries for each configured server
        for i in 0..server_count {
            server_status.server.push(ServerStatusInfo {
                idx: i,
                new_data: false,
                is_running: false,
                is_connected: false,
                is_alive: false,
                last_packet_time: 0,
                peer_ip: [0; 16],
            });
        }

        let manager = Self {
            handles: HashMap::new(),
            shutdown_notify,
            tx,
            command_rx,
            server_status,
        };
        
        (manager, command_tx)
    }

    pub async fn process_commands(&mut self) {
        while let Some(command) = self.command_rx.recv().await {
            match command {
                ServerCommand::Start(idx) => {
                    self.start_server(idx).await;
                }
                ServerCommand::Stop(idx) => {
                    let _ = self.stop_server(idx).await;
                }
                ServerCommand::StopAll => {
                    self.stop_all_servers().await;
                }
            }
        }
    }

    pub async fn start_server(&mut self, server_index: usize) {
        if self.handles.contains_key(&server_index) {
            log(&format!("Server {} is already running", server_index));
            return;
        }

        let shutdown_notify = self.shutdown_notify.clone();
        let tx = self.tx.clone();
        
        // Get initial status for this server
        let initial_status = self.server_status.server.get(server_index)
            .cloned()
            .unwrap_or(ServerStatusInfo {
                idx: server_index,
                new_data: false,
                is_running: true,
                is_connected: false,
                is_alive: false,
                last_packet_time: 0,
                peer_ip: [0; 16],
            });

        let handle = tokio::spawn(async move {
            if let Err(e) = run_server(shutdown_notify, server_index, tx, initial_status).await {
                log(&format!("Server {} exited with error: {}", server_index, e));
            } else {
                log(&format!("Server {} exited normally", server_index));
            }
        });
        
        self.handles.insert(server_index, handle);
        self.server_status.server[server_index].is_running = true;

        // Send the updated status to the UI immediately
        let _ = self.tx.send(self.server_status.server[server_index]);
        
        if DEBUG { log(&format!("Started server {}", server_index)); }
    }

    pub async fn stop_server(&mut self, server_index: usize) -> Result<(), &'static str> {
        if let Some(handle) = self.handles.remove(&server_index) {
            // Notify shutdown
            self.shutdown_notify.notify_waiters();
            
            // Wait for graceful shutdown or force abort
            tokio::select! {
                result = handle => {
                    match result {
                        Ok(_) => log(&format!("Server {} stopped gracefully", server_index)),
                        Err(e) => log(&format!("Server {} join error: {}", server_index, e)),
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    log(&format!("Server {} did not stop gracefully, aborting", server_index));
                    // Handle is dropped here, which should abort the task
                }
            }
            
            // Update local status and notify UI
            if let Some(status) = self.server_status.server.get_mut(server_index) {
                status.is_running = false;
                status.is_connected = false;
                status.is_alive = false;
                status.new_data = true;
                let _ = self.tx.send(*status);
            }
            
            Ok(())
        } else {
            self.server_status.server[server_index].is_running = false;
            Err("Server not running")
        }
    }

    pub fn is_running(&self, server_index: usize) -> bool {
        self.handles.contains_key(&server_index)
    }

    pub async fn stop_all_servers(&mut self) {
        self.shutdown_notify.notify_waiters();
        
        let handles: Vec<_> = self.handles.drain().collect();
        
        for (idx, handle) in handles {
            tokio::select! {
                result = handle => {
                    match result {
                        Ok(_) => log(&format!("Server {} stopped", idx)),
                        Err(e) => log(&format!("Server {} error: {}", idx, e)),
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    log(&format!("Force stopping server {}", idx));
                }
            }
        }
    }
}
//
/*impl Clone for ServerManager {
    fn clone(&self) -> Self {
        Self {
            handles: HashMap::new(), // Start with empty handles for the clone
            shutdown_notify: self.shutdown_notify.clone(),
            tx: self.tx.clone(),
            command_rx: self.command_rx.clone(), // Clone the receiver
        }
    }
}*/

pub async fn run_server(
    shutdown_notify: Arc<Notify>, 
    server_number: usize,
    tx: std::sync::mpsc::Sender<ServerStatusInfo>,
    mut server_status: ServerStatusInfo,
) -> std::io::Result<()> {
    let config = match unsafe { SERVER_CONFIG.server.get(server_number) } {
        Some(cfg) => cfg,
        None => {
            log(&format!("ERROR: No server config for index {}", server_number));
            return Ok(());
        }
    };

    let mut ip_address = config.ip_address.to_string();
    let mut port = config.port.to_string();
    let mut address = format!("{}:{}", ip_address, port);

    log(&format!("Attempting to bind to {}", address)); 
    let listener = loop {
        match TcpListener::bind(&address).await {
            Ok(l) => break l, // break with the listener
            Err(e) => {
                log(&format!("Failed to bind to {}: {}. Retrying in 10 seconds...", address, e));
                server_status.new_data = true; // Notify UI about the retry
                tokio::time::sleep(Duration::from_secs(10)).await;
                // Reload config in case we changed it in the meantime
                ip_address = config.ip_address.to_string();
                port = config.port.to_string();
                address = format!("{}:{}", ip_address, port);
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
                        if DEBUG { log(&format!("New connection from {}", addr)); }
                        // Update server status with peer IP
                        let peer_ip_str = addr.ip().to_string();
                        if let Ok(ipv4) = peer_ip_str.parse::<std::net::Ipv4Addr>() {
                            let octets = ipv4.octets();
                            server_status.peer_ip = [0; 16];
                            server_status.peer_ip[0..4].copy_from_slice(&octets);
                        } else if let Ok(ipv6) = peer_ip_str.parse::<std::net::Ipv6Addr>() {
                            server_status.peer_ip = ipv6.octets();
                        }
                        server_status.is_connected = true;
                        server_status.new_data = true; // Notify UI about new connection
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
                                                server_status.is_running = true;
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