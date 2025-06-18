//use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use chrono::Local;
use tokio::time::{timeout, Duration};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEBUG: bool = true; // Set to true to enable debug logging

// Event Data Packet Definition
struct EventDataPacket {
    pub raw: Vec<u8>, // Raw bytes of the packet
    pub event_code: u32, // 4 bytes for event type = 1 PLC DINT
    pub plc_packet_code: u32, // 4 bytes for PLC packet type = 1 PLC DINT
    pub data: Vec<u32>,  // Variable length data
}

fn parse_event_data_packet(bytes: &[u8]) -> Option<EventDataPacket> {
    if bytes.len() < 8 || bytes.len() % 4 != 0 {
        return None; // Not enough data or misaligned
    }

    let raw = bytes.to_vec(); // Store the raw bytes
    let event_code = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
    let plc_packet_code = u32::from_be_bytes(bytes[4..8].try_into().ok()?);

    let mut data = Vec::new();
    for chunk in bytes[8..].chunks(4) {
        let value = u32::from_be_bytes(chunk.try_into().ok()?);
        data.push(value);
    }

    Some(EventDataPacket {
        raw,
        event_code,
        plc_packet_code,
        data,
    })
}

fn log(message: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{}] {}", timestamp, message);
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:10200").await?;
    log("Server listening on port 10200");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        log(&format!("New connection from {}", addr));

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

            log("Ending connection handler.");
        });
    }
}
