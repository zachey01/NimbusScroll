#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::c_void;
use std::ptr;
use std::mem;
use std::sync::Mutex;

const IDI_APPLICATION: &[u8] = b"IDI_APPLICATION\0";
const IDC_ARROW: &[u8] = b"IDC_ARROW\0";
const RI_MOUSE_WHEEL: u16 = 0x0400;
const RIM_TYPEMOUSE: u32 = 0;

// Logging macros
macro_rules! log_info {
    ($($arg:tt)*) => {{
        let msg = format!("[INFO] {}\n", format_args!($($arg)*));
        println!("{}", msg);
        unsafe { OutputDebugStringA(msg.as_ptr() as _) };
    }};
}

macro_rules! log_error {
    ($($arg:tt)*) => {{
        let msg = format!("[ERROR] {}\n", format_args!($($arg)*));
        eprintln!("{}", msg);
        unsafe { OutputDebugStringA(msg.as_ptr() as _) };
    }};
}

// Defer struct for cleanup
struct Defer<F: FnOnce()> {
    f: Option<F>,
}
impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}

macro_rules! defer {
    ($($body:tt)*) => {
        let _defer = Defer {
            f: Some(|| { $($body)* }),
        };
    };
}

// Constants
const LIBRE_SCROLL_VERSION_TEXT: &str = "v1.0.0";
const MAGIC_WORD: [u8; 8] = *b"PASS\0\0\0\0";
const WM_TRAY: u32 = 0x8001;
const WM_RAW_STOPPED: u32 = 0x8002;
const WM_RAW_STARTED: u32 = 0x8003;
const WM_HOOK_STOPPED: u32 = 0x8004;
const WM_HOOK_STARTED: u32 = 0x8005;
const TRAY_UID: u32 = 0x69;

// Global configuration
static mut GLOBAL_CONFIG: Config = Config {
    decay: 3,
    sens_y: 18,
    sens_x: 0,
    step_y: 120,
    step_x: 120,
    flick: 0,
    think: 0,
};

// Global state
static PROCESS_MUTEX: Mutex<Option<usize>> = Mutex::new(None);
static MAIN_THREAD_ID: Mutex<u32> = Mutex::new(0);
static RAW_THREAD_ID: Mutex<u32> = Mutex::new(0);
static RAW_THREAD_HANDLE: Mutex<Option<usize>> = Mutex::new(None);
static RAW_THREAD_PENDING: Mutex<bool> = Mutex::new(false);

// Vector types
#[derive(Clone, Copy)]
struct Vec2f {
    x: f32,
    y: f32,
}
#[derive(Clone, Copy)]
struct Vec2i {
    x: i32,
    y: i32,
}

// Scrolling state
struct State {
    vel: Vec2f,
    res: Vec2f,
    rect: [i32; 4],
    is_button_scrolling: bool,
    cancel_pending: bool,
}

impl State {
    fn new() -> Self {
        State {
            vel: Vec2f { x: 0.0, y: 0.0 },
            res: Vec2f { x: 0.0, y: 0.0 },
            rect: [0; 4],
            is_button_scrolling: false,
            cancel_pending: false,
        }
    }

