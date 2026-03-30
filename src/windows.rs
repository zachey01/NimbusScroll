use crate::app::Backend;
use crate::engine::{InputEvent, MouseDeviceInfo, OutputEvent, ScrollAxis, ScrollKey};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};

use windows::Win32::System::Threading::GetCurrentThreadId;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN,
    MOUSEEVENTF_XUP, MOUSEINPUT,
};

use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};

pub(crate) struct WindowsBackend;

const XBUTTON1_DATA: u32 = 1;
const XBUTTON2_DATA: u32 = 2;

#[derive(Default)]
pub struct WindowsMouseHandle;

#[derive(Default)]
pub struct WindowsKeyboardHandle;

#[derive(Default)]
pub struct WindowsOutputHandle;

pub struct WindowsInputState {
    mouse_events: Mutex<VecDeque<InputEvent>>,
    keyboard_events: Mutex<VecDeque<InputEvent>>,
    last_cursor: Mutex<Option<(i32, i32)>>,
}

impl WindowsInputState {
    fn new() -> Self {
        Self {
            mouse_events: Mutex::new(VecDeque::new()),
            keyboard_events: Mutex::new(VecDeque::new()),
            last_cursor: Mutex::new(None),
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
static HOOK_THREAD_ID: OnceLock<u32> = OnceLock::new();
static HOOK_THREAD_STARTED: OnceLock<()> = OnceLock::new();

fn state() -> Arc<WindowsInputState> {
    STATE
        .get_or_init(|| Arc::new(WindowsInputState::new()))
        .clone()
}

pub(crate) fn request_exit() {
    if let Some(id) = HOOK_THREAD_ID.get().copied() {
        unsafe {
            let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

fn ensure_hooks_started() {
    let _ = HOOK_THREAD_STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("nimbusscroll-win-hooks".into())
            .spawn(move || unsafe {
                hook_thread_main();
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
    ensure_hooks_started();
    Ok(WindowsMouseHandle)
}

fn open_keyboard_devices_impl() -> Result<Vec<WindowsKeyboardHandle>, Box<dyn Error>> {
    ensure_hooks_started();
    Ok(vec![WindowsKeyboardHandle])
}

fn build_virtual_mouse_impl() -> Result<WindowsOutputHandle, Box<dyn Error>> {
    ensure_hooks_started();
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
        if hwnd.0 == 0 {
            None
        } else {
            Some(format!("{:?}", hwnd))
        }
    }
}

fn send_mouse_input(
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
    data: u32,
    dx: i32,
    dy: i32,
) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
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
            ScrollAxis::X => send_mouse_input(MOUSEEVENTF_MOVE, 0, value, 0),
            ScrollAxis::Y => send_mouse_input(MOUSEEVENTF_MOVE, 0, 0, value),
            ScrollAxis::Wheel => send_mouse_input(MOUSEEVENTF_WHEEL, value as u32, 0, 0),
            ScrollAxis::WheelHiRes => send_mouse_input(MOUSEEVENTF_WHEEL, value as u32, 0, 0),
            ScrollAxis::HWheel => send_mouse_input(MOUSEEVENTF_HWHEEL, value as u32, 0, 0),
            ScrollAxis::HWheelHiRes => send_mouse_input(MOUSEEVENTF_HWHEEL, value as u32, 0, 0),
            ScrollAxis::Other(_) => {}
        },
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = *(lparam.0 as *const MSLLHOOKSTRUCT);

        if (info.flags & LLMHF_INJECTED) != 0 {
            return CallNextHookEx(HHOOK(0), code, wparam, lparam);
        }

        let st = state();
        let msg = wparam.0 as u32;

        match msg {
            WM_MOUSEMOVE => {
                let pt = info.pt;
                let mut guard = st.last_cursor.lock().ok();
                if let Some(ref mut last) = guard {
                    if let Some((x, y)) = **last {
                        let dx = pt.x - x;
                        let dy = pt.y - y;
                        if dx != 0 {
                            st.push_mouse(InputEvent::Rel {
                                axis: ScrollAxis::X,
                                value: dx,
                            });
                        }
                        if dy != 0 {
                            st.push_mouse(InputEvent::Rel {
                                axis: ScrollAxis::Y,
                                value: dy,
                            });
                        }
                    }
                    **last = Some((pt.x, pt.y));
                }
                return LRESULT(1);
            }

            WM_MOUSEWHEEL => {
                let delta = ((info.mouseData >> 16) as u16) as i16 as i32;
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::WheelHiRes,
                    value: delta,
                });
                return LRESULT(1);
            }

            WM_MOUSEHWHEEL => {
                let delta = ((info.mouseData >> 16) as u16) as i16 as i32;
                st.push_mouse(InputEvent::Rel {
                    axis: ScrollAxis::HWheelHiRes,
                    value: delta,
                });
                return LRESULT(1);
            }

            WM_LBUTTONDOWN => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Left,
                    value: 1,
                });
                return LRESULT(1);
            }
            WM_LBUTTONUP => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Left,
                    value: 0,
                });
                return LRESULT(1);
            }
            WM_RBUTTONDOWN => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Right,
                    value: 1,
                });
                return LRESULT(1);
            }
            WM_RBUTTONUP => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Right,
                    value: 0,
                });
                return LRESULT(1);
            }
            WM_MBUTTONDOWN => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Middle,
                    value: 1,
                });
                return LRESULT(1);
            }
            WM_MBUTTONUP => {
                st.push_mouse(InputEvent::Key {
                    key: ScrollKey::Middle,
                    value: 0,
                });
                return LRESULT(1);
            }
            WM_XBUTTONDOWN => {
                let xbtn = ((info.mouseData >> 16) & 0xffff) as u16;
                let key = if xbtn == XBUTTON2_DATA as u16 {
                    ScrollKey::Forward
                } else {
                    ScrollKey::Back
                };
                st.push_mouse(InputEvent::Key { key, value: 1 });
                return LRESULT(1);
            }
            WM_XBUTTONUP => {
                let xbtn = ((info.mouseData >> 16) & 0xffff) as u16;
                let key = if xbtn == XBUTTON2_DATA as u16 {
                    ScrollKey::Forward
                } else {
                    ScrollKey::Back
                };
                st.push_mouse(InputEvent::Key { key, value: 0 });
                return LRESULT(1);
            }
            _ => {}
        }
    }

    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = *(lparam.0 as *const KBDLLHOOKSTRUCT);

        if (info.flags.0 & LLKHF_INJECTED.0) != 0 {
            return CallNextHookEx(HHOOK(0), code, wparam, lparam);
        }

        let st = state();
        let msg = wparam.0 as u32;

        let key = match info.vkCode as u32 {
            0x5B => Some(ScrollKey::LeftMeta),
            0x5C => Some(ScrollKey::RightMeta),
            _ => None,
        };

        if let Some(key) = key {
            match msg {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    st.push_keyboard(InputEvent::Key { key, value: 1 });
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    st.push_keyboard(InputEvent::Key { key, value: 0 });
                }
                _ => {}
            }
        }
    }

    CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

unsafe fn hook_thread_main() {
    let thread_id = GetCurrentThreadId();
    let _ = HOOK_THREAD_ID.set(thread_id);

    let mouse_hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) {
        Ok(h) => h,
        Err(_) => return,
    };

    let keyboard_hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) {
        Ok(h) => h,
        Err(_) => {
            let _ = UnhookWindowsHookEx(mouse_hook);
            return;
        }
    };

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).into() {
        if msg.message == WM_QUIT {
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    let _ = UnhookWindowsHookEx(mouse_hook);
    let _ = UnhookWindowsHookEx(keyboard_hook);
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
