use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const DEFAULT_NORMAL_WHEEL_GAIN: f64 = 0.08;
pub(crate) const DEFAULT_NORMAL_WHEEL_DAMPING: f64 = 0.975;

pub(crate) const DEFAULT_DRAG_WHEEL_GAIN: f64 = 0.035;
pub(crate) const DEFAULT_DRAG_WHEEL_DAMPING: f64 = 0.985;

pub(crate) const DEFAULT_DRAG_DEADZONE_PX: f64 = 3.0;
pub(crate) const DEFAULT_TAP_MAX_DURATION_MS: u64 = 220;
pub(crate) const DEFAULT_LOOP_SLEEP_MS: u64 = 4;
pub(crate) const DEFAULT_MAX_VELOCITY_HIRES: f64 = 18.0;

pub(crate) const VELOCITY_EPSILON: f64 = 0.0001;
pub(crate) const ACCUM_EPSILON: f64 = 0.0001;

static CONFIG: OnceLock<Arc<ScrollConfig>> = OnceLock::new();
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub(crate) struct ScrollConfig {
    normal_wheel_gain: AtomicU64,
    normal_wheel_damping: AtomicU64,
    drag_wheel_gain: AtomicU64,
    drag_wheel_damping: AtomicU64,
    drag_deadzone_px: AtomicU64,
    tap_max_duration_ms: AtomicU64,
    loop_sleep_ms: AtomicU64,
    max_velocity_hires: AtomicU64,
    smooth_enabled: AtomicBool,
    middle_scroll_enabled: AtomicBool,
    mouse_device_path: Mutex<Option<String>>,
}

impl ScrollConfig {
    pub fn new() -> Self {
        Self {
            normal_wheel_gain: AtomicU64::new(DEFAULT_NORMAL_WHEEL_GAIN.to_bits()),
            normal_wheel_damping: AtomicU64::new(DEFAULT_NORMAL_WHEEL_DAMPING.to_bits()),
            drag_wheel_gain: AtomicU64::new(DEFAULT_DRAG_WHEEL_GAIN.to_bits()),
            drag_wheel_damping: AtomicU64::new(DEFAULT_DRAG_WHEEL_DAMPING.to_bits()),
            drag_deadzone_px: AtomicU64::new(DEFAULT_DRAG_DEADZONE_PX.to_bits()),
            tap_max_duration_ms: AtomicU64::new((DEFAULT_TAP_MAX_DURATION_MS as f64).to_bits()),
            loop_sleep_ms: AtomicU64::new((DEFAULT_LOOP_SLEEP_MS as f64).to_bits()),
            max_velocity_hires: AtomicU64::new(DEFAULT_MAX_VELOCITY_HIRES.to_bits()),
            smooth_enabled: AtomicBool::new(true),
            middle_scroll_enabled: AtomicBool::new(true),
            mouse_device_path: Mutex::new(None),
        }
    }

    pub fn reset_defaults(&self) {
        self.set_normal_wheel_gain(DEFAULT_NORMAL_WHEEL_GAIN);
        self.set_normal_wheel_damping(DEFAULT_NORMAL_WHEEL_DAMPING);
        self.set_drag_wheel_gain(DEFAULT_DRAG_WHEEL_GAIN);
        self.set_drag_wheel_damping(DEFAULT_DRAG_WHEEL_DAMPING);
        self.set_drag_deadzone_px(DEFAULT_DRAG_DEADZONE_PX);
        self.set_tap_max_duration_ms(DEFAULT_TAP_MAX_DURATION_MS as f64);
        self.set_loop_sleep_ms(DEFAULT_LOOP_SLEEP_MS as f64);
        self.set_max_velocity_hires(DEFAULT_MAX_VELOCITY_HIRES);
        self.set_smooth_enabled(true);
        self.set_middle_scroll_enabled(true);
    }

    fn load_f64(atom: &AtomicU64) -> f64 {
        f64::from_bits(atom.load(Ordering::Relaxed))
    }

    fn store_f64(atom: &AtomicU64, value: f64) {
        atom.store(value.to_bits(), Ordering::Relaxed);
    }

    pub fn normal_wheel_gain(&self) -> f64 {
        Self::load_f64(&self.normal_wheel_gain)
    }
    pub fn set_normal_wheel_gain(&self, value: f64) {
        Self::store_f64(&self.normal_wheel_gain, value.clamp(0.0, 1.0));
    }