    fn step(&mut self, acu: Vec2i, tick: u64, freq: u64) -> Option<Vec2f> {
        unsafe {
            if self.is_button_scrolling {
                let mut current_rect = [0i32; 4];
                GetClipCursor(&mut current_rect);
                if current_rect != self.rect {
                    ClipCursor(&self.rect);
                }
            }
            let delta = Vec2f {
                x: GLOBAL_CONFIG.sens_x as f32 * acu.x as f32,
                y: GLOBAL_CONFIG.sens_y as f32 * acu.y as f32,
            };
            self.vel.x += delta.x;
            self.vel.y += delta.y;
            let dt = tick as f32 / freq as f32;
                let mu = GLOBAL_CONFIG.decay as f32;
        let dt = tick as f32 / freq as f32;
        let f0 = (-dt * mu).exp();
        let f1 = (1.0 - f0) / mu;

        let mut send = self.vel;
        send.x *= f1;
        send.y *= f1;
        self.vel.x *= f0;
        self.vel.y *= f0;

        // Only zero out velocity when it's extremely small
        if self.vel.x * self.vel.x + self.vel.y * self.vel.y < 0.1 {
            self.vel = Vec2f { x: 0.0, y: 0.0 };
        }
            Some(send)
        }
    }

fn flush(&mut self, delta: Vec2f) {
    unsafe {
        let send = Vec2i {
            x: delta.x as i32,
            y: delta.y as i32,
        };
        if send.x == 0 && send.y == 0 {
            return;
        }

        let mut inputs = Vec::new();
        if send.y != 0 {
            inputs.push(INPUT {
                type_: 0,
                input: INPUT_UNION {
                    mi: std::mem::ManuallyDrop::new(MOUSEINPUT {
                        mouse_data: send.y,  // <-- Fixed: removed the negative sign
                        dw_flags: 0x0800,    // MOUSEEVENTF_WHEEL
                        ..Default::default()
                    }),
                },
            });
        }
        if send.x != 0 {
            inputs.push(INPUT {
                type_: 0,
                input: INPUT_UNION {
                    mi: std::mem::ManuallyDrop::new(MOUSEINPUT {
                        mouse_data: send.x,
                        dw_flags: 0x1000,    // MOUSEEVENTF_HWHEEL
                        ..Default::default()
                    }),
                },
            });
        }
        if !inputs.is_empty() {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }
}
}

// Configuration struct
#[derive(Clone, Copy)]
struct Config {
    decay: i32,
    sens_y: i32,
    sens_x: i32,
    step_y: i32,
    step_x: i32,
    flick: i32,
    think: i32,
}

// Windows API types
#[repr(C)]
struct MSG {
    hwnd: *mut c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt: [i32; 2],
    l_private: u32,
}
#[repr(C)]
struct RAWINPUTDEVICE {
    us_usage_page: u16,
    us_usage: u16,
    dw_flags: u32,
    hwnd_target: *mut c_void,
}
#[repr(C)]
struct RAWINPUT {
    header: RAWINPUT_HEADER,
    data: RAWINPUT_DATA,
}
#[repr(C)]
struct RAWINPUT_HEADER {
    dw_type: u32,
    dw_size: u32,
    h_device: *mut c_void,
    w_param: usize,
}
#[repr(C)]
union RAWINPUT_DATA {
    mouse: std::mem::ManuallyDrop<RAWINPUT_MOUSE>,
    keyboard: std::mem::ManuallyDrop<RAWINPUT_KEYBOARD>,
}
#[repr(C)]
struct RAWINPUT_MOUSE {
    us_flags: u16,
    _reserved: u16,
    us_button_flags: u16,
    us_button_data: i16,
    ul_raw_buttons: u32,
    l_last_x: i32,
    l_last_y: i32,
    ul_extra_information: u32,
}
#[repr(C)]
struct RAWINPUT_KEYBOARD {
    make_code: u16,
    flags: u16,
    reserved: u16,
    v_key: u16,
    message: u32,
    extra_information: u32,
}
#[repr(C)]
struct INPUT {
    type_: u32,
    input: INPUT_UNION,
}
#[repr(C)]
union INPUT_UNION {
    mi: std::mem::ManuallyDrop<MOUSEINPUT>,
    ki: std::mem::ManuallyDrop<KEYBDINPUT>,
}
#[repr(C)]
struct MOUSEINPUT {
    dx: i32,
    dy: i32,
    mouse_data: i32,
    dw_flags: u32,
    time: u32,
    dw_extra_info: usize,
}
impl Default for MOUSEINPUT {
    fn default() -> Self {
        MOUSEINPUT {
            dx: 0,
            dy: 0,
            mouse_data: 0,
            dw_flags: 0,
            time: 0,
            dw_extra_info: 0,
        }
    }
}
#[repr(C)]
struct KEYBDINPUT {
    w_vk: u16,
    w_scan: u16,
    dw_flags: u32,
    time: u32,
    dw_extra_info: usize,
}
impl Default for KEYBDINPUT {
    fn default() -> Self {
        KEYBDINPUT {
            w_vk: 0,
            w_scan: 0,
            dw_flags: 0,
            time: 0,
            dw_extra_info: 0,
        }
    }
}
#[repr(C)]
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
    u_timeout: u32,
    sz_info_title: [u8; 64],
    dw_info_flags: u32,
    guid_item: u128,
    h_balloon_icon: *mut c_void,
}
#[repr(C)]
struct NOTIFYICONIDENTIFIER {
    cb_size: u32,
    h_wnd: *mut c_void,
    u_id: u32,
    guid_item: u128,
}
#[repr(C)]
struct MSLLHOOKSTRUCT {
    pt: [i32; 2],
    mouse_data: u32,
    flags: u32,
    time: u32,
    dw_extra_info: usize,
}

