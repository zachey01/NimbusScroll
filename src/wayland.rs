use crate::engine::{self, ImmediateAxis, MiddleDragState, ModifierState, MomentumAxis};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventSummary, InputEvent, KeyCode, PropType, RelativeAxisCode};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::io::{self, Write};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

const PROC_INPUT_DEVICES: &str = "/proc/bus/input/devices";
const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;

#[derive(Debug)]
struct NimbusScrollError(String);

impl fmt::Display for NimbusScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for NimbusScrollError {}

#[derive(Clone, Debug)]
pub(crate) struct MouseDeviceInfo {
    pub(crate) label: String,
    pub(crate) path: String,
}

struct FocusTracker {
    last_signature: Option<String>,
    last_check: Instant,
    check_interval: Duration,
}

impl FocusTracker {
    fn new() -> Self {
        Self {
            last_signature: active_window_signature(),
            last_check: Instant::now(),
            check_interval: Duration::from_millis(200),
        }
    }

    fn changed(&mut self) -> bool {
        let now = Instant::now();

        if now.saturating_duration_since(self.last_check) < self.check_interval {
            return false;
        }

        self.last_check = now;

        let current = active_window_signature();
        if current != self.last_signature {
            self.last_signature = current;
            true
        } else {
            false
        }
    }
}

struct LinuxState {
    normal_wheel_v: MomentumAxis,
    normal_wheel_h: MomentumAxis,
    drag_wheel_v: MomentumAxis,
    drag_wheel_h: MomentumAxis,
    immediate_drag_v: ImmediateAxis,
    immediate_drag_h: ImmediateAxis,
    middle: MiddleDragState,
    modifiers: ModifierState,
    focus: FocusTracker,
}

impl LinuxState {
    fn new() -> Self {
        Self {
            normal_wheel_v: MomentumAxis::new(),
            normal_wheel_h: MomentumAxis::new(),
            drag_wheel_v: MomentumAxis::new(),
            drag_wheel_h: MomentumAxis::new(),
            immediate_drag_v: ImmediateAxis::new(),
            immediate_drag_h: ImmediateAxis::new(),
            middle: MiddleDragState::new(),
            modifiers: ModifierState::new(),
            focus: FocusTracker::new(),
        }
    }

    fn clear_scroll_state(&mut self) {
        self.normal_wheel_v.clear();
        self.normal_wheel_h.clear();
        self.drag_wheel_v.clear();
        self.drag_wheel_h.clear();
        self.immediate_drag_v.clear();
        self.immediate_drag_h.clear();
        self.middle.clear();
    }
}

