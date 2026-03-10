use rust7::client::{S7Client};

pub fn connect() -> S7Client {
    let mut client = S7Client::new();
    let db_number: u16 = 100; // Must exist into the PLC

    // Connection
    match client.connect_s71200_1500("192.168.0.100") {
        Ok(_) => { println!("Connected to PLC") },
        Err(e) => {
            eprintln!("Connection failed: {}", e);
            return;
        }
    }
    client
}

pub fn disconnect(client: &mut S7Client) {
    client.disconnect();
    println!("Disconnected from PLC");
}

pub fn read_data(client: &mut S7Client, db_number: u16, start_address: u16, size: usize) {
    match client.read_area(rust7::areas::DataBlock(db_number), start_address, size) {
        Ok(data) => {
            println!("Data read from PLC: {:?}", data);
        },
        Err(e) => {
            eprintln!("Read failed: {}", e);
        }
    }
}

pub fn write_data(client: &mut S7Client, db_number: u16, start_address: u16, data: &[u8]) {
    match client.write_area(rust7::areas::DataBlock(db_number), start_address, data) {
        Ok(_) => {
            println!("Data written to PLC successfully");
        },
        Err(e) => {
            eprintln!("Write failed: {}", e);
        }
    }
}