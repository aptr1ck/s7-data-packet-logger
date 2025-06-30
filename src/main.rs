#![windows_subsystem = "windows"]

mod comms;
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
use crate::comms::*;
use crate::ui::*;
use crate::utils::log;

#[tokio::main]
async fn main() {//-> std::io::Result<()> {
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_ui = shutdown_notify.clone();

    // Channel for UI to server notifications
    let (tx, rx) = mpsc::channel::<ServerStatusInfo>();
    let (ui_ready_tx, ui_ready_rx) = mpsc::channel::<()>();

    // Spawn the WinAPI window in a separate thread
    thread::spawn(|| {
        main_window(Some(shutdown_notify_ui), rx, ui_ready_tx);
    });

    // Wait for the UI to signal it's ready
    ui_ready_rx.recv().expect("Failed to receive UI ready signal");

    // Now start the servers
    unsafe {
        for i in 0..SERVER_CONFIG.servers.len() {
            log(&format!("Starting server {}: {}:{}",
                     i,
                     SERVER_CONFIG.servers[i].ip_address,
                     SERVER_CONFIG.servers[i].port));
            let shutdown_notify = shutdown_notify.clone();
            let tx = tx.clone();
            let i = i;
            tokio::spawn(async move {
                let _ = run_server(shutdown_notify, i, tx).await;
            });
        }
    }
    
    // Wait for shutdown signal (block main thread)
    shutdown_notify.notified().await;
}
