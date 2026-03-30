use crate::app::Backend;
use crate::engine::{InputEvent, MouseDeviceInfo, OutputEvent, ScrollAxis, ScrollKey};
use evdev::uinput::VirtualDevice;
use evdev::{
    AttributeSet, Device, EventSummary, InputEvent as EvdevInputEvent, KeyCode, PropType,
    RelativeAxisCode,
};
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::process::Command;
use std::time::Duration;

const PROC_INPUT_DEVICES: &str = "/proc/bus/input/devices";
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;

#[derive(Debug)]
struct NimbusScrollError(String);

impl std::fmt::Display for NimbusScrollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for NimbusScrollError {}

pub(crate) struct WaylandBackend;

fn boxed_io_error_with_hint(err: std::io::Error, hint: &str) -> Box<dyn Error> {
    if err.kind() == ErrorKind::PermissionDenied {
        Box::new(NimbusScrollError(format!("{hint}: {err}")))
    } else {
        Box::new(err)
    }
}

fn map_key_to_input(key: KeyCode) -> ScrollKey {
    match key {
        KeyCode::KEY_LEFTMETA => ScrollKey::LeftMeta,
        KeyCode::KEY_RIGHTMETA => ScrollKey::RightMeta,
        KeyCode::BTN_MIDDLE => ScrollKey::Middle,
        KeyCode::BTN_LEFT => ScrollKey::Left,
        KeyCode::BTN_RIGHT => ScrollKey::Right,
        KeyCode::BTN_SIDE => ScrollKey::Side,
        KeyCode::BTN_EXTRA => ScrollKey::Extra,
        KeyCode::BTN_FORWARD => ScrollKey::Forward,
        KeyCode::BTN_BACK => ScrollKey::Back,
        KeyCode::BTN_TASK => ScrollKey::Task,
        _ => ScrollKey::Other(key.0),
    }
}

fn map_rel_to_input(axis: RelativeAxisCode) -> ScrollAxis {
    match axis {
        RelativeAxisCode::REL_X => ScrollAxis::X,
        RelativeAxisCode::REL_Y => ScrollAxis::Y,
        RelativeAxisCode::REL_WHEEL => ScrollAxis::Wheel,
        RelativeAxisCode::REL_WHEEL_HI_RES => ScrollAxis::WheelHiRes,
        RelativeAxisCode::REL_HWHEEL => ScrollAxis::HWheel,
        RelativeAxisCode::REL_HWHEEL_HI_RES => ScrollAxis::HWheelHiRes,
        _ => ScrollAxis::Other(axis.0),
    }
}

fn map_output_key(key: ScrollKey) -> Option<KeyCode> {
    Some(match key {
        ScrollKey::LeftMeta => KeyCode::KEY_LEFTMETA,
        ScrollKey::RightMeta => KeyCode::KEY_RIGHTMETA,
        ScrollKey::Middle => KeyCode::BTN_MIDDLE,
        ScrollKey::Left => KeyCode::BTN_LEFT,
        ScrollKey::Right => KeyCode::BTN_RIGHT,
        ScrollKey::Side => KeyCode::BTN_SIDE,
        ScrollKey::Extra => KeyCode::BTN_EXTRA,
        ScrollKey::Forward => KeyCode::BTN_FORWARD,
        ScrollKey::Back => KeyCode::BTN_BACK,
        ScrollKey::Task => KeyCode::BTN_TASK,
        ScrollKey::Other(_) => return None,
    })
}

fn map_output_rel(axis: ScrollAxis) -> Option<RelativeAxisCode> {
    Some(match axis {
        ScrollAxis::X => RelativeAxisCode::REL_X,
        ScrollAxis::Y => RelativeAxisCode::REL_Y,
        ScrollAxis::Wheel => RelativeAxisCode::REL_WHEEL,
        ScrollAxis::WheelHiRes => RelativeAxisCode::REL_WHEEL_HI_RES,
        ScrollAxis::HWheel => RelativeAxisCode::REL_HWHEEL,
        ScrollAxis::HWheelHiRes => RelativeAxisCode::REL_HWHEEL_HI_RES,
        ScrollAxis::Other(_) => return None,
    })
}

