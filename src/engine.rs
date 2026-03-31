use crate::easing;
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
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

const CONFIG_DIR_NAME: &str = "NimbusScroll";
const CONFIG_FILE_NAME: &str = "config.txt";

static CONFIG: OnceLock<Arc<ScrollConfig>> = OnceLock::new();
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EasingKind {
    Linear = 0,
    QuadIn = 1,
    QuadOut = 2,
    QuadInOut = 3,
    CubicIn = 4,
    CubicOut = 5,
    CubicInOut = 6,
    QuartIn = 7,
    QuartOut = 8,
    QuartInOut = 9,
    QuintIn = 10,
    QuintOut = 11,
    QuintInOut = 12,
    SineIn = 13,
    SineOut = 14,
    SineInOut = 15,
    CircIn = 16,
    CircOut = 17,
    CircInOut = 18,
    ExpoIn = 19,
    ExpoOut = 20,
    ExpoInOut = 21,
    ElasticIn = 22,
    ElasticOut = 23,
    ElasticInOut = 24,
    BackIn = 25,
    BackOut = 26,
    BackInOut = 27,
    BounceIn = 28,
    BounceOut = 29,
    BounceInOut = 30,
}

impl EasingKind {
    pub(crate) const ALL: [Self; 31] = [
        Self::Linear,
        Self::QuadIn,
        Self::QuadOut,
        Self::QuadInOut,
        Self::CubicIn,
        Self::CubicOut,
        Self::CubicInOut,
        Self::QuartIn,
        Self::QuartOut,
        Self::QuartInOut,
        Self::QuintIn,
        Self::QuintOut,
        Self::QuintInOut,
        Self::SineIn,
        Self::SineOut,
        Self::SineInOut,
        Self::CircIn,
        Self::CircOut,
        Self::CircInOut,
        Self::ExpoIn,
        Self::ExpoOut,
        Self::ExpoInOut,
        Self::ElasticIn,
        Self::ElasticOut,
        Self::ElasticInOut,
        Self::BackIn,
        Self::BackOut,
        Self::BackInOut,
        Self::BounceIn,
        Self::BounceOut,
        Self::BounceInOut,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::QuadIn => "quad_in",
            Self::QuadOut => "quad_out",
            Self::QuadInOut => "quad_inout",
            Self::CubicIn => "cubic_in",
            Self::CubicOut => "cubic_out",
            Self::CubicInOut => "cubic_inout",
            Self::QuartIn => "quart_in",
            Self::QuartOut => "quart_out",
            Self::QuartInOut => "quart_inout",
            Self::QuintIn => "quint_in",
            Self::QuintOut => "quint_out",
            Self::QuintInOut => "quint_inout",
            Self::SineIn => "sine_in",
            Self::SineOut => "sine_out",
            Self::SineInOut => "sine_inout",
            Self::CircIn => "circ_in",
            Self::CircOut => "circ_out",
            Self::CircInOut => "circ_inout",
            Self::ExpoIn => "expo_in",
            Self::ExpoOut => "expo_out",
            Self::ExpoInOut => "expo_inout",
            Self::ElasticIn => "elastic_in",
            Self::ElasticOut => "elastic_out",
            Self::ElasticInOut => "elastic_inout",
            Self::BackIn => "back_in",
            Self::BackOut => "back_out",
            Self::BackInOut => "back_inout",
            Self::BounceIn => "bounce_in",
            Self::BounceOut => "bounce_out",
            Self::BounceInOut => "bounce_inout",
        }
    }

    pub(crate) fn from_label(value: &str) -> Option<Self> {
        match value {
            "linear" => Some(Self::Linear),
            "quad_in" => Some(Self::QuadIn),
            "quad_out" => Some(Self::QuadOut),
            "quad_inout" => Some(Self::QuadInOut),
            "cubic_in" => Some(Self::CubicIn),
            "cubic_out" => Some(Self::CubicOut),
            "cubic_inout" => Some(Self::CubicInOut),
            "quart_in" => Some(Self::QuartIn),
            "quart_out" => Some(Self::QuartOut),
            "quart_inout" => Some(Self::QuartInOut),
            "quint_in" => Some(Self::QuintIn),
            "quint_out" => Some(Self::QuintOut),
            "quint_inout" => Some(Self::QuintInOut),
            "sine_in" => Some(Self::SineIn),
            "sine_out" => Some(Self::SineOut),
            "sine_inout" => Some(Self::SineInOut),
            "circ_in" => Some(Self::CircIn),
            "circ_out" => Some(Self::CircOut),
            "circ_inout" => Some(Self::CircInOut),
            "expo_in" => Some(Self::ExpoIn),
            "expo_out" => Some(Self::ExpoOut),
            "expo_inout" => Some(Self::ExpoInOut),
            "elastic_in" => Some(Self::ElasticIn),
            "elastic_out" => Some(Self::ElasticOut),
            "elastic_inout" => Some(Self::ElasticInOut),
            "back_in" => Some(Self::BackIn),
            "back_out" => Some(Self::BackOut),
            "back_inout" => Some(Self::BackInOut),
            "bounce_in" => Some(Self::BounceIn),
            "bounce_out" => Some(Self::BounceOut),
            "bounce_inout" => Some(Self::BounceInOut),
            _ => None,
        }
    }

    pub(crate) const fn from_u64(value: u64) -> Self {
        match value {
            1 => Self::QuadIn,
            2 => Self::QuadOut,
            3 => Self::QuadInOut,
            4 => Self::CubicIn,
            5 => Self::CubicOut,
            6 => Self::CubicInOut,
            7 => Self::QuartIn,
            8 => Self::QuartOut,
            9 => Self::QuartInOut,
            10 => Self::QuintIn,
            11 => Self::QuintOut,
            12 => Self::QuintInOut,
            13 => Self::SineIn,
            14 => Self::SineOut,
            15 => Self::SineInOut,
            16 => Self::CircIn,
            17 => Self::CircOut,
            18 => Self::CircInOut,
            19 => Self::ExpoIn,
            20 => Self::ExpoOut,
            21 => Self::ExpoInOut,
            22 => Self::ElasticIn,
            23 => Self::ElasticOut,
            24 => Self::ElasticInOut,
            25 => Self::BackIn,
            26 => Self::BackOut,
            27 => Self::BackInOut,
            28 => Self::BounceIn,
            29 => Self::BounceOut,
            30 => Self::BounceInOut,
            _ => Self::Linear,
        }
    }

    pub(crate) const fn to_u64(self) -> u64 {
        self as u64
    }

    pub(crate) fn apply(self, t: f64) -> f64 {
        match self {
            Self::Linear => easing::linear::<f64>(t),
            Self::QuadIn => easing::quad_in::<f64>(t),
            Self::QuadOut => easing::quad_out::<f64>(t),
            Self::QuadInOut => easing::quad_inout::<f64>(t),
            Self::CubicIn => easing::cubic_in::<f64>(t),
            Self::CubicOut => easing::cubic_out::<f64>(t),
            Self::CubicInOut => easing::cubic_inout::<f64>(t),
            Self::QuartIn => easing::quart_in::<f64>(t),
            Self::QuartOut => easing::quart_out::<f64>(t),
            Self::QuartInOut => easing::quart_inout::<f64>(t),
            Self::QuintIn => easing::quint_in::<f64>(t),
            Self::QuintOut => easing::quint_out::<f64>(t),
            Self::QuintInOut => easing::quint_inout::<f64>(t),
            Self::SineIn => easing::sine_in::<f64>(t),
            Self::SineOut => easing::sine_out::<f64>(t),
            Self::SineInOut => easing::sine_inout::<f64>(t),
            Self::CircIn => easing::circ_in::<f64>(t),
            Self::CircOut => easing::circ_out::<f64>(t),
            Self::CircInOut => easing::circ_inout::<f64>(t),
            Self::ExpoIn => easing::expo_in::<f64>(t),
            Self::ExpoOut => easing::expo_out::<f64>(t),
            Self::ExpoInOut => easing::expo_inout::<f64>(t),
            Self::ElasticIn => easing::elastic_in::<f64>(t),
            Self::ElasticOut => easing::elastic_out::<f64>(t),
            Self::ElasticInOut => easing::elastic_inout::<f64>(t),
            Self::BackIn => easing::back_in::<f64>(t),
            Self::BackOut => easing::back_out::<f64>(t),
            Self::BackInOut => easing::back_inout::<f64>(t),
            Self::BounceIn => easing::bounce_in::<f64>(t),
            Self::BounceOut => easing::bounce_out::<f64>(t),
            Self::BounceInOut => easing::bounce_inout::<f64>(t),
        }
    }
}

