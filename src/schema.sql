CREATE TABLE event_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    event_code INTEGER NOT NULL,
    plc_packet_code INTEGER NOT NULL,
    data BLOB NOT NULL
);