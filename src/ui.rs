use std::ptr::{null, null_mut};
use std::sync::Arc;
use tokio::sync::Notify;
use winapi::um::winuser::*;
//use winapi::um::winnt::{KEY_WRITE, KEY_READ, REG_DWORD, REG_SZ};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::windef::{HBRUSH, HWND}; 
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM, LOWORD, HIWORD};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use crate::registryhandling::*;   
use crate::utils::*;

// CONSTANTS
pub const ID_FILE_OPEN: u16 = 101;
pub const ID_FILE_SAVE: u16 = 102;
pub const ID_FILE_SAVE_AS: u16 = 103;
pub const ID_FILE_EXIT: u16 = 100;
pub const ID_FILE_NEW: u16 = 104;

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
                    SetMenu(hwnd, hmenu);
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
            /*SetWindowPos(
                SCROLL_VIEW,
                null_mut(),
                0,
                0,
                width,
                height,
                SWP_NOZORDER,
                );*/
            /*let ptr = GetWindowLongPtrW(SCROLL_VIEW, GWLP_USERDATA) as *mut EditorState;
            if !ptr.is_null() {
                let editor = &mut *ptr;
                update_editor_controls(SCROLL_VIEW, &mut *editor);//, Some(width), Some(height));
            }*/
            // Save window size and maximised state.
            //save_window_state_to_registry(hwnd);
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

pub fn main_window(shutdown_notify: Option<Arc<Notify>>) {
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
            widestring("Scrollable Editor").as_ptr(),
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