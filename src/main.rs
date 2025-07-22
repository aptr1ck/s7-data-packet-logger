//#![windows_subsystem = "windows"]

mod app_config;
mod comms;
mod constants;
mod sql;
mod event_data;
mod filehandling;
mod registryhandling;
mod utils;
mod ui_floem;
mod xmlhandling;

use floem::{
    action::{exec_after, inspect},
    keyboard::{Key, Modifiers, NamedKey},
    prelude::*,
};
use std::sync::{Arc, mpsc};
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
    let shutdown_notify = Arc::new(Notify::new());
    //let shutdown_notify_ui = shutdown_notify.clone();

    // Channel for UI to server notifications
    let (tx, rx) = mpsc::channel::<ServerStatusInfo>();
    //let (ui_ready_tx, ui_ready_rx) = mpsc::channel::<()>();

    // Start the servers
    unsafe {
        for i in 0..SERVER_CONFIG.server.len() {
            log(&format!("Starting server {}: {}:{}",
                     i,
                     SERVER_CONFIG.server[i].ip_address,
                     SERVER_CONFIG.server[i].port));
            let shutdown_notify = shutdown_notify.clone();
            let tx = tx.clone();
            let i = i;
            tokio::spawn(async move {
                let _ = run_server(shutdown_notify, i, tx).await;
            });
        }
    }
    
    // Floem UI has to run on the main thread
    app_config::launch_with_track(|| app_view(rx));
    // After UI exits, notify servers to shut down
    shutdown_notify.notify_waiters();
}
