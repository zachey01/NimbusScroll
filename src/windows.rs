use crate::app::Backend;
use crate::engine::{InputEvent, MouseDeviceInfo, OutputEvent, ScrollAxis, ScrollKey};

use std::collections::VecDeque;
use std::error::Error;
use std::ffi::c_void;
use std::mem::{self, ManuallyDrop};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

type BOOL = i32;
type UINT = u32;
type DWORD = u32;
type WORD = u16;
type LONG = i32;
type ULONG_PTR = usize;
type WPARAM = usize;
type LPARAM = isize;
type LRESULT = isize;
type HANDLE = *mut c_void;
type HINSTANCE = *mut c_void;
type HMODULE = *mut c_void;
type HWND = *mut c_void;
type HRAWINPUT = isize;
type HHOOK = *mut c_void;

pub(crate) struct WindowsBackend;

const XBUTTON1_DATA: u32 = 1;
const XBUTTON2_DATA: u32 = 2;

const WM_QUIT: UINT = 0x0012;
const WM_DESTROY: UINT = 0x0002;
const WM_INPUT: UINT = 0x00FF;
const WM_KEYDOWN: UINT = 0x0100;
const WM_KEYUP: UINT = 0x0101;
const WM_SYSKEYDOWN: UINT = 0x0104;
const WM_SYSKEYUP: UINT = 0x0105;

const RIDEV_INPUTSINK: DWORD = 0x00000100;
const RID_INPUT: UINT = 0x10000003;

const RIM_TYPEMOUSE: DWORD = 0;
const RIM_TYPEKEYBOARD: DWORD = 1;

const RI_MOUSE_LEFT_BUTTON_DOWN: WORD = 0x0001;
const RI_MOUSE_LEFT_BUTTON_UP: WORD = 0x0002;
const RI_MOUSE_RIGHT_BUTTON_DOWN: WORD = 0x0004;
const RI_MOUSE_RIGHT_BUTTON_UP: WORD = 0x0008;
const RI_MOUSE_MIDDLE_BUTTON_DOWN: WORD = 0x0010;
const RI_MOUSE_MIDDLE_BUTTON_UP: WORD = 0x0020;
const RI_MOUSE_BUTTON_4_DOWN: WORD = 0x0040;
const RI_MOUSE_BUTTON_4_UP: WORD = 0x0080;
const RI_MOUSE_BUTTON_5_DOWN: WORD = 0x0100;
const RI_MOUSE_BUTTON_5_UP: WORD = 0x0200;
const RI_MOUSE_WHEEL: WORD = 0x0400;
const RI_MOUSE_HWHEEL: WORD = 0x0800;

const RI_KEY_BREAK: WORD = 0x0001;

const MOUSEEVENTF_MOVE: DWORD = 0x0001;
const MOUSEEVENTF_LEFTDOWN: DWORD = 0x0002;
const MOUSEEVENTF_LEFTUP: DWORD = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: DWORD = 0x0008;
const MOUSEEVENTF_RIGHTUP: DWORD = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: DWORD = 0x0020;
const MOUSEEVENTF_MIDDLEUP: DWORD = 0x0040;
const MOUSEEVENTF_XDOWN: DWORD = 0x0080;
const MOUSEEVENTF_XUP: DWORD = 0x0100;
const MOUSEEVENTF_WHEEL: DWORD = 0x0800;
const MOUSEEVENTF_HWHEEL: DWORD = 0x1000;
const MOUSEEVENTF_MOVE_NOCOALESCE: DWORD = 0x2000;

const HWND_MESSAGE: HWND = (-3isize) as HWND;

static MAGIC_WORD: [u8; 8] = *b"PASS\0\0\0\0";

#[derive(Default)]
pub struct WindowsMouseHandle;

#[derive(Default)]
pub struct WindowsKeyboardHandle;

#[derive(Default)]
pub struct WindowsOutputHandle;

pub struct WindowsInputState {
    mouse_events: Mutex<VecDeque<InputEvent>>,
    keyboard_events: Mutex<VecDeque<InputEvent>>,
}

impl WindowsInputState {
    fn new() -> Self {
        Self {
            mouse_events: Mutex::new(VecDeque::new()),
            keyboard_events: Mutex::new(VecDeque::new()),
        }
    }

