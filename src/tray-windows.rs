use crate::app::UiHandles;
use std::error::Error;
use std::ffi::c_void;
use std::mem;
use std::ptr;

const WM_APP: u32 = 0x8000;
pub(crate) const WM_TRAYICON: u32 = WM_APP + 1;

const TRAY_ICON_ID: u32 = 1;
const CMD_SETTINGS: usize = 1001;
const CMD_ABOUT: usize = 1002;
const CMD_EXIT: usize = 1003;

const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;
const NIM_SETVERSION: u32 = 0x00000004;

const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;

const NOTIFYICON_VERSION_4: u32 = 4;

const WM_RBUTTONUP: u32 = 0x0205;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_CLOSE: u32 = 0x0010;
const WM_NULL: u32 = 0x0000;

const MF_STRING: u32 = 0x00000000;
const TPM_LEFTALIGN: u32 = 0x0000;
const TPM_RIGHTBUTTON: u32 = 0x0002;
const TPM_RETURNCMD: u32 = 0x0100;
const TPM_BOTTOMALIGN: u32 = 0x0020;

const IDI_APPLICATION: *const u8 = 32512 as usize as *const u8;

#[repr(C)]
#[derive(Copy, Clone)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct NOTIFYICONDATAA {
    cb_size: u32,
    h_wnd: *mut c_void,
    u_id: u32,
    u_flags: u32,
    u_callback_message: u32,
    h_icon: *mut c_void,
    sz_tip: [u8; 128],
    dw_state: u32,
    dw_state_mask: u32,
    sz_info: [u8; 256],
    u_timeout_or_version: u32,
    sz_info_title: [u8; 64],
    dw_info_flags: u32,
    guid_item: [u8; 16],
    h_balloon_icon: *mut c_void,
}

#[link(name = "user32")]
#[link(name = "shell32")]
unsafe extern "system" {
    fn LoadIconA(h_instance: *mut c_void, lp_icon_name: *const u8) -> *mut c_void;
    fn Shell_NotifyIconA(dw_message: u32, lp_data: *mut NOTIFYICONDATAA) -> i32;

    fn CreatePopupMenu() -> *mut c_void;
    fn DestroyMenu(h_menu: *mut c_void) -> i32;
    fn AppendMenuA(
        h_menu: *mut c_void,
        u_flags: u32,
        u_id_new_item: usize,
        lp_new_item: *const u8,
    ) -> i32;
    fn TrackPopupMenu(
        h_menu: *mut c_void,
        u_flags: u32,
        x: i32,
        y: i32,
        n_reserved: i32,
        h_wnd: *mut c_void,
        prc_rect: *const c_void,
    ) -> u32;
    fn GetCursorPos(lp_point: *mut POINT) -> i32;
    fn SetForegroundWindow(h_wnd: *mut c_void) -> i32;
    fn PostMessageA(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> i32;
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

pub(crate) fn start(ui: UiHandles) -> Result<(), Box<dyn Error>> {
    std::thread::Builder::new()
        .name("tray-windows".into())
        .spawn(move || unsafe {
            let icon = LoadIconA(ptr::null_mut(), IDI_APPLICATION);
            if icon.is_null() {
                eprintln!(
                    "[ERROR] tray icon load failed: {}",
                    std::io::Error::last_os_error()
                );
                return;
            }

            let hwnd = crate::windows::tray_hwnd();
            if hwnd.is_null() {
                return;
            }

            let tip = cstr("NimbusScroll");
            let mut nid: NOTIFYICONDATAA = mem::zeroed();
            nid.cb_size = mem::size_of::<NOTIFYICONDATAA>() as u32;
            nid.h_wnd = hwnd;
            nid.u_id = TRAY_ICON_ID;
            nid.u_flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.u_callback_message = WM_TRAYICON;
            nid.h_icon = icon;

            let copy_len = tip.len().min(nid.sz_tip.len());
            nid.sz_tip[..copy_len].copy_from_slice(&tip[..copy_len]);

            if Shell_NotifyIconA(NIM_ADD, &mut nid) == 0 {
                eprintln!(
                    "[ERROR] tray add failed: {}",
                    std::io::Error::last_os_error()
                );
                return;
            }

            nid.u_timeout_or_version = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconA(NIM_SETVERSION, &mut nid);

            crate::windows::tray_set_ui(ui);

            let _ = crate::windows::tray_run_loop();
        })?;

    Ok(())
}

pub(crate) fn handle_message(hwnd: *mut c_void, msg: u32, _w_param: usize, l_param: isize) -> bool {
    if msg != WM_TRAYICON {
        return false;
    }

    let reason = l_param as u32;
    if reason != WM_RBUTTONUP && reason != WM_LBUTTONUP && reason != WM_CONTEXTMENU {
        return true;
    }

    let _ = show_menu(hwnd);
    true
}

fn show_menu(hwnd: *mut c_void) -> Result<(), Box<dyn Error>> {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        let settings = cstr("Settings");
        let about = cstr("About");
        let exit = cstr("Exit");

        let _ = AppendMenuA(menu, MF_STRING, CMD_SETTINGS, settings.as_ptr());
        let _ = AppendMenuA(menu, MF_STRING, CMD_ABOUT, about.as_ptr());
        let _ = AppendMenuA(menu, MF_STRING, CMD_EXIT, exit.as_ptr());

        let mut pt = POINT { x: 0, y: 0 };
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);

        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            ptr::null(),
        );

        DestroyMenu(menu);

        match cmd as usize {
            CMD_SETTINGS => {
                crate::windows::tray_show_settings();
            }
            CMD_ABOUT => {
                crate::windows::tray_show_about();
            }
            CMD_EXIT => {
                crate::windows::request_exit();
                let _ = slint::quit_event_loop();
                let _ = PostMessageA(hwnd, WM_CLOSE, 0, 0);
            }
            _ => {
                let _ = PostMessageA(hwnd, WM_NULL, 0, 0);
            }
        }
    }

    Ok(())
}

pub(crate) fn cleanup(hwnd: *mut c_void) {
    unsafe {
        let mut nid: NOTIFYICONDATAA = mem::zeroed();
        nid.cb_size = mem::size_of::<NOTIFYICONDATAA>() as u32;
        nid.h_wnd = hwnd;
        nid.u_id = TRAY_ICON_ID;
        let _ = Shell_NotifyIconA(NIM_DELETE, &mut nid);
    }
}
