use crate::app::UiHandles;
use crate::engine::{self, ImmediateAxis, MiddleDragState, ModifierState, MomentumAxis};
use std::error::Error;
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const MAGIC_WORD: [u8; 8] = *b"PASS\0\0\0\0";

const SCROLL_TIMER_ID: usize = 1;
const TIMER_INTERVAL_MS: u32 = 10;

const INPUT_MOUSE: u32 = 0;
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
const MOUSEEVENTF_HWHEEL: u32 = 0x1000;

const WM_INPUT: u32 = 0x00FF;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;

const WM_MOUSEWHEEL: usize = 0x020A;
const WM_MOUSEHWHEEL: usize = 0x020E;

const WH_MOUSE_LL: i32 = 14;
const HC_ACTION: i32 = 0;

const RI_MOUSE_WHEEL: u16 = 0x0400;
const RI_MOUSE_HWHEEL: u16 = 0x0800;
const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;

const RIM_TYPEMOUSE: u32 = 0;
const RIM_TYPEKEYBOARD: u32 = 1;

const RIDEV_INPUTSINK: u32 = 0x00000100;
const RID_INPUT: u32 = 0x10000003;

const LLMHF_INJECTED: u32 = 0x00000001;
const LLMHF_LOWER_IL_INJECTED: u32 = 0x00000002;

const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;

const HWND_MESSAGE: *mut c_void = (-3isize) as *mut c_void;

struct WinState {
    normal_wheel_v: MomentumAxis,
    normal_wheel_h: MomentumAxis,
    drag_wheel_v: MomentumAxis,
    drag_wheel_h: MomentumAxis,
    immediate_drag_v: ImmediateAxis,
    immediate_drag_h: ImmediateAxis,
    middle: MiddleDragState,
    modifiers: ModifierState,
    perf_freq: u64,
    last_counter: u64,
    timer_active: bool,
    hwnd: *mut c_void,
    foreground_hwnd: *mut c_void,
    ui: Option<UiHandles>,
}

impl WinState {
    const fn new() -> Self {
        Self {
            normal_wheel_v: MomentumAxis::new(),
            normal_wheel_h: MomentumAxis::new(),
            drag_wheel_v: MomentumAxis::new(),
            drag_wheel_h: MomentumAxis::new(),
            immediate_drag_v: ImmediateAxis::new(),
            immediate_drag_h: ImmediateAxis::new(),
            middle: MiddleDragState::new(),
            modifiers: ModifierState::new(),
            perf_freq: 0,
            last_counter: 0,
            timer_active: false,
            hwnd: ptr::null_mut(),
            foreground_hwnd: ptr::null_mut(),
            ui: None,
        }
    }

    fn clear_scroll(&mut self) {
        self.normal_wheel_v.clear();
        self.normal_wheel_h.clear();
        self.drag_wheel_v.clear();
        self.drag_wheel_h.clear();
        self.immediate_drag_v.clear();
        self.immediate_drag_h.clear();
    }

    fn clear_all_scroll_state(&mut self) {
        self.clear_scroll();
        self.middle.clear();
    }

    fn is_idle(&self) -> bool {
        self.normal_wheel_v.is_idle()
            && self.normal_wheel_h.is_idle()
            && self.drag_wheel_v.is_idle()
            && self.drag_wheel_h.is_idle()
            && self.immediate_drag_v.is_idle()
            && self.immediate_drag_h.is_idle()
    }
}

static STATE: Mutex<WinState> = Mutex::new(WinState::new());
static HWND_ATOM: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[repr(C)]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
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

