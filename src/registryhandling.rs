use std::ptr::{null, null_mut};
use winapi::um::winreg::{
    HKEY_CURRENT_USER, 
    RegCreateKeyExW, 
    RegSetValueExW, 
    RegGetValueW, 
    RegCloseKey, 
    RegOpenKeyExW, 
    RRF_RT_REG_SZ,
    RegQueryValueExW,
    };
use winapi::um::winnt::{KEY_WRITE, KEY_READ, REG_DWORD, REG_SZ};
use winapi::shared::minwindef::HKEY;
use winapi::shared::windef::HWND;
use winapi::um::winuser::{GetWindowPlacement, WINDOWPLACEMENT, SW_SHOWMAXIMIZED};
use crate::utils::*;

pub fn save_window_state_to_registry(hwnd: HWND) {
    let mut placement: WINDOWPLACEMENT = unsafe { std::mem::zeroed() };
    placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;

    if unsafe { GetWindowPlacement(hwnd, &mut placement) } != 0 {
        let normal_pos = placement.rcNormalPosition;
        let is_maximized = placement.showCmd == SW_SHOWMAXIMIZED as u32;

        let left = normal_pos.left;
        let top = normal_pos.top;
        let width = normal_pos.right - normal_pos.left;
        let height = normal_pos.bottom - normal_pos.top;

        let mut hkey: HKEY = std::ptr::null_mut();
        let key_string = format!("Software\\{}", APPNAME);
        let subkey = widestring(&key_string);

        unsafe {
            if RegCreateKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, null_mut(), 0, KEY_WRITE, null_mut(), &mut hkey, null_mut()) == 0 {
                RegSetValueExW(hkey, widestring("WindowLeft").as_ptr(), 0, REG_DWORD, &left as *const _ as *const u8, std::mem::size_of::<i32>() as u32);
                RegSetValueExW(hkey, widestring("WindowTop").as_ptr(), 0, REG_DWORD, &top as *const _ as *const u8, std::mem::size_of::<i32>() as u32);
                RegSetValueExW(hkey, widestring("WindowWidth").as_ptr(), 0, REG_DWORD, &width as *const _ as *const u8, std::mem::size_of::<i32>() as u32);
                RegSetValueExW(hkey, widestring("WindowHeight").as_ptr(), 0, REG_DWORD, &height as *const _ as *const u8, std::mem::size_of::<i32>() as u32);
                let maximized_val: u32 = if is_maximized { 1 } else { 0 };
                RegSetValueExW(hkey, widestring("WindowMaximized").as_ptr(), 0, REG_DWORD, &maximized_val as *const _ as *const u8, std::mem::size_of::<u32>() as u32);
                RegCloseKey(hkey);
                if DEBUG { println!("Saving window state: left={}, top={}, width={}, height={}, maximized={}", left, top, width, height, is_maximized); }
            }
        }
    }
}


pub fn load_window_state_from_registry() -> Option<(i32, i32, i32, i32, bool)> {
    let mut hkey: HKEY = std::ptr::null_mut();
    let key_string = format!("Software\\{}", APPNAME);
    let subkey = widestring(&key_string);

    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
            let mut left: i32 = 0;
            let mut top: i32 = 0;
            let mut width: i32 = 800;
            let mut height: i32 = 600;
            let mut maximized: u32 = 0;
            let mut size = std::mem::size_of::<i32>() as u32;

            RegQueryValueExW(hkey, widestring("WindowLeft").as_ptr(), null_mut(), null_mut(), &mut left as *mut _ as *mut u8, &mut size);
            RegQueryValueExW(hkey, widestring("WindowTop").as_ptr(), null_mut(), null_mut(), &mut top as *mut _ as *mut u8, &mut size);
            RegQueryValueExW(hkey, widestring("WindowWidth").as_ptr(), null_mut(), null_mut(), &mut width as *mut _ as *mut u8, &mut size);
            RegQueryValueExW(hkey, widestring("WindowHeight").as_ptr(), null_mut(), null_mut(), &mut height as *mut _ as *mut u8, &mut size);
            size = std::mem::size_of::<u32>() as u32;
            RegQueryValueExW(hkey, widestring("WindowMaximized").as_ptr(), null_mut(), null_mut(), &mut maximized as *mut _ as *mut u8, &mut size);

            RegCloseKey(hkey);
            if DEBUG { println!("Loaded window state: left={}, top={}, width={}, height={}, maximized={}", left, top, width, height, maximized != 0); }
            return Some((left, top, width, height, maximized != 0));
        }
    }

    None
}
