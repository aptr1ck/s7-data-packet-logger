use std::env;
use std::process::Command;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::{Instant, Duration};
use chrono::{DateTime, Local, Utc};
use tokio::sync::Notify;
use winapi::um::winuser::*;
use winapi::um::commctrl::{InitCommonControls, TCITEMW, TCM_INSERTITEMW, TCM_GETCURSEL, TCM_SETCURSEL,};
use winapi::um::libloaderapi::{GetModuleHandleW, LoadLibraryW};
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::wingdi::{GetTextExtentPoint32W, CreateFontW, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_DONTCARE};
use winapi::um::wingdi::{SelectObject, GetStockObject, SetTextColor, SetBkMode, TRANSPARENT, RGB, NULL_BRUSH,};
use winapi::shared::windef::{HBRUSH, HWND, SIZE, HGDIOBJ}; 
use winapi::shared::windef::HMENU;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM, LOWORD, HIWORD};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use crate::comms::{ServerStatus, ServerStatusInfo, ServerEntry, SERVER_CONFIG, SERVER_STATUS};
use crate::filehandling::*;
use crate::ui_floem::*;
use crate::registryhandling::*;   
use crate::utils::*;
use crate::xmlhandling::load_config;

// CONSTANTS
const ID_FILE_OPEN: u16 = 101;
const ID_FILE_SAVE: u16 = 102;
const ID_FILE_SAVE_AS: u16 = 103;
const ID_FILE_EXIT: u16 = 100;
const ID_FILE_NEW: u16 = 104;
const ID_HELP_TOC: u16 = 200;
const ID_HELP_START: u16 = 201;
const ID_HELP_ABOUT: u16 = 202;
const ID_EMAIL_LABEL: i32 = 5001;
const ID_SAVE_CONFIG: u16 = 6001; // ID for the Save button in the status view
const WM_NEW_DATA: u32 = WM_USER + 1;
pub const WM_UPDATE_STATUS_VIEW: u32 = WM_USER + 2;
const LOG_LINES: usize = 500; // Number of lines to show in the log viewer

// Global Mutables.
//static MAIN_HWND: Lazy<Mutex<HWND>> = Lazy::new(|| Mutex::new(std::ptr::null_mut()));
pub static mut TAB_HWND: HWND = null_mut();
static mut LOG_HWND: HWND = null_mut();
static mut STS_HWND: HWND = null_mut();
// Server Status
//static SERVER_STATUS: Lazy<Mutex<ServerStatus>> = Lazy::new(|| Mutex::new(ServerStatus::new()));
/*[ServerStatusInfo {
    new_data: false,
    is_connected: false,
    is_alive: false,
    last_packet_time: 0,
}];*/

// Keyboard Shortcuts
static ACCELERATORS: [ACCEL; 4] = [
    ACCEL { fVirt: FCONTROL | FVIRTKEY, key: 'N' as u16, cmd: ID_FILE_NEW as u16 },
    ACCEL { fVirt: FCONTROL | FVIRTKEY, key: 'O' as u16, cmd: ID_FILE_OPEN as u16 },
    ACCEL { fVirt: FCONTROL | FVIRTKEY, key: 'S' as u16, cmd: ID_FILE_SAVE as u16 },
    ACCEL { fVirt: FCONTROL | FSHIFT | FVIRTKEY, key: 'S' as u16, cmd: ID_FILE_SAVE_AS as u16 },
];

// Window State Management
pub struct WindowState {
    pub width: i32,
    pub height: i32,
}
static WINDOW_STATE: Lazy<Mutex<WindowState>> = Lazy::new(|| {
    Mutex::new(WindowState {
        width: 800,
        height: 600,
    })
});


// SetupGuard, to prevent commands during setup of a new or loaded file.
static mut IS_SETTING_UP: bool = false;
pub struct SetupGuard;
impl SetupGuard {
    pub fn new() -> Self {
        unsafe { IS_SETTING_UP = true; }
        SetupGuard
    }
}
impl Drop for SetupGuard {
    fn drop(&mut self) {
        unsafe { IS_SETTING_UP = false; }
    }
}
// =========

