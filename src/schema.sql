CREATE TABLE event_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    data_type INTEGER NOT NULL,
    plc_packet_code INTEGER NOT NULL,
    data BLOB NOT NULL
);