    fn push_mouse(&self, ev: InputEvent) {
        if let Ok(mut q) = self.mouse_events.lock() {
            q.push_back(ev);
        }
    }

    fn push_keyboard(&self, ev: InputEvent) {
        if let Ok(mut q) = self.keyboard_events.lock() {
            q.push_back(ev);
        }
    }

    fn drain_mouse(&self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.mouse_events.lock() {
            while let Some(ev) = q.pop_front() {
                out.push(ev);
            }
        }
        out
    }

    fn drain_keyboard(&self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        if let Ok(mut q) = self.keyboard_events.lock() {
            while let Some(ev) = q.pop_front() {
                out.push(ev);
            }
        }
        out
    }
}

static STATE: OnceLock<Arc<WindowsInputState>> = OnceLock::new();
static RAW_THREAD_ID: OnceLock<u32> = OnceLock::new();
static RAW_THREAD_STARTED: OnceLock<()> = OnceLock::new();

fn state() -> Arc<WindowsInputState> {
    STATE
        .get_or_init(|| Arc::new(WindowsInputState::new()))
        .clone()
}

pub(crate) fn request_exit() {
    if let Some(id) = RAW_THREAD_ID.get().copied() {
        unsafe {
            let _ = PostThreadMessageA(id, WM_QUIT, 0, 0);
        }
    }
}

fn ensure_thread_started() {
    let _ = RAW_THREAD_STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("nimbusscroll-win-rawinput".into())
            .spawn(move || unsafe {
                raw_input_thread_main();
            });
    });
}

fn list_mouse_devices_impl() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>> {
    Ok(Vec::new())
}

fn default_mouse_path_impl() -> Result<Option<String>, Box<dyn Error>> {
    Ok(None)
}

fn open_mouse_device_impl(_path: Option<&str>) -> Result<WindowsMouseHandle, Box<dyn Error>> {
    ensure_thread_started();
    Ok(WindowsMouseHandle)
}

fn open_keyboard_devices_impl() -> Result<Vec<WindowsKeyboardHandle>, Box<dyn Error>> {
    ensure_thread_started();
    Ok(vec![WindowsKeyboardHandle])
}

fn build_virtual_mouse_impl() -> Result<WindowsOutputHandle, Box<dyn Error>> {
    ensure_thread_started();
    Ok(WindowsOutputHandle)
}

fn reset_virtual_mouse_buttons_impl(_out: &mut WindowsOutputHandle) -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn poll_mouse_events_impl() -> Vec<InputEvent> {
    state().drain_mouse()
}

fn poll_keyboard_events_impl() -> Vec<InputEvent> {
    state().drain_keyboard()
}

fn active_window_signature_impl() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            None
        } else {
            Some(format!("{:p}", hwnd))
        }
    }
}

fn send_mouse_input(flags: DWORD, data: DWORD, dx: LONG, dy: LONG) {
    unsafe {
        let input = INPUT {
            type_: 0,
            input: INPUT_UNION {
                mi: ManuallyDrop::new(MOUSEINPUT {
                    dx,
                    dy,
                    mouse_data: data as i32,
                    dw_flags: flags,
                    time: 0,
                    dw_extra_info: MAGIC_WORD.as_ptr() as usize,
                }),
            },
        };

        let _ = SendInput(1, &input, mem::size_of::<INPUT>() as i32);
    }
}

fn send_relative_mouse_move(dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }
    send_mouse_input(MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE, 0, dx, dy);
}

