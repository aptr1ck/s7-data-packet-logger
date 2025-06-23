use std::fs;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::sync::Notify;
use winapi::um::winuser::*;
//use winapi::um::winnt::{KEY_WRITE, KEY_READ, REG_DWORD, REG_SZ};
use winapi::um::libloaderapi::{GetModuleHandleW, LoadLibraryW};
use winapi::um::shellapi::ShellExecuteW;
use winapi::um::wingdi::{GetTextExtentPoint32W, CreateFontW, FW_NORMAL, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, FF_DONTCARE};
use winapi::um::wingdi::{SelectObject, GetStockObject, SetTextColor, SetBkMode, TRANSPARENT, RGB};
use winapi::shared::windef::{HBRUSH, HWND, SIZE, HGDIOBJ}; 
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM, LOWORD, HIWORD};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use crate::filehandling::*;
use crate::registryhandling::*;   
use crate::utils::*;

// CONSTANTS
const ID_FILE_OPEN: u16 = 101;
const ID_FILE_SAVE: u16 = 102;
const ID_FILE_SAVE_AS: u16 = 103;
const ID_FILE_EXIT: u16 = 100;
const ID_FILE_NEW: u16 = 104;
const ID_HELP_ABOUT: u16 = 200;
const ID_EMAIL_LABEL: i32 = 5001;
const WM_NEW_DATA: u32 = WM_USER + 1;
const LOG_LINES: usize = 500; // Number of lines to show in the log viewer

// Global Mutables.
//static MAIN_HWND: Lazy<Mutex<HWND>> = Lazy::new(|| Mutex::new(std::ptr::null_mut()));
static mut CHILD_VIEW: HWND = null_mut();

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
        let hyperlink_font = unsafe {
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
        let h_ok = CreateWindowExW(
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
                    AppendMenuW(hmenu_help, MF_STRING, ID_HELP_ABOUT as usize, widestring("About").as_ptr());
                    AppendMenuW(hmenu, MF_POPUP, hmenu_help as usize, widestring("Help").as_ptr());
                    // Set the menu to the window
                    SetMenu(hwnd, hmenu);

                    // Show the log file
                    CHILD_VIEW = create_readonly_textbox(hwnd);
                    // Load the log file into the textbox
                    //load_file_to_textbox(logview_hwnd, "log.txt");
                }   
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
                ID_HELP_ABOUT => {
                    show_about_dialog(hwnd);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        WM_ACTIVATE => {
            if DEBUG { println!("Window Activated."); }
            /*if LOWORD(wparam as u32) != WA_INACTIVE {
                if let Some(line) = *LAST_FOCUSED_LINE.lock().unwrap() {
                    //focus_edit_by_line(line);
                    PostMessageW(hwnd, WM_RESTORE_FOCUS, line as WPARAM, 0);
                }
            }*/
            0
        }

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
            if !CHILD_VIEW.is_null() {
                SetWindowPos(
                    CHILD_VIEW,
                    null_mut(),
                    0,
                    0,
                    width,
                    height,
                    SWP_NOZORDER,
                    );
            }
            /*let ptr = GetWindowLongPtrW(SCROLL_VIEW, GWLP_USERDATA) as *mut EditorState;
            if !ptr.is_null() {
                let editor = &mut *ptr;
                update_editor_controls(SCROLL_VIEW, &mut *editor);//, Some(width), Some(height));
            }*/
            // Save window size and maximised state.
            //save_window_state_to_registry(hwnd);
            0
        }

        WM_NEW_DATA => {
            // Update your UI here (e.g., reload log file, show notification, etc.)
            if DEBUG { log("UI received new data notification."); }
            // Load file and set text
            if let Ok(content) = file_tail("log.txt",LOG_LINES) {//fs::read_to_string("log.txt") {
                let wide_text = widestring(&content);
                if DEBUG { log("Log file loaded into text box."); }
                let result = SendMessageW(CHILD_VIEW, WM_SETTEXT, 0, wide_text.as_ptr() as LPARAM);
                if DEBUG { log(&format!("WM_SETTEXT result: {}", result)); }
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

pub fn main_window(shutdown_notify: Option<Arc<Notify>>, rx: std::sync::mpsc::Receiver<()>) {
    unsafe {
        let h_instance = GetModuleHandleW(null_mut());
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
        let mut last_handled = Instant::now();
        std::thread::spawn(move || {
            let hwnd = hwnd_usize as HWND;
            while rx.recv().is_ok() {
                unsafe {
                    //if last_handled.elapsed() >= Duration::from_secs(1) {
                        PostMessageW(hwnd, WM_NEW_DATA, 0, 0);
                        last_handled = Instant::now();
                    //}
                    // else: ignore this notification
                }
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
        let hAccel = unsafe {
            CreateAcceleratorTableW(ACCELERATORS.as_ptr() as *mut ACCEL, ACCELERATORS.len() as i32)
        };

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if TranslateAcceleratorW(hwnd, hAccel, &mut msg) == 0 {
                // no accelerator -- handle normally
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}