pub(crate) fn spawn() -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {
        if let Err(e) = run() {
            eprintln!("{e}");
        }
    })
}

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args
        .first()
        .map(|s| s == "--list" || s == "-l")
        .unwrap_or(false)
    {
        list_devices()?;
        return Ok(());
    }

    let debug_latency = debug_latency_enabled();

    let cfg = engine::config();
    let initial_mouse_path = cfg
        .mouse_device_path()
        .or_else(|| default_mouse_path().ok().flatten());

    let mut mouse = open_mouse_device(initial_mouse_path.as_deref())?;
    mouse.set_nonblocking(true)?;
    mouse.grab().map_err(|e| {
        boxed_io_error_with_hint(
            e,
            "не удалось захватить mouse input-устройство; для доступа к /dev/input/event* нужны права ОС",
        )
    })?;

    if cfg.mouse_device_path().is_none() {
        if let Some(path) = initial_mouse_path.clone() {
            cfg.set_mouse_device_path(Some(path));
        }
    }

    let mut keyboards = open_keyboard_devices()?;
    for kb in &mut keyboards {
        let _ = kb.set_nonblocking(true);
    }

    let mut virtual_mouse = build_virtual_mouse()?;
    reset_virtual_mouse_buttons(&mut virtual_mouse)?;

    let mut state = LinuxState::new();

    let mut last_tick = Instant::now();
    let mut current_mouse_path = initial_mouse_path;

    loop {
        if engine::should_exit() {
            break;
        }

        let loop_started = Instant::now();
        let mut saw_raw_input = false;

        if state.focus.changed() {
            state.clear_scroll_state();
        }

        if let Some(desired_path) = engine::config().mouse_device_path() {
            if current_mouse_path.as_deref() != Some(desired_path.as_str()) {
                match open_mouse_device(Some(desired_path.as_str())) {
                    Ok(mut new_mouse) => {
                        if let Err(e) = new_mouse.set_nonblocking(true) {
                            eprintln!("{e}");
                        } else if let Err(e) = new_mouse.grab() {
                            eprintln!(
                                "{}",
                                boxed_io_error_with_hint(
                                    e,
                                    "не удалось захватить новое mouse input-устройство",
                                )
                            );
                        } else {
                            mouse = new_mouse;
                            current_mouse_path = Some(desired_path);
                            state.clear_scroll_state();
                            reset_virtual_mouse_buttons(&mut virtual_mouse)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                    }
                }
            }
        }

        poll_keyboard_devices(
            &mut keyboards,
            &mut state.modifiers,
            &mut state.normal_wheel_v,
            &mut state.normal_wheel_h,
            &mut state.drag_wheel_v,
            &mut state.drag_wheel_h,
            &mut state.immediate_drag_v,
            &mut state.immediate_drag_h,
            &mut saw_raw_input,
        )?;

        match mouse.fetch_events() {
            Ok(events) => {
                for ev in events {
                    saw_raw_input = true;

                    match ev.destructure() {
                        EventSummary::Key(_, key, value) => {
                            handle_mouse_key_event(
                                key,
                                value,
                                &mut state.middle,
                                &mut virtual_mouse,
                            )?;
                        }
                        EventSummary::RelativeAxis(_, axis, value) => {
                            handle_mouse_relative_event(
                                axis,
                                value,
                                &mut state.middle,
                                &mut state.modifiers,
                                &mut state.normal_wheel_v,
                                &mut state.normal_wheel_h,
                                &mut state.drag_wheel_v,
                                &mut state.drag_wheel_h,
                                &mut state.immediate_drag_v,
                                &mut state.immediate_drag_h,
                                &mut virtual_mouse,
                            )?;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(Box::new(e)),
        }

        let now = Instant::now();
        let dt = now.saturating_duration_since(last_tick);
        last_tick = now;

        let emitted_smooth = advance_and_emit(
            &state.modifiers,
            dt,
            &mut state.normal_wheel_v,
            &mut state.normal_wheel_h,
            &mut state.drag_wheel_v,
            &mut state.drag_wheel_h,
            &mut virtual_mouse,
        )?;

        if debug_latency && saw_raw_input {
            let processing = Instant::now().saturating_duration_since(loop_started);
            eprint!(
                "\r[NimbusScroll debug] loop={:.3} ms, processing={:.3} ms   ",
                dt.as_secs_f64() * 1000.0,
                processing.as_secs_f64() * 1000.0
            );
            io::stderr().flush().ok();
        }

        let sleep_ms = if saw_raw_input {
            1
        } else if emitted_smooth {
            engine::config().loop_sleep_ms()
        } else {
            engine::config().loop_sleep_ms().max(1)
        };

        sleep(Duration::from_millis(sleep_ms));
    }

    Ok(())
}

pub(crate) fn list_mouse_devices() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>> {
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

pub(crate) fn default_mouse_path() -> Result<Option<String>, Box<dyn Error>> {
    Ok(list_mouse_devices()?.first().map(|d| d.path.clone()))
}

fn poll_keyboard_devices(
    keyboards: &mut [Device],
    modifiers: &mut ModifierState,
    normal_wheel_v: &mut MomentumAxis,
    normal_wheel_h: &mut MomentumAxis,
    drag_wheel_v: &mut MomentumAxis,
    drag_wheel_h: &mut MomentumAxis,
    immediate_drag_v: &mut ImmediateAxis,
    immediate_drag_h: &mut ImmediateAxis,
    saw_raw_input: &mut bool,
) -> Result<(), Box<dyn Error>> {
    for kb in keyboards.iter_mut() {
        match kb.fetch_events() {
            Ok(events) => {
                for ev in events {
                    *saw_raw_input = true;

                    if let EventSummary::Key(_, key, value) = ev.destructure() {
                        handle_modifier_key_event(
                            key,
                            value,
                            modifiers,
                            normal_wheel_v,
                            normal_wheel_h,
                            drag_wheel_v,
                            drag_wheel_h,
                            immediate_drag_v,
                            immediate_drag_h,
                        )?;
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(Box::new(e)),
        }
    }

    Ok(())
}

fn open_mouse_device(path: Option<&str>) -> Result<Device, Box<dyn Error>> {
    if let Some(path) = path {
        return Device::open(path).map_err(|e| {
            boxed_io_error_with_hint(
                e,
                &format!(
                    "не удалось открыть {} — если это Permission denied, дело в правах на /dev/input/event*",
                    path
                ),
            )
        });
    }

    let guessed = guess_mouse_event_from_proc()?;
    Device::open(&guessed).map_err(|e| {
        boxed_io_error_with_hint(
            e,
            &format!(
                "не удалось открыть {} — если это Permission denied, дело в правах на /dev/input/event*",
                guessed
            ),
        )
    })
}

fn open_keyboard_devices() -> Result<Vec<Device>, Box<dyn Error>> {
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
            "не удалось прочитать /proc/bus/input/devices; используй --list чтобы проверить, доступен ли proc",
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
                    return Ok(format!("/dev/input/{}", event_node));
                }
            }
        }
    }

    Err(Box::new(NimbusScrollError(
        "не удалось автоматически найти mouse event-устройство; запусти с --list и передай нужный /dev/input/eventX"
            .to_string(),
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
                    paths.push(format!("/dev/input/{}", event_node));
                }
            }
        }
    }

    Ok(paths)
}

fn is_mouse_like(dev: &Device) -> bool {
    let rel_ok = dev.supported_relative_axes().map_or(false, |axes| {
        axes.contains(RelativeAxisCode::REL_X) && axes.contains(RelativeAxisCode::REL_Y)
    });

    let key_ok = dev.supported_keys().map_or(false, |keys| {
        keys.contains(KeyCode::BTN_LEFT)
            || keys.contains(KeyCode::BTN_RIGHT)
            || keys.contains(KeyCode::BTN_MIDDLE)
            || keys.contains(KeyCode::BTN_SIDE)
            || keys.contains(KeyCode::BTN_EXTRA)
            || keys.contains(KeyCode::BTN_FORWARD)
            || keys.contains(KeyCode::BTN_BACK)
            || keys.contains(KeyCode::BTN_TASK)
    });

    rel_ok && key_ok
}

fn is_keyboard_like(dev: &Device) -> bool {
    dev.supported_keys().map_or(false, |keys| {
        keys.contains(KeyCode::KEY_LEFTMETA) || keys.contains(KeyCode::KEY_RIGHTMETA)
    })
}

fn list_devices() -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string(PROC_INPUT_DEVICES)
        .map_err(|e| boxed_io_error_with_hint(e, "не удалось прочитать /proc/bus/input/devices"))?;

    print!("{}", content);
    if !content.ends_with('\n') {
        println!();
    }

    Ok(())
}

fn build_virtual_mouse() -> Result<VirtualDevice, Box<dyn Error>> {
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

fn reset_virtual_mouse_buttons(out: &mut VirtualDevice) -> Result<(), Box<dyn Error>> {
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

fn handle_modifier_key_event(
    key: KeyCode,
    value: i32,
    modifiers: &mut ModifierState,
    normal_wheel_v: &mut MomentumAxis,
    normal_wheel_h: &mut MomentumAxis,
    drag_wheel_v: &mut MomentumAxis,
    drag_wheel_h: &mut MomentumAxis,
    immediate_drag_v: &mut ImmediateAxis,
    immediate_drag_h: &mut ImmediateAxis,
) -> Result<(), Box<dyn Error>> {
    match key {
        KeyCode::KEY_LEFTMETA | KeyCode::KEY_RIGHTMETA => {
            let new_state = value != 0;

            if !modifiers.win_down && new_state {
                normal_wheel_v.clear();
                normal_wheel_h.clear();
                drag_wheel_v.clear();
                drag_wheel_h.clear();
                immediate_drag_v.clear();
                immediate_drag_h.clear();
            }

            modifiers.win_down = new_state;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_mouse_key_event(
    key: KeyCode,
    value: i32,
    middle: &mut MiddleDragState,
    out: &mut VirtualDevice,
) -> Result<(), Box<dyn Error>> {
    match key {
        KeyCode::BTN_MIDDLE => {
            if value == 1 {
                middle.begin();
            } else if value == 0 {
                middle.clear();
            }

            emit_key(out, key, value)?;
            Ok(())
        }

        KeyCode::BTN_LEFT
        | KeyCode::BTN_RIGHT
        | KeyCode::BTN_SIDE
        | KeyCode::BTN_EXTRA
        | KeyCode::BTN_FORWARD
        | KeyCode::BTN_BACK
        | KeyCode::BTN_TASK => {
            emit_key(out, key, value)?;
            Ok(())
        }

        _ => {
            emit_key(out, key, value)?;
            Ok(())
        }
    }
}

fn handle_mouse_relative_event(
    axis: RelativeAxisCode,
    value: i32,
    middle: &mut MiddleDragState,
    modifiers: &mut ModifierState,
    normal_wheel_v: &mut MomentumAxis,
    normal_wheel_h: &mut MomentumAxis,
    drag_wheel_v: &mut MomentumAxis,
    drag_wheel_h: &mut MomentumAxis,
    immediate_drag_v: &mut ImmediateAxis,
    immediate_drag_h: &mut ImmediateAxis,
    out: &mut VirtualDevice,
) -> Result<(), Box<dyn Error>> {
    let cfg = engine::config();
    let middle_scroll_enabled = cfg.middle_scroll_enabled();
    let middle_scroll_mode =
        middle_scroll_enabled && middle.is_scroll_mode(cfg.tap_max_duration_ms());
    let smooth_enabled = cfg.smooth_enabled() && !modifiers.win_down;

    match axis {
        RelativeAxisCode::REL_X => {
            if middle_scroll_enabled && middle.pressed_at.is_some() {
                middle.push_motion(value, 0, cfg.drag_deadzone_px());
            }

            if middle_scroll_mode {
                if smooth_enabled {
                    drag_wheel_h.push_detents(
                        -(value as f64),
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    immediate_drag_h.push_detents(-(value as f64), cfg.drag_wheel_gain());
                    let _ = flush_immediate_axis(immediate_drag_h, out, false)?;
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_X, value)?;
            }
        }

        RelativeAxisCode::REL_Y => {
            if middle_scroll_enabled && middle.pressed_at.is_some() {
                middle.push_motion(0, value, cfg.drag_deadzone_px());
            }

            if middle_scroll_mode {
                if smooth_enabled {
                    drag_wheel_v.push_detents(
                        -(value as f64),
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    immediate_drag_v.push_detents(-(value as f64), cfg.drag_wheel_gain());
                    let _ = flush_immediate_axis(immediate_drag_v, out, true)?;
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_Y, value)?;
            }
        }

        RelativeAxisCode::REL_WHEEL => {
            if smooth_enabled {
                if middle_scroll_mode {
                    drag_wheel_v.push_detents(
                        value as f64,
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    normal_wheel_v.push_detents(
                        value as f64,
                        cfg.normal_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_WHEEL, value)?;
            }
        }

        RelativeAxisCode::REL_WHEEL_HI_RES => {
            if smooth_enabled {
                let detents = value as f64 / 120.0;
                if middle_scroll_mode {
                    drag_wheel_v.push_detents(
                        detents,
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    normal_wheel_v.push_detents(
                        detents,
                        cfg.normal_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_WHEEL_HI_RES, value)?;
            }
        }

        RelativeAxisCode::REL_HWHEEL => {
            if smooth_enabled {
                if middle_scroll_mode {
                    drag_wheel_h.push_detents(
                        value as f64,
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    normal_wheel_h.push_detents(
                        value as f64,
                        cfg.normal_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_HWHEEL, value)?;
            }
        }

        RelativeAxisCode::REL_HWHEEL_HI_RES => {
            if smooth_enabled {
                let detents = value as f64 / 120.0;
                if middle_scroll_mode {
                    drag_wheel_h.push_detents(
                        detents,
                        cfg.drag_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                } else {
                    normal_wheel_h.push_detents(
                        detents,
                        cfg.normal_wheel_gain(),
                        cfg.max_velocity_hires(),
                    );
                }
            } else {
                emit_rel(out, RelativeAxisCode::REL_HWHEEL_HI_RES, value)?;
            }
        }

        _ => {
            emit_rel(out, axis, value)?;
        }
    }

    Ok(())
}

fn advance_and_emit(
    modifiers: &ModifierState,
    dt: Duration,
    normal_wheel_v: &mut MomentumAxis,
    normal_wheel_h: &mut MomentumAxis,
    drag_wheel_v: &mut MomentumAxis,
    drag_wheel_h: &mut MomentumAxis,
    out: &mut VirtualDevice,
) -> Result<bool, Box<dyn Error>> {
    let cfg = engine::config();

    if modifiers.win_down || !cfg.smooth_enabled() {
        normal_wheel_v.clear();
        normal_wheel_h.clear();
        drag_wheel_v.clear();
        drag_wheel_h.clear();
        return Ok(false);
    }

    normal_wheel_v.tick(cfg.normal_wheel_damping(), dt);
    normal_wheel_h.tick(cfg.normal_wheel_damping(), dt);
    drag_wheel_v.tick(cfg.drag_wheel_damping(), dt);
    drag_wheel_h.tick(cfg.drag_wheel_damping(), dt);

    let mut emitted = false;
    emitted |= flush_axis(normal_wheel_v, out, true)?;
    emitted |= flush_axis(normal_wheel_h, out, false)?;
    emitted |= flush_axis(drag_wheel_v, out, true)?;
    emitted |= flush_axis(drag_wheel_h, out, false)?;

    Ok(emitted)
}

fn flush_axis(
    axis: &mut MomentumAxis,
    out: &mut VirtualDevice,
    vertical: bool,
) -> Result<bool, Box<dyn Error>> {
    let (hires, detents) = axis.drain();
    if hires == 0 && detents == 0 {
        return Ok(false);
    }

    let mut events = Vec::with_capacity(2);

    if hires != 0 {
        let code = if vertical {
            RelativeAxisCode::REL_WHEEL_HI_RES
        } else {
            RelativeAxisCode::REL_HWHEEL_HI_RES
        };
        events.push(InputEvent::new(EV_REL, code.0, hires));
    }

    if detents != 0 {
        let code = if vertical {
            RelativeAxisCode::REL_WHEEL
        } else {
            RelativeAxisCode::REL_HWHEEL
        };
        events.push(InputEvent::new(EV_REL, code.0, detents));
    }

    out.emit(&events)?;
    Ok(true)
}

fn flush_immediate_axis(
    axis: &mut ImmediateAxis,
    out: &mut VirtualDevice,
    vertical: bool,
) -> Result<bool, Box<dyn Error>> {
    let (hires, detents) = axis.drain();
    if hires == 0 && detents == 0 {
        return Ok(false);
    }

    let mut events = Vec::with_capacity(2);

    if hires != 0 {
        let code = if vertical {
            RelativeAxisCode::REL_WHEEL_HI_RES
        } else {
            RelativeAxisCode::REL_HWHEEL_HI_RES
        };
        events.push(InputEvent::new(EV_REL, code.0, hires));
    }

    if detents != 0 {
        let code = if vertical {
            RelativeAxisCode::REL_WHEEL
        } else {
            RelativeAxisCode::REL_HWHEEL
        };
        events.push(InputEvent::new(EV_REL, code.0, detents));
    }

    out.emit(&events)?;
    Ok(true)
}

fn emit_button_click(out: &mut VirtualDevice, button: KeyCode) -> Result<(), Box<dyn Error>> {
    let press = InputEvent::new(EV_KEY, button.0, 1);
    let release = InputEvent::new(EV_KEY, button.0, 0);
    out.emit(&[press, release])?;
    Ok(())
}

fn emit_key(out: &mut VirtualDevice, key: KeyCode, value: i32) -> Result<(), Box<dyn Error>> {
    let ev = InputEvent::new(EV_KEY, key.0, value);
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

    let ev = InputEvent::new(EV_REL, axis.0, value);
    out.emit(&[ev])?;
    Ok(())
}

fn boxed_io_error_with_hint(err: std::io::Error, hint: &str) -> Box<dyn Error> {
    if err.kind() == ErrorKind::PermissionDenied {
        Box::new(NimbusScrollError(format!("{hint}: {err}")))
    } else {
        Box::new(err)
    }
}

fn active_window_signature() -> Option<String> {
    if let Ok(cmd) = env::var("NIMBUS_SCROLL_FOCUS_CMD") {
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

fn debug_latency_enabled() -> bool {
    cfg!(debug_assertions)
}
