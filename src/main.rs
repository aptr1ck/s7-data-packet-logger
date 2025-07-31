#![windows_subsystem = "windows"]

mod app_config;
mod comms;
mod constants;
mod sql;
mod event_data;
mod filehandling;
mod registryhandling;
mod theme;
mod utils;
mod ui_floem;
mod xmlhandling;

use floem::{
    action::{exec_after, inspect},
    keyboard::{Key, Modifiers, NamedKey},
    prelude::*,
};
use std::sync::{Arc, mpsc, Mutex};
use std::collections::HashMap;
use tokio::sync::Notify;
use crate::comms::*;
use crate::ui_floem::*;
use crate::utils::log;

pub const OS_MOD: Modifiers = if cfg!(target_os = "macos") {
    Modifiers::META
} else {
    Modifiers::CONTROL
};

#[tokio::main]
async fn main() {
    let (status_tx, status_rx) = mpsc::channel::<ServerStatusInfo>();
    let shutdown_notify = Arc::new(Notify::new());
    // Clone for server autostart handling
    let status_tx_clone = status_tx.clone();
    // Create server manager using the proper constructor
    let (mut server_manager, command_tx) = ServerManager::new(status_tx, shutdown_notify.clone());
        
    // Spawn background task to handle server commands
    let _manager_handle = tokio::spawn(async move {
        server_manager.process_commands().await;
    });

    // Start autostart servers if needed
    unsafe {
        for (idx, server) in SERVER_CONFIG.server.iter_mut().enumerate() {
            println!("Server ID: {:?}", server.id);
            // Start server if autostart is enabled
            if server.autostart {
                log(&format!("Auto-starting server {}: {}:{}",
                         idx,
                         server.ip_address,
                         server.port));
                // Send start command through the channel
                let _ = command_tx.send(ServerCommand::Start(idx));
            }
            // Send initial status for this server
            let initial_status = ServerStatusInfo {
                idx,
                server_id: server.id.clone(),
                is_running: false,
                is_connected: false,
                is_alive: false,
                peer_ip: [0; 16],
                last_packet_time: 0,
                new_data: false,
            };
            println!("Sending initial status for server {}: {:?}", idx, initial_status);
            let _ = status_tx_clone.send(initial_status);
        }
    }
    
    // Floem UI has to run on the main thread
    app_config::launch_with_track(|| app_view(status_rx, command_tx));

    // After UI exits, notify servers to shut down
    shutdown_notify.notify_waiters();

    // Wait for the manager to finish
    //let _ = manager_handle.await;
}