fn map_event(ev: EvdevInputEvent) -> Option<InputEvent> {
    match ev.destructure() {
        EventSummary::Key(_, key, value) => Some(InputEvent::Key {
            key: map_key_to_input(key),
            value,
        }),
        EventSummary::RelativeAxis(_, axis, value) => Some(InputEvent::Rel {
            axis: map_rel_to_input(axis),
            value,
        }),
        _ => None,
    }
}

fn list_mouse_devices_impl() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>> {
    let content = fs::read_to_string(PROC_INPUT_DEVICES).map_err(|e| {
        boxed_io_error_with_hint(
            e,
            "не удалось прочитать /proc/bus/input/devices для списка мышей",
        )
    })?;

    let mut devices = Vec::new();

    for block in content.split("\n\n") {
        let mut handlers: Option<Vec<String>> = None;
        let mut name: Option<String> = None;

        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("N: Name=") {
                name = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                handlers = Some(rest.split_whitespace().map(|s| s.to_string()).collect());
            }
        }

        let Some(handlers) = handlers else {
            continue;
        };

        let has_mouse_handler = handlers.iter().any(|h| h.starts_with("mouse"));
        let event_node = handlers.iter().find(|h| h.starts_with("event")).cloned();

        if has_mouse_handler {
            if let Some(event_node) = event_node {
                let path = format!("/dev/input/{event_node}");
                let label = match name {
                    Some(name) if !name.is_empty() => format!("{name} ({event_node})"),
                    _ => event_node,
                };

                devices.push(MouseDeviceInfo { label, path });
            }
        }
    }

    Ok(devices)
}

fn default_mouse_path_impl() -> Result<Option<String>, Box<dyn Error>> {
    Ok(list_mouse_devices_impl()?.first().map(|d| d.path.clone()))
}

fn open_mouse_device_impl(path: Option<&str>) -> Result<Device, Box<dyn Error>> {
    if let Some(path) = path {
        return Device::open(path).map_err(|e| {
            boxed_io_error_with_hint(
                e,
                &format!(
                    "не удалось открыть {path} — если это Permission denied, дело в правах на /dev/input/event*"
                ),
            )
        });
    }

    let guessed = guess_mouse_event_from_proc()?;
    Device::open(&guessed).map_err(|e| {
        boxed_io_error_with_hint(
            e,
            &format!(
                "не удалось открыть {guessed} — если это Permission denied, дело в правах на /dev/input/event*"
            ),
        )
    })
}

fn open_keyboard_devices_impl() -> Result<Vec<Device>, Box<dyn Error>> {
    let paths = guess_keyboard_event_from_proc()?;
    let mut devices = Vec::new();

    for path in paths {
        if let Ok(dev) = Device::open(&path) {
            if is_keyboard_like(&dev) {
                devices.push(dev);
            }
        }
    }

    Ok(devices)
}

fn guess_mouse_event_from_proc() -> Result<String, Box<dyn Error>> {
    let content = fs::read_to_string(PROC_INPUT_DEVICES).map_err(|e| {
        boxed_io_error_with_hint(
            e,
            "не удалось прочитать /proc/bus/input/devices; используй настройки для выбора мыши",
        )
    })?;

    for block in content.split("\n\n") {
        let mut handlers: Option<Vec<String>> = None;

        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("H: Handlers=") {
                handlers = Some(rest.split_whitespace().map(|s| s.to_string()).collect());
                break;
            }
        }

        if let Some(handlers) = handlers {
            let has_mouse_handler = handlers.iter().any(|h| h.starts_with("mouse"));
            let event_node = handlers.iter().find(|h| h.starts_with("event")).cloned();

            if has_mouse_handler {
                if let Some(event_node) = event_node {
                    return Ok(format!("/dev/input/{event_node}"));
                }
            }
        }
    }

    Err(Box::new(NimbusScrollError(
        "не удалось автоматически найти mouse event-устройство".to_string(),
    )))
}