#[repr(C)]
#[derive(Copy, Clone)]
struct RAWINPUTDEVICE {
    us_usage_page: u16,
    us_usage: u16,
    dw_flags: u32,
    hwnd_target: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RAWINPUTHEADER {
    dw_type: u32,
    dw_size: u32,
    h_device: *mut c_void,
    w_param: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RAWMOUSE {
    us_flags: u16,
    us_button_flags: u16,
    us_button_data: u16,
    ul_raw_buttons: u32,
    l_last_x: i32,
    l_last_y: i32,
    ul_extra_information: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RAWKEYBOARD {
    make_code: u16,
    flags: u16,
    reserved: u16,
    v_key: u16,
    message: u32,
    extra_information: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
union RAWINPUT_DATA {
    mouse: RAWMOUSE,
    keyboard: RAWKEYBOARD,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RAWINPUT {
    header: RAWINPUTHEADER,
    data: RAWINPUT_DATA,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct MOUSEINPUT {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    dw_flags: u32,
    time: u32,
    dw_extra_info: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
union INPUT_DATA {
    mi: MOUSEINPUT,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct INPUT {
    type_: u32,
    data: INPUT_DATA,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct MSLLHOOKSTRUCT {
    pt: [i32; 2],
    mouse_data: u32,
    flags: u32,
    time: u32,
    dw_extra_info: usize,
}

#[link(name = "user32")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(lp_module_name: *const u8) -> *mut c_void;
    fn RegisterClassA(lp_wnd_class: *const WNDCLASSA) -> u16;
    fn CreateWindowExA(
        dw_ex_style: u32,
        lp_class_name: *const u8,
        lp_window_name: *const u8,
        dw_style: u32,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: *mut c_void,
        h_menu: *mut c_void,
        h_instance: *mut c_void,
        lp_param: *mut c_void,
    ) -> *mut c_void;
    fn DestroyWindow(h_wnd: *mut c_void) -> i32;
    fn DefWindowProcA(hwnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize;
    fn PostQuitMessage(n_exit_code: i32);
    fn PostMessageA(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> i32;

    fn GetMessageA(
        lp_msg: *mut MSG,
        h_wnd: *mut c_void,
        w_msg_filter_min: u32,
        w_msg_filter_max: u32,
    ) -> i32;
    fn TranslateMessage(lp_msg: *const MSG) -> i32;
    fn DispatchMessageA(lp_msg: *const MSG) -> isize;

    fn RegisterRawInputDevices(
        p_raw_input_devices: *const RAWINPUTDEVICE,
        ui_num_devices: u32,
        cb_size: u32,
    ) -> i32;
    fn GetRawInputData(
        h_raw_input: isize,
        ui_command: u32,
        p_data: *mut c_void,
        pcb_size: *mut u32,
        cb_size_header: u32,
    ) -> u32;

    fn SetTimer(
        h_wnd: *mut c_void,
        n_id_event: usize,
        u_elapse: u32,
        lp_timer_func: *const c_void,
    ) -> usize;
    fn KillTimer(h_wnd: *mut c_void, u_id_event: usize) -> i32;

    fn QueryPerformanceFrequency(lp_frequency: *mut u64) -> i32;
    fn QueryPerformanceCounter(lp_performance_count: *mut u64) -> i32;

    fn SetWindowsHookExA(
        id_hook: i32,
        lpfn: *const c_void,
        h_mod: *mut c_void,
        dw_thread_id: u32,
    ) -> *mut c_void;
    fn UnhookWindowsHookEx(h_hook: *mut c_void) -> i32;
    fn CallNextHookEx(h_hook: *mut c_void, n_code: i32, w_param: usize, l_param: isize) -> isize;

    fn SendInput(c_inputs: u32, p_inputs: *const INPUT, cb_size: i32) -> u32;

    fn GetForegroundWindow() -> *mut c_void;
}

fn cstr(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}

pub(crate) fn spawn() -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {
        let _ = run();
    })
}

pub(crate) fn tray_hwnd() -> *mut c_void {
    HWND_ATOM.load(Ordering::Relaxed)
}

pub(crate) fn tray_set_ui(ui: UiHandles) {
    let mut state = STATE.lock().unwrap();
    state.ui = Some(ui);
}

pub(crate) fn request_exit() {
    engine::request_exit();
    let hwnd = HWND_ATOM.load(Ordering::Relaxed);
    if !hwnd.is_null() {
        unsafe {
            let _ = PostMessageA(hwnd, WM_CLOSE, 0, 0);
        }
    }
}

fn send_mouse_wheel(vertical: bool, delta: i32) -> Result<(), Box<dyn Error>> {
    if delta == 0 {
        return Ok(());
    }

    let flags = if vertical {
        MOUSEEVENTF_WHEEL
    } else {
        MOUSEEVENTF_HWHEEL
    };

    let input = INPUT {
        type_: INPUT_MOUSE,
        data: INPUT_DATA {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouse_data: delta as u32,
                dw_flags: flags,
                time: 0,
                dw_extra_info: MAGIC_WORD.as_ptr() as usize,
            },
        },
    };

    unsafe {
        let sent = SendInput(1, &input, mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            return Err(Box::new(std::io::Error::last_os_error()));
        }
    }

    Ok(())
}

fn flush_axis(axis: &mut MomentumAxis, vertical: bool) -> Result<(), Box<dyn Error>> {
    let (hires, _detents) = axis.drain();
    if hires != 0 {
        send_mouse_wheel(vertical, hires)?;
    }
    Ok(())
}

fn flush_immediate_axis(axis: &mut ImmediateAxis, vertical: bool) -> Result<(), Box<dyn Error>> {
    let (hires, _detents) = axis.drain();
    if hires != 0 {
        send_mouse_wheel(vertical, hires)?;
    }
    Ok(())
}

fn abort_scroll_due_to_context_change(state: &mut WinState, hwnd: *mut c_void) {
    state.clear_all_scroll_state();

    if state.timer_active {
        unsafe {
            KillTimer(hwnd, SCROLL_TIMER_ID);
        }
        state.timer_active = false;
    }
}

fn sync_foreground_window(hwnd: *mut c_void) {
    unsafe {
        let fg = GetForegroundWindow();
        let mut state = STATE.lock().unwrap();
        if fg != state.foreground_hwnd {
            state.foreground_hwnd = fg;
            abort_scroll_due_to_context_change(&mut state, hwnd);
        }
    }
}

fn start_timer(hwnd: *mut c_void) {
    unsafe {
        let mut state = STATE.lock().unwrap();
        if !state.timer_active {
            let mut now = 0u64;
            QueryPerformanceCounter(&mut now);
            state.last_counter = now;
            SetTimer(hwnd, SCROLL_TIMER_ID, TIMER_INTERVAL_MS, ptr::null());
            state.timer_active = true;
        }
    }
}

fn stop_timer(hwnd: *mut c_void) {
    unsafe {
        let mut state = STATE.lock().unwrap();
        if state.timer_active {
            KillTimer(hwnd, SCROLL_TIMER_ID);
            state.timer_active = false;
        }
    }
}

fn process_timer(hwnd: *mut c_void) {
    unsafe {
        sync_foreground_window(hwnd);

        let mut state = STATE.lock().unwrap();
        if state.perf_freq == 0 {
            return;
        }

        let cfg = engine::config();
        let mut now = 0u64;
        QueryPerformanceCounter(&mut now);

        let dt_ticks = now.saturating_sub(state.last_counter);
        state.last_counter = now;

        let dt = Duration::from_secs_f64(dt_ticks as f64 / state.perf_freq as f64);

        if state.modifiers.win_down || !cfg.smooth_enabled() {
            state.clear_scroll();
            if state.timer_active {
                KillTimer(hwnd, SCROLL_TIMER_ID);
                state.timer_active = false;
            }
            return;
        }

        state.normal_wheel_v.tick(cfg.normal_wheel_damping(), dt);
        state.normal_wheel_h.tick(cfg.normal_wheel_damping(), dt);
        state.drag_wheel_v.tick(cfg.drag_wheel_damping(), dt);
        state.drag_wheel_h.tick(cfg.drag_wheel_damping(), dt);

        let _ = flush_axis(&mut state.normal_wheel_v, true);
        let _ = flush_axis(&mut state.normal_wheel_h, false);
        let _ = flush_axis(&mut state.drag_wheel_v, true);
        let _ = flush_axis(&mut state.drag_wheel_h, false);

        if state.is_idle() && state.timer_active {
            KillTimer(hwnd, SCROLL_TIMER_ID);
            state.timer_active = false;
        }
    }
}

fn handle_raw_input(hwnd: *mut c_void, l_param: isize) {
    unsafe {
        sync_foreground_window(hwnd);

        let mut size = 0u32;
        GetRawInputData(
            l_param,
            RID_INPUT,
            ptr::null_mut(),
            &mut size,
            mem::size_of::<RAWINPUTHEADER>() as u32,
        );

        if size == 0 {
            return;
        }

        let mut buffer = vec![0u8; size as usize];
        let ret = GetRawInputData(
            l_param,
            RID_INPUT,
            buffer.as_mut_ptr() as *mut c_void,
            &mut size,
            mem::size_of::<RAWINPUTHEADER>() as u32,
        );

        if ret == u32::MAX || size == 0 {
            return;
        }

        let raw = &*(buffer.as_ptr() as *const RAWINPUT);

        match raw.header.dw_type {
            RIM_TYPEMOUSE => {
                let mouse = raw.data.mouse;
                let cfg = engine::config();

                if mouse.us_button_flags & RI_MOUSE_WHEEL != 0 {
                    let delta = mouse.us_button_data as i16 as i32;
                    let mut state = STATE.lock().unwrap();

                    if state.modifiers.win_down || !cfg.smooth_enabled() {
                        state.immediate_drag_v.push_detents(
                            delta as f64 / 120.0,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                        let _ = flush_immediate_axis(&mut state.immediate_drag_v, true);
                    } else {
                        state.normal_wheel_v.push_detents(
                            delta as f64 / 120.0,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                        if !state.timer_active {
                            drop(state);
                            start_timer(hwnd);
                        }
                    }
                    return;
                }

                if mouse.us_button_flags & RI_MOUSE_HWHEEL != 0 {
                    let delta = mouse.us_button_data as i16 as i32;
                    let mut state = STATE.lock().unwrap();

                    if state.modifiers.win_down || !cfg.smooth_enabled() {
                        state.immediate_drag_h.push_detents(
                            delta as f64 / 120.0,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                        let _ = flush_immediate_axis(&mut state.immediate_drag_h, false);
                    } else {
                        state.normal_wheel_h.push_detents(
                            delta as f64 / 120.0,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                        if !state.timer_active {
                            drop(state);
                            start_timer(hwnd);
                        }
                    }
                    return;
                }

                if mouse.us_button_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN != 0 {
                    let mut state = STATE.lock().unwrap();
                    state.middle.begin();
                    return;
                }

                if mouse.us_button_flags & RI_MOUSE_MIDDLE_BUTTON_UP != 0 {
                    let mut state = STATE.lock().unwrap();
                    state.middle.clear();
                    return;
                }

                if mouse.us_button_flags == 0 {
                    let mut need_timer = false;
                    let mut do_immediate = false;

                    {
                        let mut state = STATE.lock().unwrap();

                        if state.middle.pressed_at.is_some() {
                            state.middle.push_motion(
                                mouse.l_last_x,
                                mouse.l_last_y,
                                cfg.drag_deadzone_px(),
                            );

                            if state.middle.is_scroll_mode(cfg.tap_max_duration_ms()) {
                                if cfg.smooth_enabled() && !state.modifiers.win_down {
                                    state.drag_wheel_h.push_detents(
                                        -(mouse.l_last_x as f64),
                                        cfg.drag_wheel_gain(),
                                        cfg.max_velocity_hires(),
                                    );
                                    state.drag_wheel_v.push_detents(
                                        -(mouse.l_last_y as f64),
                                        cfg.drag_wheel_gain(),
                                        cfg.max_velocity_hires(),
                                    );
                                    need_timer = true;
                                } else {
                                    state.immediate_drag_h.push_detents(
                                        -(mouse.l_last_x as f64),
                                        cfg.drag_wheel_gain(),
                                        cfg.max_velocity_hires(),
                                    );
                                    state.immediate_drag_v.push_detents(
                                        -(mouse.l_last_y as f64),
                                        cfg.drag_wheel_gain(),
                                        cfg.max_velocity_hires(),
                                    );
                                    do_immediate = true;
                                }
                            }
                        }
                    }

                    if do_immediate {
                        let mut state = STATE.lock().unwrap();
                        let _ = flush_immediate_axis(&mut state.immediate_drag_h, false);
                        let _ = flush_immediate_axis(&mut state.immediate_drag_v, true);
                    } else if need_timer {
                        start_timer(hwnd);
                    }
                }
            }
            RIM_TYPEKEYBOARD => {
                let kb = raw.data.keyboard;
                let is_up = (kb.flags & 0x01) != 0;
                let v_key = kb.v_key;

                if v_key == VK_LWIN || v_key == VK_RWIN {
                    let mut stop_now = false;
                    {
                        let mut state = STATE.lock().unwrap();
                        let new_state = !is_up;

                        if !state.modifiers.win_down && new_state {
                            state.clear_all_scroll_state();
                            stop_now = state.timer_active;
                        }

                        state.modifiers.win_down = new_state;
                    }

                    if stop_now {
                        stop_timer(hwnd);
                    }
                }
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, w_param: usize, l_param: isize) -> isize {
    if code == HC_ACTION {
        let inf = &*(l_param as *const MSLLHOOKSTRUCT);
        let injected =
            (inf.flags & LLMHF_INJECTED) != 0 || (inf.flags & LLMHF_LOWER_IL_INJECTED) != 0;
        let ours = inf.dw_extra_info == MAGIC_WORD.as_ptr() as usize;

        if !ours {
            match w_param {
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                    return 1;
                }
                _ => {}
            }
        }

        if injected && ours {
            return CallNextHookEx(ptr::null_mut(), code, w_param, l_param);
        }
    }

    CallNextHookEx(ptr::null_mut(), code, w_param, l_param)
}

unsafe extern "system" fn wnd_proc(
    hwnd: *mut c_void,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    match msg {
        WM_INPUT => {
            handle_raw_input(hwnd, l_param);
            0
        }
        WM_TIMER => {
            if w_param == SCROLL_TIMER_ID {
                process_timer(hwnd);
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcA(hwnd, msg, w_param, l_param),
    }
}

struct Cleanup {
    hwnd: *mut c_void,
    hook: *mut c_void,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        unsafe {
            if !self.hook.is_null() {
                UnhookWindowsHookEx(self.hook);
            }
            if !self.hwnd.is_null() {
                KillTimer(self.hwnd, SCROLL_TIMER_ID);
                DestroyWindow(self.hwnd);
            }
        }
    }
}

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    unsafe {
        let h_instance = GetModuleHandleA(ptr::null());
        if h_instance.is_null() {
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        let class_name = cstr("NimbusScrollHiddenWindow");

        let wc = WNDCLASSA {
            style: 0,
            lpfn_wnd_proc: Some(wnd_proc),
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
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        let hwnd = CreateWindowExA(
            0,
            class_name.as_ptr(),
            ptr::null(),
            0x80000000,
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
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        HWND_ATOM.store(hwnd, Ordering::Relaxed);

        {
            let mut state = STATE.lock().unwrap();
            state.hwnd = hwnd;
            state.foreground_hwnd = GetForegroundWindow();
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

        if RegisterRawInputDevices(
            devices.as_ptr(),
            devices.len() as u32,
            mem::size_of::<RAWINPUTDEVICE>() as u32,
        ) == 0
        {
            DestroyWindow(hwnd);
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        let hook = SetWindowsHookExA(WH_MOUSE_LL, hook_proc as *const c_void, ptr::null_mut(), 0);
        if hook.is_null() {
            DestroyWindow(hwnd);
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        let mut freq = 0u64;
        if QueryPerformanceFrequency(&mut freq) == 0 || freq == 0 {
            UnhookWindowsHookEx(hook);
            DestroyWindow(hwnd);
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        {
            let mut state = STATE.lock().unwrap();
            state.perf_freq = freq;
        }

        let _cleanup = Cleanup { hwnd, hook };

        let mut msg: MSG = mem::zeroed();
        loop {
            if engine::should_exit() {
                break;
            }

            let r = GetMessageA(&mut msg, ptr::null_mut(), 0, 0);
            if r == -1 {
                break;
            }
            if r == 0 {
                break;
            }

            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
    }

    Ok(())
}