// =========
// About this Application Dialog
unsafe extern "system" fn about_dlg_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            if LOWORD(wparam as u32) as usize == 1 {
                DestroyWindow(hwnd);
                return 0;
            }
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            // Post quit message to exit the dialog's message loop
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn show_about_dialog(hwnd: HWND) {
    unsafe {
        let h_instance = GetModuleHandleW(null_mut());
        let class_name = widestring("about_dialog_class");
        let dialog_width = 350;
        let hyperlink_font = {
            CreateFontW(
                16, 0, 0, 0, FW_NORMAL, 1, 1, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH | FF_DONTCARE,
                widestring("").as_ptr(),
            )
        };

        // Register a custom window class for the dialog
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(about_dlg_proc),
            hInstance: h_instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW) as HBRUSH,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wnd_class);

        // Create a simple dialog window
        let about_hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class_name.as_ptr(),
            widestring("").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT, CW_USEDEFAULT, dialog_width, 250,
            hwnd, null_mut(), h_instance, null_mut(),
        );

        // App info
        let info = format!(
            "{} v{}\n\nThis application receives data packets from a PLC and stores them in a sqlite3 database.\n\nDeveloped by {}.",
            APPNAME, APPVERSION, APPAUTHOR
        );
        CreateWindowExW(
            0,
            widestring("STATIC").as_ptr(),
            widestring(&info).as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            10, 10, dialog_width-50, 80,
            about_hwnd, null_mut(), h_instance, null_mut(),
        );

        // Email as a "hyperlink"
        let email = widestring(APPEMAIL);
        let hdc = GetDC(about_hwnd);
        let mut size: SIZE = std::mem::zeroed();
        GetTextExtentPoint32W(hdc, email.as_ptr(), email.len() as i32, &mut size);
        ReleaseDC(about_hwnd, hdc);
        let email_width = size.cx + 8; // add some padding
        let email_x = ((dialog_width - email_width) / 2)-15;
        let h_email = CreateWindowExW(
            0,
            widestring("STATIC").as_ptr(),
            email.as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER,
            email_x, 100, email_width, 20,
            about_hwnd,
            ID_EMAIL_LABEL as _,
            h_instance,
            null_mut(),
        );
        SendMessageW(h_email, WM_SETFONT, hyperlink_font as WPARAM, 1);
        
        // OK button
        let _ = CreateWindowExW(
            0,
            widestring("BUTTON").as_ptr(),
            widestring("OK").as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON,
            120, 130, 80, 30,
            about_hwnd, 1 as _, h_instance, null_mut(),
        );

        ShowWindow(about_hwnd, SW_SHOW);
        UpdateWindow(about_hwnd);

        // Message loop for the dialog
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if GetMessageW(&mut msg, /*about_hwnd*/null_mut(), 0, 0) <= 0 {
                break;
            }
            
            if msg.hwnd == about_hwnd {
                if msg.message == WM_COMMAND {
                    log(&format!("WM_COMMAND received: wParam={}, lParam={}", msg.wParam, msg.lParam));
                    if LOWORD(msg.wParam as u32) as usize == 1 {
                        DestroyWindow(about_hwnd);
                        break;
                    }
                }

                if msg.message == WM_SETCURSOR {
                    let hwnd_cursor = msg.wParam as HWND;
                    if hwnd_cursor == h_email { 
                        SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                        //return 1;
                    }
                }

                if msg.message == WM_CTLCOLORSTATIC {
                    let hdc_static = msg.wParam as winapi::shared::windef::HDC;
                    let hwnd_static = msg.lParam as HWND;
                    // Check if this is the email label
                    if GetDlgCtrlID(hwnd_static) == h_email as i32 {
                        SetTextColor(hdc_static, RGB(0, 0, 255)); // Blue
                        SetBkMode(hdc_static, TRANSPARENT as i32);
                        SelectObject(hdc_static, hyperlink_font as HGDIOBJ);
                    }
                    //return GetStockObject(NULL_BRUSH as i32) as isize;
                }

                if /*msg.hwnd == about_hwnd &&*/ msg.message == WM_LBUTTONDOWN {
                    // Check if click is on the email label
                    let x = LOWORD(msg.lParam as u32) as i32;
                    let y = HIWORD(msg.lParam as u32) as i32;
                    log(&format!("Mouse click at: ({}, {})", x, y));
                    // Get the rectangle of the email label
                    let mut rect = std::mem::zeroed();
                    GetWindowRect(h_email, &mut rect);
                    // Convert label rect from screen to client coordinates
                    let mut top_left = winapi::shared::windef::POINT { x: rect.left, y: rect.top };
                    let mut bottom_right = winapi::shared::windef::POINT { x: rect.right, y: rect.bottom };
                    ScreenToClient(about_hwnd, &mut top_left);
                    ScreenToClient(about_hwnd, &mut bottom_right);
                    if x >= top_left.x && x <= bottom_right.x && y >= top_left.y && y <= bottom_right.y {
                    //if LOWORD(msg.wParam as u32) as i32 == ID_EMAIL_LABEL {
                        ShellExecuteW(
                            about_hwnd,
                            widestring("open").as_ptr(),
                            widestring(&format!("mailto:{}", APPEMAIL)).as_ptr(),
                            null(),
                            null(),
                            SW_SHOWNORMAL,
                        );
                    }
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
// =========

// Show Help File
fn show_help() -> std::io::Result<()>{
    let chm = env::current_exe()
        .expect("Failed to get current executable path")
        .with_file_name("s7-event-monitor.chm");
    Command::new("hh.exe").arg(chm).spawn()?;
    Ok(())
}

// =========
// Log Viewer
fn create_readonly_textbox(hwnd_parent: HWND) -> HWND {
    unsafe {
        let h_instance = GetModuleHandleW(null_mut());

        LoadLibraryW(widestring("Riched20.dll").as_ptr());

        // Create RichEdit control
        let h_edit = CreateWindowExW(
            0,
            widestring("RICHEDIT20W").as_ptr(),
            null_mut(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_HSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_READONLY,
            10,
            10,
            600,
            400,
            hwnd_parent,
            null_mut(),
            h_instance,
            null_mut(),
        );

        if h_edit.is_null() {
            if DEBUG { log("Failed to create RichEdit control."); }
            return 0 as HWND;
        }

        // Load file and set text
        if let Ok(content) = file_tail("log.txt",LOG_LINES) {//fs::read_to_string("log.txt") {
            let wide_text = widestring(&content);
            if DEBUG { log("Log file loaded into text box."); }
            let result = SendMessageW(h_edit, WM_SETTEXT, 0, wide_text.as_ptr() as LPARAM);
            if DEBUG { log(&format!("WM_SETTEXT result: {}", result)); }
        }

        return h_edit;
    }
}
// =========

// =========
// Status View
#[derive(Debug)]
pub struct ServerStatusControls {
    pub connected_hwnd: HWND,
    pub alive_hwnd: HWND,
    pub timestamp_hwnd: HWND,
    pub ip_hwnd: HWND,
    pub port_hwnd: HWND,
}
static mut HWNDS_STATUS_INFO: Vec<ServerStatusControls> = Vec::<ServerStatusControls>::new(); // Global vector to hold status controls
//
fn update_status_view(hwnd_parent: HWND, y: i32, servers: &[ServerEntry]) -> HWND {//, ServerStatusControls) {
    unsafe {
        let h_instance = GetModuleHandleW(null_mut());

        // 1. Create the container window (STATIC as a panel)
        let sts_hwnd = CreateWindowExW(
            WS_EX_TRANSPARENT,
            widestring("STATIC").as_ptr(),
            null(),
            WS_CHILD | WS_VISIBLE,
            0, y, 600, 600, // Adjust size as needed
            hwnd_parent,
            null_mut(),
            h_instance,
            null_mut(),
        );
        let old_proc = SetWindowLongPtrW(
            sts_hwnd,
            GWLP_WNDPROC,
            sts_subclass_proc as isize,
        );
        // Store the old proc in GWLP_USERDATA for later use
        SetWindowLongPtrW(sts_hwnd, GWLP_USERDATA, old_proc);

        let server_status = SERVER_STATUS.clone(); // Access the global server status
        log(&format!("Servers = {:?}", server_status));
        for (i, server) in server_status.server.iter().enumerate() {
            log(&format!("Server status i = {}", i));
            let y_offset = i as i32 * 65;

            let connected_hwnd = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring(&format!("Connected: {:?}",server.is_connected)).as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                10,
                y_offset,
                130,
                20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );
            let alive_hwnd = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring(&format!("Alive: {:?}",server.is_alive)).as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                150,
                y_offset,
                150,
                20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );
            // Timestamp of last packet received
            let _ = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring("Last packet received:").as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                10,
                y_offset+25,
                150,
                20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );
            let timestamp_hwnd = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring(&format!("{:?}", server.last_packet_time)).as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                160,
                y_offset+25,
                130,
                20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );

            let _ = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring("IP Address:").as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                300, y_offset, 100, 20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );

            let ip_hwnd = CreateWindowExW(
                0,
                widestring("EDIT").as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
                415, y_offset, 100, 20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );

            let _ = CreateWindowExW(
                0,
                widestring("STATIC").as_ptr(),
                widestring("Port:").as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT,
                300, y_offset+25, 70, 20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );

            let port_hwnd = CreateWindowExW(
                0,
                widestring("EDIT").as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
                415, y_offset+25, 100, 20,
                sts_hwnd,
                null_mut(),
                h_instance,
                null_mut(),
            );

            let save_btn_hwnd = CreateWindowExW(
                0,
                widestring("BUTTON").as_ptr(),
                widestring("Save").as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                530, y_offset, 60, 44,
                sts_hwnd,
                ID_SAVE_CONFIG as HMENU,
                h_instance,
                null_mut(),
            );
            // Store reference to server number in the button's user data
            SetWindowLongPtrW(save_btn_hwnd, GWLP_USERDATA, i as isize);

            // Set the font for the status view
            let font = CreateFontW(
                16, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH | FF_DONTCARE,
                widestring("").as_ptr(),
            );
            SendMessageW(connected_hwnd, WM_SETFONT, font as WPARAM, true as LPARAM);
            SendMessageW(alive_hwnd, WM_SETFONT, font as WPARAM, true as LPARAM);
            SendMessageW(timestamp_hwnd, WM_SETFONT, font as WPARAM, true as LPARAM);

            let ip_wide = widestring(&servers[i].ip_address);
            SetWindowTextW(ip_hwnd, ip_wide.as_ptr());
            let port_str = servers[i].port.to_string();
            let port_wide = widestring(&port_str);
            SetWindowTextW(port_hwnd, port_wide.as_ptr());

            if let Some(info) = HWNDS_STATUS_INFO.get_mut(i) {
                info.connected_hwnd = connected_hwnd;
                info.alive_hwnd = alive_hwnd;
                info.timestamp_hwnd = timestamp_hwnd;
                info.ip_hwnd = ip_hwnd;
                info.port_hwnd = port_hwnd;
            } else {
                HWNDS_STATUS_INFO.push(ServerStatusControls {
                    connected_hwnd,
                    alive_hwnd,
                    timestamp_hwnd,
                    ip_hwnd,
                    port_hwnd,
                });
            }
        }
        // Load config and set initial values
        // TODO: Use global config variable instead of loading from file every time
        /*if let Ok(config) = load_config("config.xml") {
            // IP Address
            let ip_wide = widestring(&config.servers[server_num].ip_address);
            SetWindowTextW(ip_hwnd, ip_wide.as_ptr());
            // Port
            let port_str = config.servers[server_num].port.to_string();
            let port_wide = widestring(&port_str);
            SetWindowTextW(EDIT_PORT_HWND, port_wide.as_ptr());
            // Timestamp
            let time_wide = widestring("No data received yet.");
            SetWindowTextW(STATUS_TIMESTAMP, time_wide.as_ptr());
        }*/
        sts_hwnd
    }
}
unsafe extern "system" fn sts_subclass_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            // Forward WM_COMMAND to the main window
            let main_hwnd = GetParent(hwnd);
            if !main_hwnd.is_null() {
                return SendMessageW(main_hwnd, WM_COMMAND, wparam, lparam);
            }
            0
        }
        _ => {
            // Call the original window proc
            let old_proc = std::mem::transmute::<isize, WNDPROC>(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
            if let Some(proc_fn) = old_proc {
                return CallWindowProcW(Some(proc_fn), hwnd, msg, wparam, lparam);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}
// =========

static mut OLD_TAB_PROC: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT> = None;
unsafe extern "system" fn tab_subclass_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            // Forward WM_COMMAND to the parent (main window)
            let parent = GetParent(hwnd);
            if !parent.is_null() {
                return SendMessageW(parent, WM_COMMAND, wparam, lparam);
            }
        }
        WM_UPDATE_STATUS_VIEW => {
            log("WM_UPDATE_STATUS_VIEW received, updating status view.");
            // Destroy the old status view
            if !STS_HWND.is_null() {
                DestroyWindow(STS_HWND);
            }
            // Recreate with the new server list
            let servers = &SERVER_CONFIG.servers;
            STS_HWND = update_status_view(TAB_HWND, 30, servers.as_slice());
            ShowWindow(STS_HWND, SW_SHOW);
            return 0;
        }
        _ => {}
    }
    // Call the original window proc
    if let Some(old_proc) = OLD_TAB_PROC {
        return CallWindowProcW(Some(old_proc), hwnd, msg, wparam, lparam);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            {
                let _guard = SetupGuard::new(); // IS_SETTING_UP = true
                unsafe {
                    let h_instance = GetModuleHandleW(null_mut());

                    // FILE MENU
                    let hmenu = CreateMenu();
                    let hmenu_file = CreateMenu();
                    //AppendMenuW(hmenu_file, MF_STRING, ID_FILE_NEW as usize, widestring("New").as_ptr());
                    //AppendMenuW(hmenu_file, MF_STRING, ID_FILE_OPEN as usize, widestring("Open").as_ptr());
                    //AppendMenuW(hmenu_file, MF_STRING, ID_FILE_SAVE as usize, widestring("Save").as_ptr());
                    //AppendMenuW(hmenu_file, MF_STRING, ID_FILE_SAVE_AS as usize, widestring("Save As").as_ptr());
                    //AppendMenuW(hmenu_file, MF_SEPARATOR, 0, null_mut());
                    AppendMenuW(hmenu_file, MF_STRING, ID_FILE_EXIT as usize, widestring("Exit").as_ptr());
                    AppendMenuW(hmenu, MF_POPUP, hmenu_file as usize, widestring("File").as_ptr());
                    // HELP MENU
                    let hmenu_help = CreateMenu();
                    AppendMenuW(hmenu_help, MF_STRING, ID_HELP_TOC as usize, widestring("Table Of Contents").as_ptr());
                    AppendMenuW(hmenu_help, MF_STRING, ID_HELP_START as usize, widestring("Getting Started").as_ptr());
                    AppendMenuW(hmenu_help, MF_SEPARATOR, 0, null_mut());
                    AppendMenuW(hmenu_help, MF_STRING, ID_HELP_ABOUT as usize, widestring("About").as_ptr());
                    AppendMenuW(hmenu, MF_POPUP | MF_HELP, hmenu_help as usize, widestring("Help").as_ptr());
                    // Set the menu to the window
                    SetMenu(hwnd, hmenu);

                    // Create Tab Control
                    TAB_HWND = CreateWindowExW(
                        0,
                        widestring("SysTabControl32").as_ptr(),
                        null_mut(),
                        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                        0, 0, 800, 600,
                        hwnd,
                        null_mut(),
                        h_instance,
                        null_mut(),
                    );
                    // Subclass
                    let old_proc = SetWindowLongPtrW(
                        TAB_HWND,
                        GWLP_WNDPROC,
                        tab_subclass_proc as isize,
                    );
                    OLD_TAB_PROC = Some(std::mem::transmute(old_proc));

                    // Add "Log" tab
                    {
                        let mut tcitem: TCITEMW = std::mem::zeroed();
                        tcitem.mask = winapi::um::commctrl::TCIF_TEXT;
                        let log_tab = widestring("Log");
                        tcitem.pszText = log_tab.as_ptr() as *mut u16;
                        SendMessageW(TAB_HWND, TCM_INSERTITEMW, 0, &tcitem as *const _ as LPARAM);
                    }
                    // Add "Status" tab
                    {
                        let mut tcitem: TCITEMW = std::mem::zeroed();
                        tcitem.mask = winapi::um::commctrl::TCIF_TEXT;
                        let log_tab = widestring("Status");
                        tcitem.pszText = log_tab.as_ptr() as *mut u16;
                        SendMessageW(TAB_HWND, TCM_INSERTITEMW, 0, &tcitem as *const _ as LPARAM);
                    }
                    // Show the log file
                    LOG_HWND = create_readonly_textbox(TAB_HWND);
                    let servers = &SERVER_CONFIG.servers;
                    let sts_hwnd = update_status_view(TAB_HWND, 30, servers.as_slice()); // TODO: make y variable for multiple connections
                    STS_HWND = sts_hwnd;
                    // Initial tab view/hide
                    ShowWindow(LOG_HWND, SW_HIDE);
                    ShowWindow(STS_HWND, SW_SHOW);
                    // Select the first tab (index 0)
                    SendMessageW(TAB_HWND, TCM_SETCURSEL, 0, 0); 
                }   
            }
            0
        }

        WM_GETMINMAXINFO => {
            let minmax = lparam as *mut MINMAXINFO;
            if !minmax.is_null() {
                // Set minimum window size
                (*minmax).ptMinTrackSize.x = 625;
                (*minmax).ptMinTrackSize.y = 600;
                // Optionally set maximum window size
                // (*minmax).ptMaxTrackSize.x = 1200;
                // (*minmax).ptMaxTrackSize.y = 900;
            }
            0
        }

        WM_COMMAND => {        
            unsafe {
                if IS_SETTING_UP {
                    return 0; // Ignore changes during setup
                }
            }
            let control_id = LOWORD(wparam as u32);
            let notification_code = HIWORD(wparam as u32);

            if notification_code == EN_CHANGE as u16 {
                println!("WM_COMMAND received: control_id={}, notification_code={}", control_id, notification_code);
            }

            match control_id {
                ID_FILE_EXIT => {
                    // Save window size and maximised state.
                    save_window_state_to_registry(hwnd);
                    // Notify other threads to shutdown
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Arc<Notify>;
                    if !ptr.is_null() {
                        let notify = &*ptr;
                        log("WM_DESTROY received, notifying shutdown.");
                        notify.notify_one();
                        // Clean up the Arc
                        drop(Box::from_raw(ptr));
                    }
                    PostQuitMessage(0);
                    0
                }
                ID_SAVE_CONFIG => {
                    unsafe {
                        // Get text from EDIT_IP_HWND and EDIT_PORT_HWND
                        let mut ip_buf = [0u16; 64];
                        let mut port_buf = [0u16; 16];
                        // Get the server number from the button's user data
                        let server_num = GetWindowLongPtrW(lparam as HWND, GWLP_USERDATA) as usize;
                        log(&format!("Saving config for server {}", server_num));
                        log(&format!("HWNDS_STATUS_INFO: {:?}", HWNDS_STATUS_INFO));
                        if let Some(ctrls) = HWNDS_STATUS_INFO.get(server_num) {
                            let ip_len = GetWindowTextW(ctrls.ip_hwnd, ip_buf.as_mut_ptr(), ip_buf.len() as i32);
                            let port_len = GetWindowTextW(ctrls.port_hwnd, port_buf.as_mut_ptr(), port_buf.len() as i32);

                            let ip = String::from_utf16_lossy(&ip_buf[..ip_len as usize]);
                            let port_str = String::from_utf16_lossy(&port_buf[..port_len as usize]);
                            let port: u16 = port_str.trim().parse().unwrap_or(0);

                            // Update config struct
                            SERVER_CONFIG.servers[server_num].ip_address = ip.clone();
                            SERVER_CONFIG.servers[server_num].port = port;
                            println!("Saving config for server {}: IP={}, Port={}", server_num, ip, port);

                            // Save config
                            if let Err(e) = crate::xmlhandling::save_config("config.xml") {//, &config) {
                                log(&format!("Failed to save config: {}", e));
                            } else {
                                log("Config saved.");
                            }
                        }
                    }
                    0
                }
                ID_HELP_TOC => {
                    let _ = show_help();
                    0
                }
                ID_HELP_START => {
                    let _ = show_help();
                    0
                }
                ID_HELP_ABOUT => {
                    show_about_dialog(hwnd);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        /*WM_ACTIVATE => {
            if DEBUG { println!("Window Activated."); }
            /*if LOWORD(wparam as u32) != WA_INACTIVE {
                if let Some(line) = *LAST_FOCUSED_LINE.lock().unwrap() {
                    //focus_edit_by_line(line);
                    PostMessageW(hwnd, WM_RESTORE_FOCUS, line as WPARAM, 0);
                }
            }*/
            0
        }*/

        /*WM_RESTORE_FOCUS => {
            let line = wparam as usize;
            focus_edit_by_line(line);
            if DEBUG { println!("Focus restored to line {}", line); }
            return 0;
        }*/

        WM_SIZE => {
            let width = LOWORD(lparam as u32) as i32;
            let height = HIWORD(lparam as u32) as i32;
            // Update Window Dimensions
            let mut state = WINDOW_STATE.lock().unwrap();
            state.width = width;
            state.height = height;
            if !TAB_HWND.is_null() {
                SetWindowPos(
                    TAB_HWND,
                    null_mut(),
                    0,
                    0,
                    width,
                    height,
                    SWP_NOZORDER,
                    );
                    // Resize the child view (e.g., log view) to fit inside the tab control
                    // Adjust y and height for the tab header (typically ~30px)
                    let tab_header_height = 30;
                    SetWindowPos(
                        LOG_HWND, // your log view HWND
                        null_mut(),
                        5, tab_header_height + 5,
                        width - 10, height - tab_header_height - 10,
                        SWP_NOZORDER,
                    );
            }
            0
        }

        WM_NEW_DATA => {
            // New data received from the server, update UI
            if DEBUG { log("UI received new data notification."); }
            let server_status = SERVER_STATUS.clone(); // Access the global server status
            // Load file and set text
            if let Ok(content) = file_tail("log.txt",LOG_LINES) {//fs::read_to_string("log.txt") {
                let wide_text = widestring(&content);
                if DEBUG { log("Log file loaded into text box."); }
                let result = SendMessageW(LOG_HWND, WM_SETTEXT, 0, wide_text.as_ptr() as LPARAM);
                if DEBUG { log(&format!("WM_SETTEXT for log result: {}", result)); }
            }
            // Find which server we are for
            //let server_number = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as usize;
            let server_number = wparam as usize;
            log(&format!("WM_NEW_DATA received for server {}", server_number));
            // Update status labels
            unsafe {
                if let Some(ctrls) = HWNDS_STATUS_INFO.get(server_number) {
                    log(&format!("Connected: {:?}", server_status.server[server_number].last_packet_time));
                    let connected_text = widestring(&format!("Connected: {:?}", server_status.server[server_number].is_connected));
                    let alive_text = widestring(&format!("Alive: {:?}", server_status.server[server_number].is_alive));
                    let timestamp = server_status.server[server_number].last_packet_time;
                    // Split into seconds and nanoseconds
                    let secs = (timestamp / 1000) as i64;
                    let nanos = ((timestamp % 1000) * 1_000_000) as u32;
                    let utc_dt = DateTime::<Utc>::from_timestamp(secs, nanos).unwrap();
                    let local_dt = utc_dt.with_timezone(&Local);
                    // Format the datetime as a string
                    let formatted = local_dt.format("%Y-%m-%d %H:%M:%S").to_string();
                
                    // TIMESTAMP DISPLAY
                    if !formatted.is_empty() && timestamp > 0 {
                        let timestamp_text = widestring(&formatted);
                        SetWindowTextW(ctrls.timestamp_hwnd, timestamp_text.as_ptr());
                    } else {
                        SetWindowTextW(ctrls.timestamp_hwnd, widestring("No data received yet").as_ptr());
                    }
                    // CONNECTED/ALIVE STATUS
                    if !ctrls.connected_hwnd.is_null() {
                        SetWindowTextW(ctrls.connected_hwnd, connected_text.as_ptr());
                    }
                    if !ctrls.alive_hwnd.is_null() {
                        SetWindowTextW(ctrls.alive_hwnd, alive_text.as_ptr());
                    }
                }
            }
            0
        }

        WM_NOTIFY => {
            let nmhdr = lparam as *const NMHDR;
            if !nmhdr.is_null() && (*nmhdr).hwndFrom == TAB_HWND {
                match (*nmhdr).code {
                    TCN_SELCHANGE => {
                        let sel = SendMessageW(TAB_HWND, TCM_GETCURSEL, 0, 0) as i32;
                        // Show/hide views based on selected tab
                        if sel == 0 {
                            ShowWindow(LOG_HWND, SW_HIDE);
                            ShowWindow(STS_HWND, SW_SHOW);
                        } else if sel == 1 {
                            ShowWindow(LOG_HWND, SW_SHOW);
                            ShowWindow(STS_HWND, SW_HIDE);
                        }
                        return 0;
                    }
                    _ => {}
                }
            }
            0
        }

        WM_DESTROY => {
            // Save window size and maximised state.
            save_window_state_to_registry(hwnd);
            // Notify other threads to shutdown
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Arc<Notify>;
            if !ptr.is_null() {
                let notify = &*ptr;
                log("WM_DESTROY received, notifying shutdown.");
                notify.notify_one();
                // Clean up the Arc
                drop(Box::from_raw(ptr));
            }
            //if let Some(notify) = &shutdown_notify {
            //    notify.notify_one();
            //}
            PostQuitMessage(0);
            0
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn main_window(
    shutdown_notify: Option<Arc<Notify>>, 
    rx: std::sync::mpsc::Receiver<ServerStatusInfo>,
    ui_ready_tx: Sender<()>) {
        unsafe {
            floem::launch(app_view);
            let _ = ui_ready_tx.send(());
            std::thread::spawn(move || {
                //let hwnd = hwnd_usize as HWND;
                while let Ok(server_status) = rx.recv() { //.is_ok() {
                    //unsafe {
                        //let mut glob_server_status = SERVER_STATUS.clone();
                        SERVER_STATUS.server[server_status.idx] = server_status.clone();
                        if server_status.new_data {
                            //PostMessageW(hwnd, WM_NEW_DATA, server_status.idx as WPARAM, 0);
                            //last_handled = Instant::now();
                        }
                    //}
                }
            });
        }
        //ui_ready_sent = true;
    //unsafe {
        /*let h_instance = GetModuleHandleW(null_mut());
        let class_name = widestring("my_window_class");

        
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..std::mem::zeroed()
        };

        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            widestring(APPNAME).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            null_mut(),
            null_mut(),
            h_instance,
            null_mut(),
        );
        //*MAIN_HWND.lock().unwrap() = hwnd;
        // After window creation, spawn a thread to listen for notifications:
        let hwnd_usize = hwnd as usize;
        //let mut last_handled = Instant::now();
        std::thread::spawn(move || {
            let hwnd = hwnd_usize as HWND;
            while let Ok(server_status) = rx.recv() { //.is_ok() {
                //unsafe {
                    //let mut glob_server_status = SERVER_STATUS.clone();
                    SERVER_STATUS.server[server_status.idx] = server_status.clone();
                    if server_status.new_data {
                        PostMessageW(hwnd, WM_NEW_DATA, server_status.idx as WPARAM, 0);
                        //last_handled = Instant::now();
                    }
                //}
            }
        });

        // Restore last window size and maximised state
        if let Some((left, top, width, height, maximized)) = load_window_state_from_registry() {
            SetWindowPos(hwnd, null_mut(), left, top, width, height, SWP_NOZORDER);
            if maximized {
                ShowWindow(hwnd, SW_MAXIMIZE);
            } else {
                ShowWindow(hwnd, SW_SHOWNORMAL);
            }
        }

        // Double buffering for smoother scrolling.
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | WS_EX_COMPOSITED as isize);

        // Store shutdown_notify in window user data
        if let Some(notify) = shutdown_notify {
            let boxed = Box::new(notify);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(boxed) as isize);
        }

        // Setup keyboard shortcuts
        let h_accel = {
            CreateAcceleratorTableW(ACCELERATORS.as_ptr() as *mut ACCEL, ACCELERATORS.len() as i32)
        };

        let mut ui_ready_sent = false;
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if !ui_ready_sent && !TAB_HWND.is_null() {
                let _ = ui_ready_tx.send(());
                ui_ready_sent = true;
            }
            if TranslateAcceleratorW(hwnd, h_accel, &mut msg) == 0 {
                // no accelerator -- handle normally
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }*/*/
}