    pub fn normal_wheel_damping(&self) -> f64 {
        Self::load_f64(&self.normal_wheel_damping)
    }
    pub fn set_normal_wheel_damping(&self, value: f64) {
        Self::store_f64(&self.normal_wheel_damping, value.clamp(0.0, 1.0));
    }

    pub fn drag_wheel_gain(&self) -> f64 {
        Self::load_f64(&self.drag_wheel_gain)
    }
    pub fn set_drag_wheel_gain(&self, value: f64) {
        Self::store_f64(&self.drag_wheel_gain, value.clamp(0.0, 1.0));
    }

    pub fn drag_wheel_damping(&self) -> f64 {
        Self::load_f64(&self.drag_wheel_damping)
    }
    pub fn set_drag_wheel_damping(&self, value: f64) {
        Self::store_f64(&self.drag_wheel_damping, value.clamp(0.0, 1.0));
    }

    pub fn drag_deadzone_px(&self) -> f64 {
        Self::load_f64(&self.drag_deadzone_px)
    }
    pub fn set_drag_deadzone_px(&self, value: f64) {
        Self::store_f64(&self.drag_deadzone_px, value.max(0.0));
    }

    pub fn tap_max_duration_ms(&self) -> u64 {
        Self::load_f64(&self.tap_max_duration_ms).round().max(1.0) as u64
    }
    pub fn set_tap_max_duration_ms(&self, value: f64) {
        Self::store_f64(&self.tap_max_duration_ms, value.max(1.0));
    }

    pub fn loop_sleep_ms(&self) -> u64 {
        Self::load_f64(&self.loop_sleep_ms).round().max(1.0) as u64
    }
    pub fn set_loop_sleep_ms(&self, value: f64) {
        Self::store_f64(&self.loop_sleep_ms, value.max(1.0));
    }

    pub fn max_velocity_hires(&self) -> f64 {
        Self::load_f64(&self.max_velocity_hires).max(0.0)
    }
    pub fn set_max_velocity_hires(&self, value: f64) {
        Self::store_f64(&self.max_velocity_hires, value.max(0.0));
    }

    pub fn smooth_enabled(&self) -> bool {
        self.smooth_enabled.load(Ordering::Relaxed)
    }
    pub fn set_smooth_enabled(&self, value: bool) {
        self.smooth_enabled.store(value, Ordering::Relaxed);
    }

    pub fn middle_scroll_enabled(&self) -> bool {
        self.middle_scroll_enabled.load(Ordering::Relaxed)
    }
    pub fn set_middle_scroll_enabled(&self, value: bool) {
        self.middle_scroll_enabled.store(value, Ordering::Relaxed);
    }

    pub fn mouse_device_path(&self) -> Option<String> {
        self.mouse_device_path
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn set_mouse_device_path(&self, value: Option<String>) {
        if let Ok(mut guard) = self.mouse_device_path.lock() {
            *guard = value;
        }
    }
}

pub(crate) fn init_config(cfg: Arc<ScrollConfig>) {
    let _ = CONFIG.set(cfg);
}

pub(crate) fn config() -> &'static ScrollConfig {
    CONFIG.get().expect("ScrollConfig not initialized")
}

pub(crate) fn request_exit() {
    EXIT_REQUESTED.store(true, Ordering::Relaxed);
}

pub(crate) fn should_exit() -> bool {
    EXIT_REQUESTED.load(Ordering::Relaxed)
}

#[derive(Debug, Default)]
pub(crate) struct ModifierState {
    pub(crate) win_down: bool,
}

impl ModifierState {
    pub const fn new() -> Self {
        Self { win_down: false }
    }
}

#[derive(Debug)]
pub(crate) struct MomentumAxis {
    pub(crate) velocity_hires: f64,
    pub(crate) hires_accum: f64,
    pub(crate) detent_accum: f64,
}