#[link(name = "user32")]
#[link(name = "kernel32")]
#[link(name = "gdi32")]
#[link(name = "shell32")]
#[link(name = "comctl32")]
#[link(name = "ole32")]
#[link(name = "oleaut32")]
#[link(name = "advapi32")]
#[link(name = "shlwapi")]
#[link(name = "comdlg32")]
#[link(name = "winmm")]
#[link(name = "ws2_32")]
unsafe extern "system" {
    fn OutputDebugStringA(lp_output_string: *const u8);
    fn CreateMutexA(lp_security_attributes: *const c_void, b_initial_owner: i32, lp_name: *const u8) -> *mut c_void;
    fn GetModuleFileNameA(h_module: *mut c_void, lp_filename: *mut u8, n_size: u32) -> u32;
    fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
    fn GetPrivateProfileIntA(lp_app_name: *const u8, lp_key_name: *const u8, n_default: i32, lp_file_name: *const u8) -> i32;
    fn WritePrivateProfileStringA(lp_app_name: *const u8, lp_key_name: *const u8, lp_string: *const u8, lp_file_name: *const u8) -> i32;
    fn SetThreadPriority(h_thread: *mut c_void, n_priority: i32) -> i32;
    fn GetWindowLongPtrA(h_wnd: *mut c_void, n_index: i32) -> isize;
    fn SetWindowLongPtrA(h_wnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
    fn SetWindowLongA(h_wnd: *mut c_void, n_index: i32, dw_new_long: i32) -> i32;
    fn SetWindowTextA(h_wnd: *mut c_void, lp_string: *const u8) -> i32;
    fn CreateWindowExA(dw_ex_style: u32, lp_class_name: *const u8, lp_window_name: *const u8, dw_style: u32, x: i32, y: i32, n_width: i32, n_height: i32, h_wnd_parent: *mut c_void, h_menu: *mut c_void, h_instance: *mut c_void, lp_param: *mut c_void) -> *mut c_void;
    fn DestroyWindow(h_wnd: *mut c_void) -> i32;
    fn ShowWindowAsync(h_wnd: *mut c_void, n_cmd_show: i32) -> i32;
    fn IsWindowVisible(h_wnd: *mut c_void) -> i32;
    fn PostQuitMessage(n_exit_code: i32);
    fn PostThreadMessageA(id_thread: u32, msg: u32, w_param: usize, l_param: isize) -> i32;
    fn SendMessageA(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> i32;
    fn GetMessageA(lp_msg: *mut MSG, h_wnd: *mut c_void, w_msg_filter_min: u32, w_msg_filter_max: u32) -> i32;
    fn DispatchMessageA(lp_msg: *const MSG) -> isize;
    fn TranslateMessage(lp_msg: *const MSG) -> i32;
    fn RegisterRawInputDevices(p_raw_input_devices: *const RAWINPUTDEVICE, ui_num_devices: u32, cb_size: u32) -> i32;
    fn GetRawInputData(h_raw_input: isize, ui_command: u32, p_data: *mut c_void, pcb_size: *mut u32, cb_size_header: u32) -> u32;
    fn SendInput(c_inputs: u32, p_inputs: *const INPUT, cb_size: i32) -> u32;
    fn LoadIconA(h_instance: *mut c_void, lp_icon_name: *const u8) -> *mut c_void;
    fn LoadMenuA(h_instance: *mut c_void, lp_menu_name: *const u8) -> *mut c_void;
    fn DestroyMenu(h_menu: *mut c_void) -> i32;
    fn TrackPopupMenu(h_menu: *mut c_void, u_flags: u32, x: i32, y: i32, n_reserved: i32, h_wnd: *mut c_void, prc_rect: *const c_void) -> u32;
    fn SetForegroundWindow(h_wnd: *mut c_void) -> i32;
    fn GetSubMenu(h_menu: *mut c_void, n_pos: i32) -> *mut c_void;
    fn MessageBoxA(h_wnd: *mut c_void, lp_text: *const u8, lp_caption: *const u8, u_type: u32) -> i32;
    fn SetTimer(h_wnd: *mut c_void, n_id_event: usize, u_elapse: u32, lp_timer_func: *const c_void) -> usize;
    fn KillTimer(h_wnd: *mut c_void, u_id_event: usize) -> i32;
    fn GetClipCursor(lp_rect: *mut [i32; 4]) -> i32;
    fn GetCursorPos(lp_point: *mut [i32; 2]) -> i32;
    fn ClipCursor(lp_rect: *const [i32; 4]) -> i32;
    fn SetThreadDpiAwarenessContext(dpi_context: isize) -> isize;
    fn CreateDialogParamA(h_instance: *mut c_void, lp_template_name: *const u8, h_wnd_parent: *mut c_void, lp_dialog_func: *const c_void, dw_init_param: isize) -> *mut c_void;
    fn GetDlgItem(h_dlg: *mut c_void, n_iddlg_item: i32) -> *mut c_void;
    fn SetDlgItemInt(h_dlg: *mut c_void, n_iddlg_item: i32, u_value: u32, b_signed: i32) -> i32;
    fn GetDlgItemInt(h_dlg: *mut c_void, n_iddlg_item: i32, lp_translated: *mut i32, b_signed: i32) -> u32;
    fn GetDlgItemTextA(h_wnd: *mut c_void, n_iddlg_item: i32, lp_string: *mut u8, cch_max: i32) -> u32;
    fn IsDialogMessageA(h_dlg: *mut c_void, lp_msg: *mut MSG) -> i32;
    fn IsDlgButtonChecked(h_dlg: *mut c_void, n_id_button: i32) -> u32;
    fn CheckDlgButton(h_dlg: *mut c_void, n_id_button: i32, u_check: u32) -> i32;
    fn SetWindowsHookExA(id_hook: i32, lpfn: *const c_void, h_mod: *mut c_void, dw_thread_id: u32) -> *mut c_void;
    fn UnhookWindowsHookEx(h_hook: *mut c_void) -> i32;
    fn CallNextHookEx(h_hook: *mut c_void, n_code: i32, w_param: usize, l_param: isize) -> isize;
    fn CallWindowProcA(lp_prev_wnd_func: *const c_void, h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize;
    fn IsUserAnAdmin() -> i32;
    fn ShellExecuteA(h_wnd: *mut c_void, lp_operation: *const u8, lp_file: *const u8, lp_parameters: *const u8, lp_directory: *const u8, n_show_cmd: i32) -> *mut c_void;
    fn Shell_NotifyIconGetRect(identifier: *const NOTIFYICONIDENTIFIER, rect: *mut [i32; 4]) -> i32;
    fn Shell_NotifyIconA(dw_message: u32, lp_data: *const NOTIFYICONDATAA) -> i32;
    fn GetCurrentThreadId() -> u32;
    fn CreateThread(lp_thread_attributes: *const c_void, dw_stack_size: usize, lp_start_address: *const c_void, lp_parameter: *mut c_void, dw_creation_flags: u32, lp_thread_id: *mut u32) -> *mut c_void;
    fn CloseHandle(h_object: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn QueryPerformanceFrequency(lp_frequency: *mut u64) -> i32;
    fn QueryPerformanceCounter(lp_performance_count: *mut u64) -> i32;
    fn FreeLibrary(h_lib_module: *mut c_void) -> i32;
    fn CreatePopupMenu() -> *mut c_void;
    fn AppendMenuA(h_menu: *mut c_void, u_flags: u32, u_id_new_item: usize, lp_new_item: *const u8) -> i32;
}

// DPI awareness context values
const DPI_AWARENESS_CONTEXT_NULL: isize = 0;
const DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED: isize = -5;
const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

// Shell notify icon messages
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIM_SETVERSION: u32 = 4;

fn main() {
    unsafe {
        log_info!("Starting NimbusScroll version {}", LIBRE_SCROLL_VERSION_TEXT);

        let mutex_name = cstr("NimbusScroll");
        let mutex = CreateMutexA(ptr::null(), 1, mutex_name.as_ptr()) as usize;
        *PROCESS_MUTEX.lock().unwrap() = Some(mutex);
        if mutex == 0 || GetLastError() != 0 {
            log_error!("Failed to create mutex - another instance may be running");
            MessageBoxA(
                ptr::null_mut(),
                cstr("Another instance of NimbusScroll is already running.").as_ptr(),
                cstr("NimbusScroll").as_ptr(),
                0x30,
            );
            return;
        }
        *MAIN_THREAD_ID.lock().unwrap() = GetCurrentThreadId();

        let h_instance = GetModuleHandleA(ptr::null());
        let wnd_class = cstr("NimbusScroll");

        let wc = WNDCLASSA {
            style: 0,
            lpfn_wnd_proc: Some(tray_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: h_instance,
            h_icon: LoadIconA(h_instance, IDI_APPLICATION.as_ptr()),
            h_cursor: LoadCursorA(h_instance, IDC_ARROW.as_ptr()),
            hbr_background: (COLOR_WINDOW + 1) as *mut c_void,
            lpsz_menu_name: ptr::null(),
            lpsz_class_name: wnd_class.as_ptr(),
        };

        if RegisterClassA(&wc) == 0 {
            log_error!("Failed to register window class");
            MessageBoxA(
                ptr::null_mut(),
                cstr("Failed to register window class").as_ptr(),
                cstr("NimbusScroll Error").as_ptr(),
                0x10,
            );
            return;
        }

        let hwnd_tray = CreateWindowExA(
            0,
            wnd_class.as_ptr(),
            cstr("NimbusScroll Tray").as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );

        if hwnd_tray.is_null() {
            log_error!("Failed to create tray window");
            MessageBoxA(
                ptr::null_mut(),
                cstr("Failed to create system tray window").as_ptr(),
                cstr("NimbusScroll Error").as_ptr(),
                0x10,
            );
            return;
        }

        let h_sens_y = CreateWindowExA(
            0,
            cstr("EDIT").as_ptr(),
            ptr::null(),
            0x50000000,
            0,
            0,
            0,
            0,
            hwnd_tray,
            0x4002 as _,
            h_instance,
            ptr::null_mut(),
        );

        let h_sens_x = CreateWindowExA(
            0,
            cstr("EDIT").as_ptr(),
            ptr::null(),
            0x50000000,
            0,
            0,
            0,
            0,
            hwnd_tray,
            0x4003 as _,
            h_instance,
            ptr::null_mut(),
        );

        if !h_sens_y.is_null() && !h_sens_x.is_null() {
            SetWindowLongPtrA(h_sens_y, -21, SetWindowLongPtrA(h_sens_y, -4, input_proc as isize));
            SetWindowLongPtrA(h_sens_x, -21, SetWindowLongPtrA(h_sens_x, -4, input_proc as isize));
        }

        let ico = {
            let cpl = LoadLibraryA(cstr("main.cpl").as_ptr());
            if cpl.is_null() {
                LoadIconA(h_instance, IDI_APPLICATION.as_ptr())
            } else {
                let icon = LoadIconA(cpl, 608 as *const u8);
                FreeLibrary(cpl);
                if icon.is_null() {
                    LoadIconA(h_instance, IDI_APPLICATION.as_ptr())
                } else {
                    icon
                }
            }
        };

        if !ico.is_null() {
            SendMessageA(hwnd_tray, 0x0080, 0, ico as isize);
            SendMessageA(hwnd_tray, 0x0080, 1, ico as isize);
        }

        let mut tray_data = NOTIFYICONDATAA {
            cb_size: mem::size_of::<NOTIFYICONDATAA>() as u32,
            h_wnd: hwnd_tray,
            u_id: TRAY_UID,
            u_flags: 0x8F,
            u_callback_message: WM_TRAY,
            h_icon: ico,
            u_timeout: 4,
            sz_tip: [0; 128],
            dw_state: 0,
            dw_state_mask: 1,
            sz_info: [0; 256],
            sz_info_title: [0; 64],
            dw_info_flags: 0,
            guid_item: 0,
            h_balloon_icon: ptr::null_mut(),
        };

        tray_data.sz_tip[..12].copy_from_slice(b"NimbusScroll");

        if Shell_NotifyIconA(NIM_ADD, &tray_data) == 0 {
            log_error!("Failed to add system tray icon");
            MessageBoxA(
                hwnd_tray,
                cstr("Failed to initialize system tray icon").as_ptr(),
                cstr("NimbusScroll Error").as_ptr(),
                0x10,
            );
        }

        let cleanup_tray_data = NOTIFYICONDATAA {
            cb_size: tray_data.cb_size,
            h_wnd: tray_data.h_wnd,
            u_id: tray_data.u_id,
            u_flags: tray_data.u_flags,
            u_callback_message: tray_data.u_callback_message,
            h_icon: tray_data.h_icon,
            sz_tip: tray_data.sz_tip,
            dw_state: tray_data.dw_state,
            dw_state_mask: tray_data.dw_state_mask,
            sz_info: tray_data.sz_info,
            u_timeout: tray_data.u_timeout,
            sz_info_title: tray_data.sz_info_title,
            dw_info_flags: tray_data.dw_info_flags,
            guid_item: tray_data.guid_item,
            h_balloon_icon: tray_data.h_balloon_icon,
        };

        defer! {
            log_info!("Cleaning up resources...");
            Shell_NotifyIconA(NIM_DELETE, &cleanup_tray_data);
            DestroyWindow(hwnd_tray);
            if let Some(mutex) = *PROCESS_MUTEX.lock().unwrap() {
                CloseHandle(mutex as *mut c_void);
            }
        }

        if Shell_NotifyIconA(NIM_SETVERSION, &tray_data) == 0 {
            log_error!("Failed to set tray icon version");
        }

        if !start_thread() {
            log_error!("Failed to start raw input thread");
            MessageBoxA(
                hwnd_tray,
                cstr("Failed to start raw input processing thread").as_ptr(),
                cstr("NimbusScroll Error").as_ptr(),
                0x10,
            );
        }

        let mut msg: MSG = mem::zeroed();
        while GetMessageA(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if msg.hwnd.is_null() {
                if msg.message == WM_RAW_STOPPED {
                    log_info!("Raw input thread stopped");
                    tray_data.sz_tip[11..22].copy_from_slice(b" - Inactive");
                    Shell_NotifyIconA(NIM_MODIFY, &tray_data);
                    let h_pause = GetDlgItem(hwnd_tray, 104);
                    if !h_pause.is_null() {
                        SetWindowTextA(h_pause, cstr("Unpause").as_ptr());
                        SetWindowLongA(h_pause, -12, 105);
                    }
                    if let Some(handle) = *RAW_THREAD_HANDLE.lock().unwrap() {
                        CloseHandle(handle as *mut c_void);
                    }
                    *RAW_THREAD_HANDLE.lock().unwrap() = None;
                    if *RAW_THREAD_PENDING.lock().unwrap() {
                        *RAW_THREAD_PENDING.lock().unwrap() = false;
                        start_thread();
                    }
                } else if msg.message == WM_RAW_STARTED {
                    log_info!("Raw input thread started");
                    tray_data.sz_tip[11..20].copy_from_slice(b" - Active");
                    Shell_NotifyIconA(NIM_MODIFY, &tray_data);
                    let h_unpause = GetDlgItem(hwnd_tray, 105);
                    if !h_unpause.is_null() {
                        SetWindowTextA(h_unpause, cstr("Pause").as_ptr());
                        SetWindowLongA(h_unpause, -12, 104);
                    }
                }
            } else if IsDialogMessageA(hwnd_tray, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageA(&msg);
            }
        }
    }
}

unsafe extern "system" fn tray_proc(hwnd: *mut c_void, u_msg: u32, w_param: usize, l_param: isize) -> isize {
    match u_msg {
        0x0010 => {
            ShowWindowAsync(hwnd, 0);
            0
        }
        0x0111 => {
            on_wm_command(hwnd, w_param, l_param);
            1
        }
        WM_TRAY => {
            on_wm_tray(hwnd, w_param, l_param);
            1
        }
        0x0002 => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcA(hwnd, u_msg, w_param, l_param),
    }
}

const COLOR_WINDOW: i32 = 5;
const CW_USEDEFAULT: i32 = -2147483648;

#[repr(C)]
struct WNDCLASSA {
    style: u32,
    lpfn_wnd_proc: Option<unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: *mut c_void,
    h_icon: *mut c_void,
    h_cursor: *mut c_void,
    hbr_background: *mut c_void,
    lpsz_menu_name: *const u8,
    lpsz_class_name: *const u8,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassA(lp_wnd_class: *const WNDCLASSA) -> u16;
    fn LoadCursorA(h_instance: *mut c_void, lp_cursor_name: *const u8) -> *mut c_void;
    fn GetModuleHandleA(lp_module_name: *const u8) -> *mut c_void;
    fn DefWindowProcA(hwnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize;
}

unsafe extern "system" fn input_proc(hwnd: *mut c_void, u_msg: u32, w_param: usize, l_param: isize) -> isize {
    if u_msg == 0x0102 && w_param >= ' ' as usize {
        if w_param != '-' as usize || SendMessageA(hwnd, 0x00B0, 0, 0) != 0 {
            if !('0' as usize..='9' as usize).contains(&w_param) {
                return 0;
            }
        }
    }
    let proc = GetWindowLongPtrA(hwnd, -21) as *const c_void;
    CallWindowProcA(proc, hwnd, u_msg, w_param, l_param)
}

fn on_wm_command(hwnd: *mut c_void, w_param: usize, l_param: isize) {
    let id = w_param & 0xFFFF;
    match id {
        100 => quit(),
        101 => show(hwnd),
        102 => info(hwnd),
        103 => elevate(),
        104 => {
            if RAW_THREAD_HANDLE.lock().unwrap().is_some() {
                *RAW_THREAD_PENDING.lock().unwrap() = false;
                unsafe { PostThreadMessageA(*RAW_THREAD_ID.lock().unwrap(), 0x0012, 0, 0); }
            }
        }
        105 | 106 => {
            if id == 106 {
                save(hwnd);
            }
            if RAW_THREAD_HANDLE.lock().unwrap().is_some() {
                *RAW_THREAD_PENDING.lock().unwrap() = true;
                unsafe { PostThreadMessageA(*RAW_THREAD_ID.lock().unwrap(), 0x0012, 0, 0); }
            } else {
                *RAW_THREAD_PENDING.lock().unwrap() = false;
                if !start_thread() {
                    quit();
                }
            }
        }
        _ => {}
    }
}

fn on_wm_tray(hwnd: *mut c_void, w_param: usize, l_param: isize) {
    let src_msg = (l_param as usize & 0xFFFF) as u16;
    let src_uid = ((l_param as usize) >> 16) as u16;
    let pos_x = (w_param as i32) & 0xFFFF;
    let pos_y = ((w_param as i32) >> 16) & 0xFFFF;
    match src_msg {
        0x007B => menu(hwnd, src_uid, pos_x as i16, pos_y as i16),
        0x0400 => show(hwnd),
        _ => {}
    }
}

fn elevate() {
    unsafe {
        log_info!("Requesting elevation");
        let mut buf = [0u8; 32767];
        let len = GetModuleFileNameA(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32);
        if len == 0 || (len == buf.len() as u32 && GetLastError() != 0) {
            log_error!("Failed to get module file name");
            return;
        }
        if let Some(mutex) = *PROCESS_MUTEX.lock().unwrap() {
            CloseHandle(mutex as *mut c_void);
        }
        ShellExecuteA(
            ptr::null_mut(),
            cstr("runas").as_ptr(),
            buf.as_ptr(),
            ptr::null(),
            ptr::null(),
            0,
        );
        quit();
    }
}

fn quit() {
    log_info!("Shutting down...");
    unsafe {
        PostQuitMessage(0);
    }
}

fn info(hwnd: *mut c_void) {
    unsafe {
        log_info!("Displaying about dialog");
        let text = cstr("Visit https://github.com/zachey01/NimbusScroll for more info.");
        let caption = cstr(&format!("About NimbusScroll {}", LIBRE_SCROLL_VERSION_TEXT));
        MessageBoxA(hwnd, text.as_ptr(), caption.as_ptr(), 0);
    }
}

fn menu(hwnd: *mut c_void, uid: u16, x: i16, y: i16) {
    unsafe {
        log_info!("Displaying system tray menu");

        // Создаем корневое меню
        let tray_hmenu = CreatePopupMenu();
        if tray_hmenu.is_null() {
            log_error!("Failed to create popup menu");
            return;
        }

        defer! {
            DestroyMenu(tray_hmenu);
        }

        // Определяем состояние пользователя (админ/не админ)
        let is_admin = IsUserAnAdmin() != 0;
        let thread_active = RAW_THREAD_HANDLE.lock().unwrap().is_some();

        // Добавляем пункты меню в зависимости от состояния
        if thread_active {
            AppendMenuA(tray_hmenu, 0, 104, cstr("Stop Thread").as_ptr());
        } else {
            AppendMenuA(tray_hmenu, 0, 105, cstr("Start Thread").as_ptr());
        }

        // Пункт "Restart as admin" всегда доступен
        AppendMenuA(tray_hmenu, 0, 103, cstr("Restart as Admin").as_ptr());

        // Разделитель
        AppendMenuA(tray_hmenu, 0x800, 0, ptr::null());

        // Общие пункты
        AppendMenuA(tray_hmenu, 0, 102, cstr("About NimbusScroll").as_ptr());
        AppendMenuA(tray_hmenu, 0, 101, cstr("Options").as_ptr());
        AppendMenuA(tray_hmenu, 0, 100, cstr("Quit").as_ptr());

        // Показываем меню
        let mut rect = [0i32; 4];
        let identifier = NOTIFYICONIDENTIFIER {
            cb_size: mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            h_wnd: hwnd,
            u_id: uid as u32,
            guid_item: 0,
        };
        Shell_NotifyIconGetRect(&identifier, &mut rect);

        SetForegroundWindow(hwnd);
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        TrackPopupMenu(tray_hmenu, 0, x as i32, y as i32, 0, hwnd, ptr::null());
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_NULL);
    }
}

fn show(hwnd: *mut c_void) {
    unsafe {
        log_info!("Displaying configuration window");
        SetDlgItemInt(hwnd, 0x4001, GLOBAL_CONFIG.decay as u32, 0);
        SetDlgItemInt(hwnd, 0x4002, GLOBAL_CONFIG.sens_y as u32, 1);
        SetDlgItemInt(hwnd, 0x4003, GLOBAL_CONFIG.sens_x as u32, 1);
        SetDlgItemInt(hwnd, 0x4004, GLOBAL_CONFIG.step_y as u32, 0);
        SetDlgItemInt(hwnd, 0x4005, GLOBAL_CONFIG.step_x as u32, 0);
        CheckDlgButton(hwnd, 0x4006, GLOBAL_CONFIG.flick as u32);
        CheckDlgButton(hwnd, 0x4007, GLOBAL_CONFIG.think as u32);
        if IsWindowVisible(hwnd) == 0 {
            ShowWindowAsync(hwnd, 5);
        }
        SetForegroundWindow(hwnd);
    }
}

fn save(hwnd: *mut c_void) {
    unsafe {
        log_info!("Saving configuration");
        let ini = cstr("./options.ini");
        let sec = cstr("NimbusScroll");
        let mut buf = [0u8; 32767];
        for (key, id) in [
            ("decay", 0x4001),
            ("sensY", 0x4002),
            ("sensX", 0x4003),
            ("stepY", 0x4004),
            ("stepX", 0x4005),
        ].iter() {
            GetDlgItemTextA(hwnd, *id, buf.as_mut_ptr(), buf.len() as i32);
            WritePrivateProfileStringA(sec.as_ptr(), cstr(key).as_ptr(), buf.as_ptr(), ini.as_ptr());
        }
        WritePrivateProfileStringA(
            sec.as_ptr(),
            cstr("flick").as_ptr(),
            cstr(if IsDlgButtonChecked(hwnd, 0x4006) == 0 { "0" } else { "1" }).as_ptr(),
            ini.as_ptr(),
        );
        WritePrivateProfileStringA(
            sec.as_ptr(),
            cstr("think").as_ptr(),
            cstr(if IsDlgButtonChecked(hwnd, 0x4007) == 0 { "0" } else { "1" }).as_ptr(),
            ini.as_ptr(),
        );
    }
}

fn start_thread() -> bool {
    unsafe {
        log_info!("Starting raw input thread");
        let ini = cstr("./options.ini");
        let sec = cstr("NimbusScroll");
        GLOBAL_CONFIG.decay = GetPrivateProfileIntA(sec.as_ptr(), cstr("decay").as_ptr(), GLOBAL_CONFIG.decay, ini.as_ptr()).max(0);
        GLOBAL_CONFIG.sens_y = GetPrivateProfileIntA(sec.as_ptr(), cstr("sensY").as_ptr(), GLOBAL_CONFIG.sens_y, ini.as_ptr());
        GLOBAL_CONFIG.sens_x = GetPrivateProfileIntA(sec.as_ptr(), cstr("sensX").as_ptr(), GLOBAL_CONFIG.sens_x, ini.as_ptr());
        GLOBAL_CONFIG.step_y = GetPrivateProfileIntA(sec.as_ptr(), cstr("stepY").as_ptr(), GLOBAL_CONFIG.step_y, ini.as_ptr()).max(0);
        GLOBAL_CONFIG.step_x = GetPrivateProfileIntA(sec.as_ptr(), cstr("stepX").as_ptr(), GLOBAL_CONFIG.step_x, ini.as_ptr()).max(0);
        GLOBAL_CONFIG.flick = GetPrivateProfileIntA(sec.as_ptr(), cstr("flick").as_ptr(), GLOBAL_CONFIG.flick, ini.as_ptr()).clamp(0, 1);
        GLOBAL_CONFIG.think = GetPrivateProfileIntA(sec.as_ptr(), cstr("think").as_ptr(), GLOBAL_CONFIG.think, ini.as_ptr()).clamp(0, 1);

        let mut thread_id = 0;
        let handle = CreateThread(ptr::null(), 0, raw_main as _, ptr::null_mut(), 0, &mut thread_id) as usize;
        if handle == 0 {
            log_error!("Failed to create raw input thread");
            return false;
        }

        SetThreadPriority(handle as *mut c_void, 15);
        *RAW_THREAD_ID.lock().unwrap() = thread_id;
        *RAW_THREAD_HANDLE.lock().unwrap() = Some(handle);
        log_info!("Raw input thread started successfully");
        true
    }
}

unsafe extern "system" fn hook_proc(code: i32, w_param: usize, l_param: isize) -> isize {
    if w_param == 0x207 || w_param == 0x208 {
        let inf = &*(l_param as *const MSLLHOOKSTRUCT);
        let pass = MAGIC_WORD.as_ptr() as usize;
        if inf.flags & 3 == 0 || inf.dw_extra_info != pass {
            return 1;
        }
    }
    CallNextHookEx(ptr::null_mut(), code, w_param, l_param)
}

unsafe extern "system" fn hook_main(_: *mut c_void) -> u32 {
    defer! {
        PostThreadMessageA(*RAW_THREAD_ID.lock().unwrap(), 0x0012, 0, 0);
    }
    log_info!("Starting low-level mouse hook");
    let h_hook = SetWindowsHookExA(14, hook_proc as _, ptr::null_mut(), 0);
    if h_hook.is_null() {
        log_error!("Failed to install low-level mouse hook");
        return 0;
    }
    defer! {
        UnhookWindowsHookEx(h_hook);
    }
    PostThreadMessageA(*RAW_THREAD_ID.lock().unwrap(), WM_HOOK_STARTED, 0, 0);
    let mut msg: MSG = mem::zeroed();
    while GetMessageA(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        log_info!("Received message: {}", msg.message);
        DispatchMessageA(&msg);
    }
    0
}

unsafe extern "system" fn raw_main(_: *mut c_void) -> u32 {
    defer! {
        PostThreadMessageA(*MAIN_THREAD_ID.lock().unwrap(), WM_RAW_STOPPED, 0, 0);
    }
    log_info!("Raw input processing thread started");

    if SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == DPI_AWARENESS_CONTEXT_NULL {
        log_error!("Failed to set DPI awareness context for raw input thread");
        return 0;
    }

    let hwnd = CreateWindowExA(
        0,
        cstr("Message").as_ptr(),
        ptr::null(),
        0x80000000,  // WS_POPUP style
        0,
        0,
        0,
        0,
        !2usize as *mut c_void,
        ptr::null_mut(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if hwnd.is_null() {
        log_error!("Failed to create raw input window");
        return 0;
    }

    defer! {
        DestroyWindow(hwnd);
    }

    let raw_input_device = RAWINPUTDEVICE {
        us_usage_page: 0x01, // Generic desktop controls
        us_usage: 0x02,      // Mouse
        dw_flags: 0x00000100,         hwnd_target: hwnd,
    };

    if RegisterRawInputDevices(&raw_input_device, 1, mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
        let err = GetLastError();
        log_error!("Failed to register raw input device. Error code: {}", err);
        match err {
            87 => log_error!("ERROR_INVALID_PARAMETER - Check RAWINPUTDEVICE structure"),
            1004 => log_error!("ERROR_INVALID_FLAGS - Invalid dwFlags value"),
            1008 => log_error!("ERROR_INVALID_HANDLE - Invalid hwndTarget"),
            1168 => log_error!("ERROR_NOT_FOUND - Device not found"),
            _ => {}
        }
        return 0;
    }

    defer! {
        let off_device = RAWINPUTDEVICE {
            us_usage_page: 0x01,
            us_usage: 0x02,
            dw_flags: 0x1,
            hwnd_target: ptr::null_mut(),
        };
        RegisterRawInputDevices(&off_device, 1, mem::size_of::<RAWINPUTDEVICE>() as u32);
    }

    let mut hook_active = false;
    let mut hook_thread_id = 0;
    let hook_thread_handle = CreateThread(ptr::null(), 0, hook_main as _, ptr::null_mut(), 0, &mut hook_thread_id) as usize;
    if hook_thread_handle == 0 {
        log_error!("Failed to create hook thread");
        return 0;
    }

    SetThreadPriority(hook_thread_handle as *mut c_void, 15);
    defer! {
        CloseHandle(hook_thread_handle as *mut c_void);
        PostThreadMessageA(hook_thread_id, 0x0012, 0, 0);
    }

    PostThreadMessageA(*MAIN_THREAD_ID.lock().unwrap(), WM_RAW_STARTED, 0, 0);

    let interval_ms = 10;
    let mut qpf = 0;
    QueryPerformanceFrequency(&mut qpf);
    let mut past = 0;
    QueryPerformanceCounter(&mut past);

    let mut size = mem::size_of::<RAWINPUT_MOUSE>() as u32;
    let mut data: RAWINPUT = mem::zeroed();
    let mut state = State::new();
    let mut timer = 0;
    let mut scroll_acu = Vec2i { x: 0, y: 0 };
    let mut unclip_pending = false;

    let mut msg: MSG = mem::zeroed();
    while GetMessageA(&mut msg, ptr::null_mut(), 0, 0) > 0 {
        DispatchMessageA(&msg);
        if !hook_active {
            if msg.message == WM_HOOK_STARTED {
                hook_active = true;
            }
            continue;
        }

        if msg.message == 0xff {
            if GetRawInputData(
                msg.l_param,
                0x10000003,
                &mut data as *mut _ as *mut c_void,
                &mut size,
                mem::size_of::<RAWINPUT_HEADER>() as u32,
            ) > 0 {
                if data.header.dw_type == RIM_TYPEMOUSE {
                    let flags = data.data.mouse.us_button_flags;
                    if data.header.h_device.is_null() {
                        if unclip_pending && flags & 32 == 32 {
                            unclip_pending = false;
                            ClipCursor(ptr::null());
                        }
                        continue;
                    }
        
// In the raw_main function, modify the RI_MOUSE_WHEEL handling:
if flags & RI_MOUSE_WHEEL != 0 {
    let delta = data.data.mouse.us_button_data as i32;
let delta = data.data.mouse.us_button_data as i32;
let velocity_increment = (delta as f32) * GLOBAL_CONFIG.sens_y as f32 / 120.0;
scroll_acu.y += velocity_increment as i32; // Добавляем в scroll_acu вместо прямого изменения velocity
log_info!("Wheel scroll: delta={}, velocity_increment={}", delta, velocity_increment);
    if timer == 0 {
        timer = SetTimer(ptr::null_mut(), 0, interval_ms, ptr::null());
    }
} else if flags & 16 == 16 {
                        state.is_button_scrolling = true;
                        state.cancel_pending = true;
                        scroll_acu = Vec2i { x: 0, y: 0 };
                        let mut cursor_pos = [0i32; 2];
                        GetCursorPos(&mut cursor_pos);
                        state.rect[0] = cursor_pos[0];
                        state.rect[1] = cursor_pos[1];
                        state.rect[2] = state.rect[0] + 1;
                        state.rect[3] = state.rect[1] + 1;
                        ClipCursor(&state.rect);
                        if timer == 0 {
                            timer = SetTimer(ptr::null_mut(), 0, interval_ms, ptr::null());
                        }
                    } else if flags & 32 == 32 {
                        state.is_button_scrolling = false;
                        if GLOBAL_CONFIG.flick == 0 {
                            state.vel = Vec2f { x: 0.0, y: 0.0 };
                            state.res = Vec2f { x: 0.0, y: 0.0 };
                            if KillTimer(ptr::null_mut(), timer) != 0 {
                                timer = 0;
                            }
                        }
                        if state.cancel_pending {
                            state.cancel_pending = false;
                            let cancel = [
                                INPUT {
                                    type_: 1,
                                    input: INPUT_UNION {
                                        ki: std::mem::ManuallyDrop::new(KEYBDINPUT {
                                            dw_flags: 0,
                                            ..Default::default()
                                        }),
                                    },
                                },
                                INPUT {
                                    type_: 1,
                                    input: INPUT_UNION {
                                        ki: std::mem::ManuallyDrop::new(KEYBDINPUT {
                                            dw_flags: 2,
                                            ..Default::default()
                                        }),
                                    },
                                },
                            ];
                            SendInput(2, cancel.as_ptr(), mem::size_of::<INPUT>() as i32);
                        }
                        ClipCursor(ptr::null());
                    } else if flags == 0 && state.is_button_scrolling {
                        scroll_acu.x += data.data.mouse.l_last_x;
                        scroll_acu.y += data.data.mouse.l_last_y;
                    }
                }
            }
        }

        let mut now = 0;
        QueryPerformanceCounter(&mut now);
        let dt = now - past;

        if dt * 1000 > qpf * interval_ms as u64 {
            log_info!("Processing scroll state - dt: {}ms", dt * 1000 / qpf);
            log_info!("Pre-step velocity: x={}, y={}", state.vel.x, state.vel.y);
            
            if let Some(send) = state.step(scroll_acu, dt, qpf) {
                log_info!("Sending scroll: x={}, y={}", send.x, send.y);
                state.flush(send);
            }
            
            scroll_acu = Vec2i { x: 0, y: 0 };
            past = now;
        }
    }
    log_info!("Raw input thread exiting");
    0
}


fn cstr(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.bytes().collect();
    v.push(0);
    v
}