fn emit_output_impl(event: OutputEvent) {
    match event {
        OutputEvent::Key { key, value } => match key {
            ScrollKey::Left => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_LEFTDOWN, 0, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_LEFTUP, 0, 0, 0);
                }
            }
            ScrollKey::Right => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_RIGHTDOWN, 0, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_RIGHTUP, 0, 0, 0);
                }
            }
            ScrollKey::Middle => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_MIDDLEUP, 0, 0, 0);
                }
            }
            ScrollKey::Side => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_XDOWN, XBUTTON1_DATA, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_XUP, XBUTTON1_DATA, 0, 0);
                }
            }
            ScrollKey::Back => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_XDOWN, XBUTTON1_DATA, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_XUP, XBUTTON1_DATA, 0, 0);
                }
            }
            ScrollKey::Extra => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_XDOWN, XBUTTON2_DATA, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_XUP, XBUTTON2_DATA, 0, 0);
                }
            }
            ScrollKey::Forward => {
                if value != 0 {
                    send_mouse_input(MOUSEEVENTF_XDOWN, XBUTTON2_DATA, 0, 0);
                } else {
                    send_mouse_input(MOUSEEVENTF_XUP, XBUTTON2_DATA, 0, 0);
                }
            }
            ScrollKey::LeftMeta | ScrollKey::RightMeta | ScrollKey::Task | ScrollKey::Other(_) => {}
        },
        OutputEvent::Rel { axis, value } => match axis {
            ScrollAxis::X => send_relative_mouse_move(value, 0),
            ScrollAxis::Y => send_relative_mouse_move(0, value),
            ScrollAxis::Wheel => send_mouse_input(MOUSEEVENTF_WHEEL, value as u32, 0, 0),
            ScrollAxis::WheelHiRes => send_mouse_input(MOUSEEVENTF_WHEEL, value as u32, 0, 0),
            ScrollAxis::HWheel => send_mouse_input(MOUSEEVENTF_HWHEEL, value as u32, 0, 0),
            ScrollAxis::HWheelHiRes => send_mouse_input(MOUSEEVENTF_HWHEEL, value as u32, 0, 0),
            ScrollAxis::Other(_) => {}
        },
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MSG {
    hwnd: HWND,
    message: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
    time: DWORD,
    pt_x: LONG,
    pt_y: LONG,
    l_private: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RAWINPUTDEVICE {
    us_usage_page: WORD,
    us_usage: WORD,
    dw_flags: DWORD,
    hwnd_target: HWND,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RAWINPUTHEADER {
    dw_type: DWORD,
    dw_size: DWORD,
    h_device: HANDLE,
    w_param: WPARAM,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RAWMOUSE {
    us_flags: WORD,
    _reserved: WORD,
    us_button_flags: WORD,
    us_button_data: WORD,
    ul_raw_buttons: DWORD,
    l_last_x: LONG,
    l_last_y: LONG,
    ul_extra_information: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RAWKEYBOARD {
    make_code: WORD,
    flags: WORD,
    reserved: WORD,
    v_key: WORD,
    message: UINT,
    extra_information: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
union RAWINPUT_DATA {
    mouse: ManuallyDrop<RAWMOUSE>,
    keyboard: ManuallyDrop<RAWKEYBOARD>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RAWINPUT {
    header: RAWINPUTHEADER,
    data: RAWINPUT_DATA,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct WNDCLASSA {
    style: UINT,
    lpfn_wnd_proc: Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: HINSTANCE,
    h_icon: HANDLE,
    h_cursor: HANDLE,
    hbr_background: HANDLE,
    lpsz_menu_name: *const u8,
    lpsz_class_name: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct INPUT {
    type_: DWORD,
    input: INPUT_UNION,
}

#[repr(C)]
#[derive(Clone, Copy)]
union INPUT_UNION {
    mi: ManuallyDrop<MOUSEINPUT>,
    ki: ManuallyDrop<KEYBDINPUT>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MOUSEINPUT {
    dx: LONG,
    dy: LONG,
    mouse_data: i32,
    dw_flags: DWORD,
    time: DWORD,
    dw_extra_info: ULONG_PTR,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KEYBDINPUT {
    w_vk: WORD,
    w_scan: WORD,
    dw_flags: DWORD,
    time: DWORD,
    dw_extra_info: ULONG_PTR,
}

#[link(name = "user32")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> HWND;
    fn PostThreadMessageA(id_thread: DWORD, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> BOOL;
    fn GetCurrentThreadId() -> DWORD;

    fn SendInput(c_inputs: UINT, p_inputs: *const INPUT, cb_size: i32) -> UINT;

    fn RegisterClassA(lp_wnd_class: *const WNDCLASSA) -> u16;
    fn CreateWindowExA(
        dw_ex_style: DWORD,
        lp_class_name: *const u8,
        lp_window_name: *const u8,
        dw_style: DWORD,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: HWND,
        h_menu: HANDLE,
        h_instance: HINSTANCE,
        lp_param: *mut c_void,
    ) -> HWND;
    fn DefWindowProcA(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
    fn DispatchMessageA(lp_msg: *const MSG) -> isize;
    fn GetMessageA(lp_msg: *mut MSG, h_wnd: HWND, w_msg_filter_min: UINT, w_msg_filter_max: UINT) -> i32;
    fn TranslateMessage(lp_msg: *const MSG) -> i32;
    fn PostQuitMessage(n_exit_code: i32);

    fn GetModuleHandleA(lp_module_name: *const u8) -> HMODULE;

    fn RegisterRawInputDevices(
        p_raw_input_devices: *const RAWINPUTDEVICE,
        ui_num_devices: UINT,
        cb_size: UINT,
    ) -> BOOL;

    fn GetRawInputData(
        h_raw_input: HRAWINPUT,
        ui_command: UINT,
        p_data: *mut c_void,
        pcb_size: *mut UINT,
        cb_size_header: UINT,
    ) -> UINT;
}

unsafe extern "system" fn raw_input_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            handle_raw_input(lparam);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcA(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_raw_input(lparam: LPARAM) {
    let mut size: UINT = 0;

    if GetRawInputData(
        lparam,
        RID_INPUT,
        ptr::null_mut(),
        &mut size,
        mem::size_of::<RAWINPUTHEADER>() as UINT,
    ) == u32::MAX
    {
        return;
    }

    if size == 0 {
        return;
    }

    let mut buf = vec![0u8; size as usize];
    if GetRawInputData(
        lparam,
        RID_INPUT,
        buf.as_mut_ptr() as *mut c_void,
        &mut size,
        mem::size_of::<RAWINPUTHEADER>() as UINT,
    ) == u32::MAX
    {
        return;
    }

    let raw = ptr::read_unaligned(buf.as_ptr() as *const RAWINPUT);
    let st = state();

    match raw.header.dw_type {
        RIM_TYPEMOUSE => {
            let mouse = raw.data.mouse;

            if mouse.ul_extra_information as usize == MAGIC_WORD.as_ptr() as usize {
                return;
            }

            if mouse.l_last_x != 0 {
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::X,
                    value: mouse.l_last_x,
                });
            }

            if mouse.l_last_y != 0 {
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::Y,
                    value: mouse.l_last_y,
                });
            }

            let flags = mouse.us_button_flags;

            if flags & RI_MOUSE_LEFT_BUTTON_DOWN != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Left,
                    value: 1,
                });
            }
            if flags & RI_MOUSE_LEFT_BUTTON_UP != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Left,
                    value: 0,
                });
            }
            if flags & RI_MOUSE_RIGHT_BUTTON_DOWN != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Right,
                    value: 1,
                });
            }
            if flags & RI_MOUSE_RIGHT_BUTTON_UP != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Right,
                    value: 0,
                });
            }
            if flags & RI_MOUSE_MIDDLE_BUTTON_DOWN != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Middle,
                    value: 1,
                });
            }
            if flags & RI_MOUSE_MIDDLE_BUTTON_UP != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Middle,
                    value: 0,
                });
            }
            if flags & RI_MOUSE_BUTTON_4_DOWN != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Back,
                    value: 1,
                });
            }
            if flags & RI_MOUSE_BUTTON_4_UP != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Back,
                    value: 0,
                });
            }
            if flags & RI_MOUSE_BUTTON_5_DOWN != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Forward,
                    value: 1,
                });
            }
            if flags & RI_MOUSE_BUTTON_5_UP != 0 {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Forward,
                    value: 0,
                });
            }

            if flags & RI_MOUSE_WHEEL != 0 {
                let delta = mouse.us_button_data as i16 as i32;
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::WheelHiRes,
                    value: delta,
                });
            }

            if flags & RI_MOUSE_HWHEEL != 0 {
                let delta = mouse.us_button_data as i16 as i32;
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::HWheelHiRes,
                    value: delta,
                });
            }
        }

        RIM_TYPEKEYBOARD => {
            let kb = raw.data.keyboard;

            let key = match kb.v_key as u32 {
                0x5B => Some(ScrollKey::LeftMeta),
                0x5C => Some(ScrollKey::RightMeta),
                _ => None,
            };

            if let Some(key) = key {
                let value = if (kb.flags & RI_KEY_BREAK) != 0 { 0 } else { 1 };
                st.push_keyboard(InputEvent::Key { key, value });
            }
        }

        _ => {}
    }
}

