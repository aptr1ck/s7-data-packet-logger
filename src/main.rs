#[windows_subsystem = "windows"]

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
use crate::xmlhandling::*;

#[tokio::main]
async fn main() {//-> std::io::Result<()> {
    let shutdown_notify = Arc::new(Notify::new());
    let shutdown_notify_ui = shutdown_notify.clone();

    // Channel for UI to server notifications
    let (tx, rx) = mpsc::channel::<ServerStatus>();

    // Spawn the WinAPI window in a separate thread
    thread::spawn(|| {
        main_window(Some(shutdown_notify_ui), rx);
    });

    // Load configuration from XML file
    let config = load_config("config.xml")
        .expect("Failed to load configuration");
    let ip_address = config.ip_address;
    let port = config.port.to_string();
    let _ = run_server(shutdown_notify, ip_address, port, tx).await;
}