fn guess_keyboard_event_from_proc() -> Result<Vec<String>, Box<dyn Error>> {
    let content = fs::read_to_string(PROC_INPUT_DEVICES).map_err(|e| {
        boxed_io_error_with_hint(
            e,
            "не удалось прочитать /proc/bus/input/devices для поиска клавиатур",
        )
    })?;

    let mut paths = Vec::new();

    for block in content.split("\n\n") {
        let mut handlers: Option<Vec<String>> = None;

        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("H: Handlers=") {
                handlers = Some(rest.split_whitespace().map(|s| s.to_string()).collect());
                break;
            }
        }

        if let Some(handlers) = handlers {
            let has_kbd_handler = handlers.iter().any(|h| h == "kbd" || h == "keyboard");
            let event_node = handlers.iter().find(|h| h.starts_with("event")).cloned();

            if has_kbd_handler {
                if let Some(event_node) = event_node {
                    paths.push(format!("/dev/input/{event_node}"));
                }
            }
        }
    }

    Ok(paths)
}

fn is_keyboard_like(dev: &Device) -> bool {
    dev.supported_keys().map_or(false, |keys| {
        keys.contains(KeyCode::KEY_LEFTMETA) || keys.contains(KeyCode::KEY_RIGHTMETA)
    })
}

fn build_virtual_mouse_impl() -> Result<VirtualDevice, Box<dyn Error>> {
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_LEFT);
    keys.insert(KeyCode::BTN_RIGHT);
    keys.insert(KeyCode::BTN_MIDDLE);
    keys.insert(KeyCode::BTN_SIDE);
    keys.insert(KeyCode::BTN_EXTRA);
    keys.insert(KeyCode::BTN_FORWARD);
    keys.insert(KeyCode::BTN_BACK);
    keys.insert(KeyCode::BTN_TASK);

    let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
    rel_axes.insert(RelativeAxisCode::REL_X);
    rel_axes.insert(RelativeAxisCode::REL_Y);
    rel_axes.insert(RelativeAxisCode::REL_WHEEL);
    rel_axes.insert(RelativeAxisCode::REL_HWHEEL);
    rel_axes.insert(RelativeAxisCode::REL_WHEEL_HI_RES);
    rel_axes.insert(RelativeAxisCode::REL_HWHEEL_HI_RES);

    let mut props = AttributeSet::<PropType>::new();
    props.insert(PropType::POINTER);

    let dev = VirtualDevice::builder()
        .map_err(|e| boxed_io_error_with_hint(e, "не удалось создать uinput builder"))?
        .name("NimbusScroll Wayland")
        .with_properties(&props)
        .map_err(|e| {
            boxed_io_error_with_hint(e, "не удалось задать properties для виртуальной мыши")
        })?
        .with_keys(&keys)
        .map_err(|e| boxed_io_error_with_hint(e, "не удалось задать keys для виртуальной мыши"))?
        .with_relative_axes(&rel_axes)
        .map_err(|e| {
            boxed_io_error_with_hint(e, "не удалось задать relative axes для виртуальной мыши")
        })?
        .build()
        .map_err(|e| {
            boxed_io_error_with_hint(
                e,
                "не удалось создать virtual uinput device; проверь права на /dev/uinput",
            )
        })?;

    Ok(dev)
}

fn reset_virtual_mouse_buttons_impl(out: &mut VirtualDevice) -> Result<(), Box<dyn Error>> {
    for &button in &[
        KeyCode::BTN_LEFT,
        KeyCode::BTN_RIGHT,
        KeyCode::BTN_MIDDLE,
        KeyCode::BTN_SIDE,
        KeyCode::BTN_EXTRA,
        KeyCode::BTN_FORWARD,
        KeyCode::BTN_BACK,
        KeyCode::BTN_TASK,
    ] {
        emit_key(out, button, 0)?;
    }

    Ok(())
}

fn emit_key(out: &mut VirtualDevice, key: KeyCode, value: i32) -> Result<(), Box<dyn Error>> {
    let ev = EvdevInputEvent::new(EV_KEY, key.0, value);
    out.emit(&[ev])?;
    Ok(())
}