impl MomentumAxis {
    pub const fn new() -> Self {
        Self {
            velocity_hires: 0.0,
            hires_accum: 0.0,
            detent_accum: 0.0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.velocity_hires = 0.0;
        self.hires_accum = 0.0;
        self.detent_accum = 0.0;
    }

    pub(crate) fn push_detents(&mut self, input_detents: f64, gain: f64, max_velocity: f64) {
        self.velocity_hires += input_detents * 120.0 * gain;
        self.velocity_hires = self.velocity_hires.clamp(-max_velocity, max_velocity);
    }

    pub(crate) fn tick(&mut self, damping: f64, dt: Duration) {
        let dt_ms = dt.as_secs_f64() * 1000.0;

        // todo: fix hz
        let base_dt = 1000.0 / 144.0; // ≈ 6.94 ms

        let scale = (dt_ms / base_dt).clamp(0.25, 4.0);

        self.hires_accum += self.velocity_hires * scale;

        let effective_damping = damping.powf(scale);
        self.velocity_hires *= effective_damping;

        if self.velocity_hires.abs() < VELOCITY_EPSILON {
            self.velocity_hires = 0.0;
        }
        if self.hires_accum.abs() < ACCUM_EPSILON {
            self.hires_accum = 0.0;
        }
        if self.detent_accum.abs() < ACCUM_EPSILON {
            self.detent_accum = 0.0;
        }
    }

    pub(crate) fn drain(&mut self) -> (i32, i32) {
        let hires = trunc_to_i32(self.hires_accum);
        self.hires_accum -= hires as f64;

        self.detent_accum += hires as f64 / 120.0;
        let detents = trunc_to_i32(self.detent_accum);
        self.detent_accum -= detents as f64;

        (hires, detents)
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.velocity_hires == 0.0 && self.hires_accum == 0.0 && self.detent_accum == 0.0
    }
}

#[derive(Debug)]
pub(crate) struct ImmediateAxis {
    pub(crate) hires_accum: f64,
    pub(crate) detent_accum: f64,
}

impl ImmediateAxis {
    pub const fn new() -> Self {
        Self {
            hires_accum: 0.0,
            detent_accum: 0.0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.hires_accum = 0.0;
        self.detent_accum = 0.0;
    }

    pub(crate) fn push_detents(&mut self, input_detents: f64, gain: f64) {
        self.hires_accum += input_detents * 120.0 * gain;
        self.detent_accum += input_detents * gain;
    }

    pub(crate) fn drain(&mut self) -> (i32, i32) {
        let hires = trunc_to_i32(self.hires_accum);
        self.hires_accum -= hires as f64;

        let detents = trunc_to_i32(self.detent_accum);
        self.detent_accum -= detents as f64;

        (hires, detents)
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.hires_accum == 0.0 && self.detent_accum == 0.0
    }
}

#[derive(Debug)]
pub(crate) struct MiddleDragState {
    pub(crate) pressed_at: Option<Instant>,
    moved: bool,
    dx: f64,
    dy: f64,
}

impl MiddleDragState {
    pub const fn new() -> Self {
        Self {
            pressed_at: None,
            moved: false,
            dx: 0.0,
            dy: 0.0,
        }
    }

    pub(crate) fn begin(&mut self) {
        self.pressed_at = Some(Instant::now());
        self.moved = false;
        self.dx = 0.0;
        self.dy = 0.0;
    }

    pub(crate) fn clear(&mut self) {
        self.pressed_at = None;
        self.moved = false;
        self.dx = 0.0;
        self.dy = 0.0;
    }

    pub(crate) fn held_for(&self) -> Duration {
        self.pressed_at
            .map(|t| t.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0))
    }

    pub(crate) fn is_tap(&self, tap_max_duration_ms: u64) -> bool {
        !self.moved && self.held_for() <= Duration::from_millis(tap_max_duration_ms)
    }

    pub(crate) fn is_scroll_mode(&self, tap_max_duration_ms: u64) -> bool {
        self.pressed_at.is_some() && !self.is_tap(tap_max_duration_ms)
    }

    pub(crate) fn push_motion(&mut self, x: i32, y: i32, deadzone_px: f64) {
        self.dx += x as f64;
        self.dy += y as f64;
        if self.dx.abs() >= deadzone_px || self.dy.abs() >= deadzone_px {
            self.moved = true;
        }
    }
}

pub(crate) fn trunc_to_i32(v: f64) -> i32 {
    if v >= 0.0 {
        v.floor() as i32
    } else {
        v.ceil() as i32
    }
}
