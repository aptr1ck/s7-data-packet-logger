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

    // Create server manager using the proper constructor
    let (mut server_manager, command_tx) = ServerManager::new(status_tx, shutdown_notify.clone());
    
    // Create the command channel for server commands
    //let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel::<ServerCommand>();

    // Create server manager with the command receiver
    /*let mut server_manager = ServerManager {
        handles: HashMap::new(),
        shutdown_notify: shutdown_notify.clone(),
        tx: status_tx,
        command_rx,
        server_manager,
    };*/
        
    // Spawn background task to handle server commands
    let manager_handle = tokio::spawn(async move {
        server_manager.process_commands().await;
    });

    // Start autostart servers if needed
    unsafe {
        for i in 0..SERVER_CONFIG.server.len() {
            if SERVER_CONFIG.server[i].autostart {
                log(&format!("Auto-starting server {}: {}:{}",
                         i,
                         SERVER_CONFIG.server[i].ip_address,
                         SERVER_CONFIG.server[i].port));
                // Send start command through the channel
                let _ = command_tx.send(ServerCommand::Start(i));
            }
        }
    }
    
    // Floem UI has to run on the main thread
    app_config::launch_with_track(|| app_view(status_rx, command_tx));

    // After UI exits, notify servers to shut down
    shutdown_notify.notify_waiters();

    // Wait for the manager to finish
    //let _ = manager_handle.await;
}