#[derive(Debug, Clone)]
struct ConfigSnapshot {
    normal_wheel_gain: f64,
    normal_wheel_damping: f64,
    drag_wheel_gain: f64,
    drag_wheel_damping: f64,
    drag_deadzone_px: f64,
    tap_max_duration_ms: u64,
    loop_sleep_ms: u64,
    max_velocity_hires: f64,
    easing_kind: EasingKind,
    smooth_enabled: bool,
    middle_scroll_enabled: bool,
    mouse_device_path: Option<String>,
}

impl ConfigSnapshot {
    fn defaults() -> Self {
        Self {
            normal_wheel_gain: DEFAULT_NORMAL_WHEEL_GAIN,
            normal_wheel_damping: DEFAULT_NORMAL_WHEEL_DAMPING,
            drag_wheel_gain: DEFAULT_DRAG_WHEEL_GAIN,
            drag_wheel_damping: DEFAULT_DRAG_WHEEL_DAMPING,
            drag_deadzone_px: DEFAULT_DRAG_DEADZONE_PX,
            tap_max_duration_ms: DEFAULT_TAP_MAX_DURATION_MS,
            loop_sleep_ms: DEFAULT_LOOP_SLEEP_MS,
            max_velocity_hires: DEFAULT_MAX_VELOCITY_HIRES,
            easing_kind: EasingKind::Linear,
            smooth_enabled: true,
            middle_scroll_enabled: true,
            mouse_device_path: None,
        }
    }
}

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
    easing_kind_bits: AtomicU64,
    smooth_enabled: AtomicBool,
    middle_scroll_enabled: AtomicBool,
    mouse_device_path: Mutex<Option<String>>,
}