fn emit_rel(
    out: &mut VirtualDevice,
    axis: RelativeAxisCode,
    value: i32,
) -> Result<(), Box<dyn Error>> {
    if value == 0 {
        return Ok(());
    }

    let ev = EvdevInputEvent::new(EV_REL, axis.0, value);
    out.emit(&[ev])?;
    Ok(())
}

fn emit_output_impl(out: &mut VirtualDevice, event: OutputEvent) -> Result<(), Box<dyn Error>> {
    match event {
        OutputEvent::Key { key, value } => {
            if let Some(code) = map_output_key(key) {
                emit_key(out, code, value)?;
            }
        }
        OutputEvent::Rel { axis, value } => {
            if let Some(code) = map_output_rel(axis) {
                emit_rel(out, code, value)?;
            }
        }
    }

    Ok(())
}

fn poll_device_events(dev: &mut Device) -> Result<Vec<InputEvent>, Box<dyn Error>> {
    let mut out = Vec::new();

    match dev.fetch_events() {
        Ok(events) => {
            for ev in events {
                if let Some(mapped) = map_event(ev) {
                    out.push(mapped);
                }
            }
            Ok(out)
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(out),
        Err(e) => Err(Box::new(e)),
    }
}

fn active_window_signature_impl() -> Option<String> {
    if let Ok(cmd) = std::env::var("NIMBUS_SCROLL_FOCUS_CMD") {
        let sig = run_shell_command(&cmd);
        if sig.is_some() {
            return sig;
        }
    }

    run_command_capture("hyprctl", &["activewindow"])
        .or_else(|| run_command_capture("xdotool", &["getwindowfocus"]))
        .or_else(|| run_command_capture("xprop", &["-root", "_NET_ACTIVE_WINDOW"]))
}

fn run_shell_command(cmd: &str) -> Option<String> {
    let output = Command::new("sh").arg("-lc").arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn run_command_capture(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

impl Backend for WaylandBackend {
    type Mouse = Device;
    type Keyboard = Device;
    type Output = VirtualDevice;

    fn list_mouse_devices() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>> {
        list_mouse_devices_impl()
    }

    fn default_mouse_path() -> Result<Option<String>, Box<dyn Error>> {
        default_mouse_path_impl()
    }

    fn open_mouse_device(path: Option<&str>) -> Result<Self::Mouse, Box<dyn Error>> {
        open_mouse_device_impl(path)
    }

    fn set_mouse_nonblocking(mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>> {
        mouse.set_nonblocking(true)?;
        Ok(())
    }

    fn grab_mouse(mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>> {
        mouse.grab()?;
        Ok(())
    }

    fn open_keyboard_devices() -> Result<Vec<Self::Keyboard>, Box<dyn Error>> {
        open_keyboard_devices_impl()
    }

    fn set_keyboard_nonblocking(kb: &mut Self::Keyboard) -> Result<(), Box<dyn Error>> {
        kb.set_nonblocking(true)?;
        Ok(())
    }

    fn poll_mouse_events(mouse: &mut Self::Mouse) -> Result<Vec<InputEvent>, Box<dyn Error>> {
        poll_device_events(mouse)
    }

    fn poll_keyboard_events(kb: &mut Self::Keyboard) -> Result<Vec<InputEvent>, Box<dyn Error>> {
        poll_device_events(kb)
    }

    fn build_virtual_mouse() -> Result<Self::Output, Box<dyn Error>> {
        build_virtual_mouse_impl()
    }

    fn reset_virtual_mouse_buttons(out: &mut Self::Output) -> Result<(), Box<dyn Error>> {
        reset_virtual_mouse_buttons_impl(out)
    }

    fn emit_output(out: &mut Self::Output, event: OutputEvent) -> Result<(), Box<dyn Error>> {
        emit_output_impl(out, event)
    }

    fn active_window_signature() -> Option<String> {
        active_window_signature_impl()
    }

    fn sleep(duration: Duration) {
        std::thread::sleep(duration);
    }
}