unsafe fn raw_input_thread_main() {
    let thread_id = GetCurrentThreadId();
    let _ = RAW_THREAD_ID.set(thread_id);

    let class_name = b"NimbusScrollRawInput\0";
    let h_instance = GetModuleHandleA(ptr::null());

    let wc = WNDCLASSA {
        style: 0,
        lpfn_wnd_proc: Some(raw_input_wnd_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance,
        h_icon: ptr::null_mut(),
        h_cursor: ptr::null_mut(),
        hbr_background: ptr::null_mut(),
        lpsz_menu_name: ptr::null(),
        lpsz_class_name: class_name.as_ptr(),
    };

    if RegisterClassA(&wc) == 0 {
        return;
    }

    let hwnd = CreateWindowExA(
        0,
        class_name.as_ptr(),
        class_name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        ptr::null_mut(),
        h_instance,
        ptr::null_mut(),
    );

    if hwnd.is_null() {
        return;
    }

    let devices = [
        RAWINPUTDEVICE {
            us_usage_page: 0x01,
            us_usage: 0x02,
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
        RAWINPUTDEVICE {
            us_usage_page: 0x01,
            us_usage: 0x06,
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd,
        },
    ];

    if RegisterRawInputDevices(&devices[0], 2, mem::size_of::<RAWINPUTDEVICE>() as UINT) == 0 {
        return;
    }

    let mut msg = MSG {
        hwnd: ptr::null_mut(),
        message: 0,
        w_param: 0,
        l_param: 0,
        time: 0,
        pt_x: 0,
        pt_y: 0,
        l_private: 0,
    };

    loop {
        let got = GetMessageA(&mut msg, ptr::null_mut(), 0, 0);
        if got <= 0 {
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageA(&msg);
    }
}

impl Backend for WindowsBackend {
    type Mouse = WindowsMouseHandle;
    type Keyboard = WindowsKeyboardHandle;
    type Output = WindowsOutputHandle;

    fn list_mouse_devices() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>> {
        list_mouse_devices_impl()
    }

    fn default_mouse_path() -> Result<Option<String>, Box<dyn Error>> {
        default_mouse_path_impl()
    }

    fn open_mouse_device(path: Option<&str>) -> Result<Self::Mouse, Box<dyn Error>> {
        open_mouse_device_impl(path)
    }

    fn set_mouse_nonblocking(_mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn grab_mouse(_mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn open_keyboard_devices() -> Result<Vec<Self::Keyboard>, Box<dyn Error>> {
        open_keyboard_devices_impl()
    }

    fn set_keyboard_nonblocking(_kb: &mut Self::Keyboard) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn poll_mouse_events(_mouse: &mut Self::Mouse) -> Result<Vec<InputEvent>, Box<dyn Error>> {
        Ok(poll_mouse_events_impl())
    }

    fn poll_keyboard_events(_kb: &mut Self::Keyboard) -> Result<Vec<InputEvent>, Box<dyn Error>> {
        Ok(poll_keyboard_events_impl())
    }

    fn build_virtual_mouse() -> Result<Self::Output, Box<dyn Error>> {
        build_virtual_mouse_impl()
    }

    fn reset_virtual_mouse_buttons(_out: &mut Self::Output) -> Result<(), Box<dyn Error>> {
        reset_virtual_mouse_buttons_impl(_out)
    }

    fn emit_output(_out: &mut Self::Output, event: OutputEvent) -> Result<(), Box<dyn Error>> {
        emit_output_impl(event);
        Ok(())
    }

    fn active_window_signature() -> Option<String> {
        active_window_signature_impl()
    }

    fn sleep(duration: Duration) {
        std::thread::sleep(duration);
    }
}