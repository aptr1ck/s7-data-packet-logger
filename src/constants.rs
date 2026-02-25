pub const DEBUG: bool = false; // Set to true to enable debug logging
pub const APPNAME: &str = "S7 Event Monitor"; 
pub const APPVERSION: &str = "0.0.1";
pub const APPAUTHOR: &str = "Patrick McDermott";
pub const APPEMAIL: &str = "mcd@omg.lol";
pub const MENU_HEIGHT: f32 = 35.0;
pub const FONT_SIZE_MENU: f32 = 13.0;
pub const FONT_SIZE_WIN_CONTROLS: f32 = 10.0;
pub const RESIZE_HANDLE_SIZE: f64 = 5.0;

pub const EVENT_TYPE_SPECIAL: u32 = 1;
pub const EVENT_TYPE_KEEPALIVE: u32 = 12;
pub const EVENT_TYPE_PLC: u32 = 50;