impl ScrollConfig {
    pub fn new() -> Self {
        let this = Self {
            normal_wheel_gain: AtomicU64::new(DEFAULT_NORMAL_WHEEL_GAIN.to_bits()),
            normal_wheel_damping: AtomicU64::new(DEFAULT_NORMAL_WHEEL_DAMPING.to_bits()),
            drag_wheel_gain: AtomicU64::new(DEFAULT_DRAG_WHEEL_GAIN.to_bits()),
            drag_wheel_damping: AtomicU64::new(DEFAULT_DRAG_WHEEL_DAMPING.to_bits()),
            drag_deadzone_px: AtomicU64::new(DEFAULT_DRAG_DEADZONE_PX.to_bits()),
            tap_max_duration_ms: AtomicU64::new((DEFAULT_TAP_MAX_DURATION_MS as f64).to_bits()),
            loop_sleep_ms: AtomicU64::new((DEFAULT_LOOP_SLEEP_MS as f64).to_bits()),
            max_velocity_hires: AtomicU64::new(DEFAULT_MAX_VELOCITY_HIRES.to_bits()),
            easing_kind_bits: AtomicU64::new(EasingKind::Linear.to_u64()),
            smooth_enabled: AtomicBool::new(true),
            middle_scroll_enabled: AtomicBool::new(true),
            mouse_device_path: Mutex::new(None),
        };

        let _ = this.load_from_disk();
        let _ = this.save_to_disk();
        this
    }

    pub fn reset_defaults(&self) {
        self.set_normal_wheel_gain_raw(DEFAULT_NORMAL_WHEEL_GAIN);
        self.set_normal_wheel_damping_raw(DEFAULT_NORMAL_WHEEL_DAMPING);
        self.set_drag_wheel_gain_raw(DEFAULT_DRAG_WHEEL_GAIN);
        self.set_drag_wheel_damping_raw(DEFAULT_DRAG_WHEEL_DAMPING);
        self.set_drag_deadzone_px_raw(DEFAULT_DRAG_DEADZONE_PX);
        self.set_tap_max_duration_ms_raw(DEFAULT_TAP_MAX_DURATION_MS as f64);
        self.set_loop_sleep_ms_raw(DEFAULT_LOOP_SLEEP_MS as f64);
        self.set_max_velocity_hires_raw(DEFAULT_MAX_VELOCITY_HIRES);
        self.set_easing_kind_raw(EasingKind::Linear);
        self.set_smooth_enabled_raw(true);
        self.set_middle_scroll_enabled_raw(true);
        self.set_mouse_device_path_raw(None);
        let _ = self.save_to_disk();
    }

    fn load_f64(atom: &AtomicU64) -> f64 {
        f64::from_bits(atom.load(Ordering::Relaxed))
    }

