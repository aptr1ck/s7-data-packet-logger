
// Event Data Packet Definition
pub struct EventDataPacket {
    pub raw: Vec<u8>, // Raw bytes of the packet
    pub event_code: u32, // 4 bytes for event type = 1 PLC DINT
    pub plc_packet_code: u32, // 4 bytes for PLC packet type = 1 PLC DINT
    pub data: Vec<u32>,  // Variable length data
}

pub fn system_packet(packet: &EventDataPacket) -> bool {
    // Check if the packet is a system packet based on event_code
    packet.event_code == 12
}

pub fn parse_event_data_packet(bytes: &[u8]) -> Option<EventDataPacket> {
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