    fn store_f64(atom: &AtomicU64, value: f64) {
        atom.store(value.to_bits(), Ordering::Relaxed);
    }

    fn snapshot(&self) -> ConfigSnapshot {
        ConfigSnapshot {
            normal_wheel_gain: self.normal_wheel_gain(),
            normal_wheel_damping: self.normal_wheel_damping(),
            drag_wheel_gain: self.drag_wheel_gain(),
            drag_wheel_damping: self.drag_wheel_damping(),
            drag_deadzone_px: self.drag_deadzone_px(),
            tap_max_duration_ms: self.tap_max_duration_ms(),
            loop_sleep_ms: self.loop_sleep_ms(),
            max_velocity_hires: self.max_velocity_hires(),
            easing_kind: self.easing_kind(),
            smooth_enabled: self.smooth_enabled(),
            middle_scroll_enabled: self.middle_scroll_enabled(),
            mouse_device_path: self.mouse_device_path(),
        }
    }

    fn apply_snapshot(&self, snap: ConfigSnapshot) {
        self.set_normal_wheel_gain_raw(snap.normal_wheel_gain);
        self.set_normal_wheel_damping_raw(snap.normal_wheel_damping);
        self.set_drag_wheel_gain_raw(snap.drag_wheel_gain);
        self.set_drag_wheel_damping_raw(snap.drag_wheel_damping);
        self.set_drag_deadzone_px_raw(snap.drag_deadzone_px);
        self.set_tap_max_duration_ms_raw(snap.tap_max_duration_ms as f64);
        self.set_loop_sleep_ms_raw(snap.loop_sleep_ms as f64);
        self.set_max_velocity_hires_raw(snap.max_velocity_hires);
        self.set_easing_kind_raw(snap.easing_kind);
        self.set_smooth_enabled_raw(snap.smooth_enabled);
        self.set_middle_scroll_enabled_raw(snap.middle_scroll_enabled);
        self.set_mouse_device_path_raw(snap.mouse_device_path);
    }

    fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let home = env::var_os("USERPROFILE").map(PathBuf::from)?;
            return Some(
                home.join("Documents")
                    .join(CONFIG_DIR_NAME)
                    .join(CONFIG_FILE_NAME),
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            let home = env::var_os("HOME").map(PathBuf::from)?;
            return Some(
                home.join(".config")
                    .join(CONFIG_DIR_NAME)
                    .join(CONFIG_FILE_NAME),
            );
        }
    }

    fn format_f64(value: f64) -> String {
        format!("{:.17}", value)
    }

    fn escape_string(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 8);
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out
    }

    fn unescape_string(value: &str) -> Option<String> {
        let mut out = String::with_capacity(value.len());
        let mut chars = value.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                out.push(ch);
                continue;
            }

            let next = chars.next()?;
            match next {
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
        }
        Some(out)
    }

    fn parse_bool(value: &str) -> Option<bool> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    fn parse_f64(value: &str) -> Option<f64> {
        value.trim().parse::<f64>().ok()
    }

    fn parse_u64(value: &str) -> Option<u64> {
        value.trim().parse::<u64>().ok()
    }

    fn parse_optional_string(value: &str) -> Option<Option<String>> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Some(None);
        }

        if trimmed == "\"\"" {
            return Some(None);
        }

        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            let inner = &trimmed[1..trimmed.len() - 1];
            return Self::unescape_string(inner).map(|s| if s.is_empty() { None } else { Some(s) });
        }

        Some(Some(trimmed.to_string()))
    }

    fn parse_snapshot(text: &str) -> ConfigSnapshot {
        let mut snap = ConfigSnapshot::defaults();

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();

            match key {
                "normal_wheel_gain" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.normal_wheel_gain = v;
                    }
                }
                "normal_wheel_damping" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.normal_wheel_damping = v;
                    }
                }
                "drag_wheel_gain" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.drag_wheel_gain = v;
                    }
                }
                "drag_wheel_damping" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.drag_wheel_damping = v;
                    }
                }
                "drag_deadzone_px" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.drag_deadzone_px = v;
                    }
                }
                "tap_max_duration_ms" => {
                    if let Some(v) = Self::parse_u64(value) {
                        snap.tap_max_duration_ms = v;
                    } else if let Some(v) = Self::parse_f64(value) {
                        snap.tap_max_duration_ms = v.max(1.0) as u64;
                    }
                }
                "loop_sleep_ms" => {
                    if let Some(v) = Self::parse_u64(value) {
                        snap.loop_sleep_ms = v;
                    } else if let Some(v) = Self::parse_f64(value) {
                        snap.loop_sleep_ms = v.max(1.0) as u64;
                    }
                }
                "max_velocity_hires" => {
                    if let Some(v) = Self::parse_f64(value) {
                        snap.max_velocity_hires = v;
                    }
                }
                "easing_kind" => {
                    if let Some(kind) = EasingKind::from_label(value.trim_matches('"')) {
                        snap.easing_kind = kind;
                    } else if let Ok(raw) = value.trim().parse::<u64>() {
                        snap.easing_kind = EasingKind::from_u64(raw);
                    }
                }
                "smooth_enabled" => {
                    if let Some(v) = Self::parse_bool(value) {
                        snap.smooth_enabled = v;
                    }
                }
                "middle_scroll_enabled" => {
                    if let Some(v) = Self::parse_bool(value) {
                        snap.middle_scroll_enabled = v;
                    }
                }
                "mouse_device_path" => {
                    if let Some(v) = Self::parse_optional_string(value) {
                        snap.mouse_device_path = v;
                    }
                }
                _ => {}
            }
        }

        snap
    }

    fn load_from_disk(&self) -> io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        match fs::read_to_string(&path) {
            Ok(text) => {
                let snap = Self::parse_snapshot(&text);
                self.apply_snapshot(snap);
                Ok(())
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn save_to_disk(&self) -> io::Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let snap = self.snapshot();

        let mut text = String::new();
        text.push_str("# NimbusScroll configuration\n");
        text.push_str("# This file is rewritten automatically by the application.\n");
        text.push_str("# Values are loaded on startup.\n\n");

        text.push_str(&format!(
            "normal_wheel_gain={}\n",
            Self::format_f64(snap.normal_wheel_gain)
        ));
        text.push_str(&format!(
            "normal_wheel_damping={}\n",
            Self::format_f64(snap.normal_wheel_damping)
        ));
        text.push_str(&format!(
            "drag_wheel_gain={}\n",
            Self::format_f64(snap.drag_wheel_gain)
        ));
        text.push_str(&format!(
            "drag_wheel_damping={}\n",
            Self::format_f64(snap.drag_wheel_damping)
        ));
        text.push_str(&format!(
            "drag_deadzone_px={}\n",
            Self::format_f64(snap.drag_deadzone_px)
        ));
        text.push_str(&format!(
            "tap_max_duration_ms={}\n",
            snap.tap_max_duration_ms
        ));
        text.push_str(&format!("loop_sleep_ms={}\n", snap.loop_sleep_ms));
        text.push_str(&format!(
            "max_velocity_hires={}\n",
            Self::format_f64(snap.max_velocity_hires)
        ));
        text.push_str(&format!("easing_kind={}\n", snap.easing_kind.label()));
        text.push_str(&format!("smooth_enabled={}\n", snap.smooth_enabled));
        text.push_str(&format!(
            "middle_scroll_enabled={}\n",
            snap.middle_scroll_enabled
        ));
        text.push_str(&format!(
            "mouse_device_path=\"{}\"\n",
            Self::escape_string(snap.mouse_device_path.as_deref().unwrap_or(""))
        ));

        fs::write(path, text)
    }

    fn set_normal_wheel_gain_raw(&self, value: f64) {
        Self::store_f64(&self.normal_wheel_gain, value.clamp(0.0, 1.0));
    }

    fn set_normal_wheel_damping_raw(&self, value: f64) {
        Self::store_f64(&self.normal_wheel_damping, value.clamp(0.0, 1.0));
    }

    fn set_drag_wheel_gain_raw(&self, value: f64) {
        Self::store_f64(&self.drag_wheel_gain, value.clamp(0.0, 1.0));
    }

    fn set_drag_wheel_damping_raw(&self, value: f64) {
        Self::store_f64(&self.drag_wheel_damping, value.clamp(0.0, 1.0));
    }

    fn set_drag_deadzone_px_raw(&self, value: f64) {
        Self::store_f64(&self.drag_deadzone_px, value.max(0.0));
    }

    fn set_tap_max_duration_ms_raw(&self, value: f64) {
        Self::store_f64(&self.tap_max_duration_ms, value.max(1.0));
    }

    fn set_loop_sleep_ms_raw(&self, value: f64) {
        Self::store_f64(&self.loop_sleep_ms, value.max(1.0));
    }

    fn set_max_velocity_hires_raw(&self, value: f64) {
        Self::store_f64(&self.max_velocity_hires, value.max(0.0));
    }

    fn set_easing_kind_raw(&self, value: EasingKind) {
        self.easing_kind_bits
            .store(value.to_u64(), Ordering::Relaxed);
    }

    fn set_smooth_enabled_raw(&self, value: bool) {
        self.smooth_enabled.store(value, Ordering::Relaxed);
    }

    fn set_middle_scroll_enabled_raw(&self, value: bool) {
        self.middle_scroll_enabled.store(value, Ordering::Relaxed);
    }

    fn set_mouse_device_path_raw(&self, value: Option<String>) {
        if let Ok(mut guard) = self.mouse_device_path.lock() {
            *guard = value;
        }
    }

    pub fn normal_wheel_gain(&self) -> f64 {
        Self::load_f64(&self.normal_wheel_gain)
    }
    pub fn set_normal_wheel_gain(&self, value: f64) {
        self.set_normal_wheel_gain_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn normal_wheel_damping(&self) -> f64 {
        Self::load_f64(&self.normal_wheel_damping)
    }
    pub fn set_normal_wheel_damping(&self, value: f64) {
        self.set_normal_wheel_damping_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn drag_wheel_gain(&self) -> f64 {
        Self::load_f64(&self.drag_wheel_gain)
    }
    pub fn set_drag_wheel_gain(&self, value: f64) {
        self.set_drag_wheel_gain_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn drag_wheel_damping(&self) -> f64 {
        Self::load_f64(&self.drag_wheel_damping)
    }
    pub fn set_drag_wheel_damping(&self, value: f64) {
        self.set_drag_wheel_damping_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn drag_deadzone_px(&self) -> f64 {
        Self::load_f64(&self.drag_deadzone_px)
    }
    pub fn set_drag_deadzone_px(&self, value: f64) {
        self.set_drag_deadzone_px_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn tap_max_duration_ms(&self) -> u64 {
        Self::load_f64(&self.tap_max_duration_ms).round().max(1.0) as u64
    }
    pub fn set_tap_max_duration_ms(&self, value: f64) {
        self.set_tap_max_duration_ms_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn loop_sleep_ms(&self) -> u64 {
        Self::load_f64(&self.loop_sleep_ms).round().max(1.0) as u64
    }
    pub fn set_loop_sleep_ms(&self, value: f64) {
        self.set_loop_sleep_ms_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn max_velocity_hires(&self) -> f64 {
        Self::load_f64(&self.max_velocity_hires).max(0.0)
    }
    pub fn set_max_velocity_hires(&self, value: f64) {
        self.set_max_velocity_hires_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn easing_kind(&self) -> EasingKind {
        EasingKind::from_u64(self.easing_kind_bits.load(Ordering::Relaxed))
    }
    pub fn set_easing_kind(&self, value: EasingKind) {
        self.set_easing_kind_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn smooth_enabled(&self) -> bool {
        self.smooth_enabled.load(Ordering::Relaxed)
    }
    pub fn set_smooth_enabled(&self, value: bool) {
        self.set_smooth_enabled_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn middle_scroll_enabled(&self) -> bool {
        self.middle_scroll_enabled.load(Ordering::Relaxed)
    }
    pub fn set_middle_scroll_enabled(&self, value: bool) {
        self.set_middle_scroll_enabled_raw(value);
        let _ = self.save_to_disk();
    }

    pub fn mouse_device_path(&self) -> Option<String> {
        self.mouse_device_path
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn set_mouse_device_path(&self, value: Option<String>) {
        self.set_mouse_device_path_raw(value);
        let _ = self.save_to_disk();
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

#[derive(Clone, Debug)]
pub(crate) struct MouseDeviceInfo {
    pub(crate) label: String,
    pub(crate) path: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollKey {
    LeftMeta,
    RightMeta,
    Middle,
    Left,
    Right,
    Side,
    Extra,
    Forward,
    Back,
    Task,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollAxis {
    X,
    Y,
    Wheel,
    WheelHiRes,
    HWheel,
    HWheelHiRes,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputEvent {
    Key { key: ScrollKey, value: i32 },
    Rel { axis: ScrollAxis, value: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputEvent {
    Key { key: ScrollKey, value: i32 },
    Rel { axis: ScrollAxis, value: i32 },
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

    pub(crate) fn tick(&mut self, damping: f64, dt: Duration, easing_kind: EasingKind) {
        let dt_ms = dt.as_secs_f64() * 1000.0;

        let base_dt = 1000.0 / 144.0;
        let raw_scale = (dt_ms / base_dt).clamp(0.25, 4.0);
        let normalized = ((raw_scale - 0.25) / 3.75).clamp(0.0, 1.0);
        let eased = easing_kind.apply(normalized).clamp(0.0, 1.0);
        let scale = 0.25 + eased * 3.75;

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

    pub(crate) fn drain_events(&mut self, vertical: bool) -> Vec<OutputEvent> {
        let (hires, detents) = self.drain();
        let mut out = Vec::with_capacity(2);

        if hires != 0 {
            let axis = if vertical {
                ScrollAxis::WheelHiRes
            } else {
                ScrollAxis::HWheelHiRes
            };
            out.push(OutputEvent::Rel { axis, value: hires });
        }

        if detents != 0 {
            let axis = if vertical {
                ScrollAxis::Wheel
            } else {
                ScrollAxis::HWheel
            };
            out.push(OutputEvent::Rel {
                axis,
                value: detents,
            });
        }

        out
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

    pub(crate) fn drain_events(&mut self, vertical: bool) -> Vec<OutputEvent> {
        let (hires, detents) = self.drain();
        let mut out = Vec::with_capacity(2);

        if hires != 0 {
            let axis = if vertical {
                ScrollAxis::WheelHiRes
            } else {
                ScrollAxis::HWheelHiRes
            };
            out.push(OutputEvent::Rel { axis, value: hires });
        }

        if detents != 0 {
            let axis = if vertical {
                ScrollAxis::Wheel
            } else {
                ScrollAxis::HWheel
            };
            out.push(OutputEvent::Rel {
                axis,
                value: detents,
            });
        }

        out
    }
}

#[derive(Debug)]
pub(crate) struct ScrollController {
    normal_wheel_v: MomentumAxis,
    normal_wheel_h: MomentumAxis,
    drag_wheel_v: MomentumAxis,
    drag_wheel_h: MomentumAxis,
    immediate_drag_v: ImmediateAxis,
    immediate_drag_h: ImmediateAxis,
    middle: MiddleDragState,
    modifiers: ModifierState,
}

impl ScrollController {
    pub fn new() -> Self {
        Self {
            normal_wheel_v: MomentumAxis::new(),
            normal_wheel_h: MomentumAxis::new(),
            drag_wheel_v: MomentumAxis::new(),
            drag_wheel_h: MomentumAxis::new(),
            immediate_drag_v: ImmediateAxis::new(),
            immediate_drag_h: ImmediateAxis::new(),
            middle: MiddleDragState::new(),
            modifiers: ModifierState::new(),
        }
    }

    pub fn clear_scroll_state(&mut self) {
        self.normal_wheel_v.clear();
        self.normal_wheel_h.clear();
        self.drag_wheel_v.clear();
        self.drag_wheel_h.clear();
        self.immediate_drag_v.clear();
        self.immediate_drag_h.clear();
        self.middle.clear();
    }

    pub fn handle_input(&mut self, input: InputEvent, cfg: &ScrollConfig) -> Vec<OutputEvent> {
        match input {
            InputEvent::Key { key, value } => self.handle_key(key, value),
            InputEvent::Rel { axis, value } => self.handle_rel(axis, value, cfg),
        }
    }

    fn handle_key(&mut self, key: ScrollKey, value: i32) -> Vec<OutputEvent> {
        let mut out = Vec::new();

        match key {
            ScrollKey::LeftMeta | ScrollKey::RightMeta => {
                let new_state = value != 0;

                if !self.modifiers.win_down && new_state {
                    self.normal_wheel_v.clear();
                    self.normal_wheel_h.clear();
                    self.drag_wheel_v.clear();
                    self.drag_wheel_h.clear();
                    self.immediate_drag_v.clear();
                    self.immediate_drag_h.clear();
                }

                self.modifiers.win_down = new_state;
            }

            ScrollKey::Middle => {
                if value == 1 {
                    self.middle.begin();
                } else if value == 0 {
                    self.middle.clear();
                }

                out.push(OutputEvent::Key { key, value });
            }

            ScrollKey::Left
            | ScrollKey::Right
            | ScrollKey::Side
            | ScrollKey::Extra
            | ScrollKey::Forward
            | ScrollKey::Back
            | ScrollKey::Task
            | ScrollKey::Other(_) => {
                out.push(OutputEvent::Key { key, value });
            }
        }

        out
    }

    fn handle_rel(&mut self, axis: ScrollAxis, value: i32, cfg: &ScrollConfig) -> Vec<OutputEvent> {
        let middle_scroll_enabled = cfg.middle_scroll_enabled();
        let middle_scroll_mode =
            middle_scroll_enabled && self.middle.is_scroll_mode(cfg.tap_max_duration_ms());
        let smooth_enabled = cfg.smooth_enabled() && !self.modifiers.win_down;

        let mut out = Vec::new();

        match axis {
            ScrollAxis::X => {
                if middle_scroll_enabled && self.middle.pressed_at.is_some() {
                    self.middle.push_motion(value, 0, cfg.drag_deadzone_px());
                }

                if middle_scroll_mode {
                    if smooth_enabled {
                        self.drag_wheel_h.push_detents(
                            -(value as f64),
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.immediate_drag_h
                            .push_detents(-(value as f64), cfg.drag_wheel_gain());
                        out.extend(self.immediate_drag_h.drain_events(false));
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::Y => {
                if middle_scroll_enabled && self.middle.pressed_at.is_some() {
                    self.middle.push_motion(0, value, cfg.drag_deadzone_px());
                }

                if middle_scroll_mode {
                    if smooth_enabled {
                        self.drag_wheel_v.push_detents(
                            -(value as f64),
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.immediate_drag_v
                            .push_detents(-(value as f64), cfg.drag_wheel_gain());
                        out.extend(self.immediate_drag_v.drain_events(true));
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::Wheel => {
                if smooth_enabled {
                    if middle_scroll_mode {
                        self.drag_wheel_v.push_detents(
                            value as f64,
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.normal_wheel_v.push_detents(
                            value as f64,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::WheelHiRes => {
                if smooth_enabled {
                    let detents = value as f64 / 120.0;
                    if middle_scroll_mode {
                        self.drag_wheel_v.push_detents(
                            detents,
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.normal_wheel_v.push_detents(
                            detents,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::HWheel => {
                if smooth_enabled {
                    if middle_scroll_mode {
                        self.drag_wheel_h.push_detents(
                            value as f64,
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.normal_wheel_h.push_detents(
                            value as f64,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::HWheelHiRes => {
                if smooth_enabled {
                    let detents = value as f64 / 120.0;
                    if middle_scroll_mode {
                        self.drag_wheel_h.push_detents(
                            detents,
                            cfg.drag_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    } else {
                        self.normal_wheel_h.push_detents(
                            detents,
                            cfg.normal_wheel_gain(),
                            cfg.max_velocity_hires(),
                        );
                    }
                } else {
                    out.push(OutputEvent::Rel { axis, value });
                }
            }

            ScrollAxis::Other(_) => {
                out.push(OutputEvent::Rel { axis, value });
            }
        }

        out
    }

    pub fn advance(&mut self, cfg: &ScrollConfig, dt: Duration) -> Vec<OutputEvent> {
        if self.modifiers.win_down || !cfg.smooth_enabled() {
            self.normal_wheel_v.clear();
            self.normal_wheel_h.clear();
            self.drag_wheel_v.clear();
            self.drag_wheel_h.clear();
            return Vec::new();
        }

        let easing_kind = cfg.easing_kind();

        self.normal_wheel_v
            .tick(cfg.normal_wheel_damping(), dt, easing_kind);
        self.normal_wheel_h
            .tick(cfg.normal_wheel_damping(), dt, easing_kind);
        self.drag_wheel_v
            .tick(cfg.drag_wheel_damping(), dt, easing_kind);
        self.drag_wheel_h
            .tick(cfg.drag_wheel_damping(), dt, easing_kind);

        let mut out = Vec::new();
        out.extend(self.normal_wheel_v.drain_events(true));
        out.extend(self.normal_wheel_h.drain_events(false));
        out.extend(self.drag_wheel_v.drain_events(true));
        out.extend(self.drag_wheel_h.drain_events(false));
        out
    }
}

pub(crate) fn trunc_to_i32(v: f64) -> i32 {
    if v >= 0.0 {
        v.floor() as i32
    } else {
        v.ceil() as i32
    }
}
