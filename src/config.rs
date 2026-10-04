use serde::{Deserialize, Serialize};
use std::{env, error::Error, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub display: Display,
    pub devices: Devices,
    #[serde(default)]
    pub joystick: Option<Joystick>,
    #[serde(default)]
    pub aim: Option<Aim>,
    #[serde(default)]
    pub taps: Vec<Tap>,
    #[serde(default)]
    pub holds: Vec<Hold>,
    #[serde(default)]
    pub mouse_taps: Vec<MouseTap>,
    #[serde(default)]
    pub mouse_holds: Vec<MouseHold>,
    #[serde(default)]
    pub performance: Performance,
    #[serde(default)]
    pub touch: TouchSettings,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Display {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Devices {
    pub keyboard: Option<String>,
    pub mouse: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Joystick {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    #[serde(default = "default_true")]
    pub normalize_diagonal: bool,
    #[serde(default = "slot_0")]
    pub slot: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Aim {
    pub button: String,
    pub center_x: f32,
    pub center_y: f32,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    #[serde(default = "slot_1")]
    pub slot: u8,
    #[serde(default)]
    pub invert_x: bool,
    #[serde(default)]
    pub invert_y: bool,
    #[serde(default = "default_one")]
    pub scale_x: f32,
    #[serde(default = "default_one")]
    pub scale_y: f32,
    #[serde(default = "default_edge")]
    pub edge_margin: f32,
    #[serde(default = "default_touch_mode")]
    pub mode: String,
}

impl Aim {
    pub fn is_continuous(&self) -> bool {
        self.button.trim().eq_ignore_ascii_case("ALWAYS")
            || self.button.trim().eq_ignore_ascii_case("CONTINUOUS")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tap {
    pub key: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "slot_2")]
    pub slot: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Hold {
    pub key: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "slot_3")]
    pub slot: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MouseTap {
    pub button: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "slot_4")]
    pub slot: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MouseHold {
    pub button: String,
    pub x: f32,
    pub y: f32,
    #[serde(default = "slot_5")]
    pub slot: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Performance {
    #[serde(default = "default_true")]
    pub grab: bool,
    #[serde(default = "default_true")]
    pub realtime: bool,
    #[serde(default = "default_priority")]
    pub realtime_priority: i32,
    #[serde(default)]
    pub mouse_lock: bool,
    #[serde(default = "default_true")]
    pub auto_lock_on_aim: bool,
    #[serde(default = "default_toggle")]
    pub mouse_toggle_key: String,
    #[serde(default = "default_retries")]
    pub fifo_write_retries: u8,
    #[serde(default = "default_wait")]
    pub fifo_write_wait_ms: u64,
    #[serde(default = "default_reconnect")]
    pub fifo_reconnect_ms: u64,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            grab: true,
            realtime: true,
            realtime_priority: 10,
            mouse_lock: false,
            auto_lock_on_aim: true,
            mouse_toggle_key: "F8".into(),
            fifo_write_retries: 3,
            fifo_write_wait_ms: 1,
            fifo_reconnect_ms: 25,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TouchSettings {
    #[serde(default = "default_pressure")]
    pub pressure: i32,
    #[serde(default = "default_size")]
    pub major: i32,
    #[serde(default = "default_size")]
    pub minor: i32,
}

impl Default for TouchSettings {
    fn default() -> Self {
        Self {
            pressure: 80,
            major: 8,
            minor: 8,
        }
    }
}

fn default_sensitivity() -> f32 { 1.0 }
fn default_one() -> f32 { 1.0 }
fn default_edge() -> f32 { 0.12 }
fn default_touch_mode() -> String { "touch".into() }
fn default_true() -> bool { true }
fn default_priority() -> i32 { 10 }
fn default_toggle() -> String { "F8".into() }
fn default_retries() -> u8 { 3 }
fn default_wait() -> u64 { 1 }
fn default_reconnect() -> u64 { 25 }
fn default_pressure() -> i32 { 80 }
fn default_size() -> i32 { 8 }
fn slot_0() -> u8 { 0 }
fn slot_1() -> u8 { 1 }
fn slot_2() -> u8 { 2 }
fn slot_3() -> u8 { 3 }
fn slot_4() -> u8 { 4 }
fn slot_5() -> u8 { 5 }

impl Config {
    pub fn conflicts(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut keys: Vec<(u16, String)> = Vec::new();
        let mut mice: Vec<(u16, String)> = Vec::new();

        if let Some(j) = &self.joystick {
            for (name, key) in [
                ("joystick.up", &j.up),
                ("joystick.down", &j.down),
                ("joystick.left", &j.left),
                ("joystick.right", &j.right),
            ] {
                if let Ok(code) = crate::input::key_code(key) {
                    keys.push((code, name.into()));
                }
            }
        }

        for (i, x) in self.taps.iter().enumerate() {
            if let Ok(code) = crate::input::key_code(&x.key) {
                keys.push((code, format!("taps[{i}]")));
            }
        }
        for (i, x) in self.holds.iter().enumerate() {
            if let Ok(code) = crate::input::key_code(&x.key) {
                keys.push((code, format!("holds[{i}]")));
            }
        }
        if let Ok(toggle) = crate::input::key_code(&self.performance.mouse_toggle_key) {
            keys.push((toggle, "performance.mouse_toggle_key".into()));
        }

        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                if keys[i].0 == keys[j].0 {
                    out.push(format!("keyboard conflict: {} <-> {}", keys[i].1, keys[j].1));
                }
            }
        }

        if let Some(a) = &self.aim {
            if !a.is_continuous() {
                if let Ok(code) = crate::input::button_code(&a.button) {
                    mice.push((code, "aim.button".into()));
                }
            }
        }
        for (i, x) in self.mouse_taps.iter().enumerate() {
            if let Ok(code) = crate::input::button_code(&x.button) {
                mice.push((code, format!("mouse_taps[{i}]")));
            }
        }
        for (i, x) in self.mouse_holds.iter().enumerate() {
            if let Ok(code) = crate::input::button_code(&x.button) {
                mice.push((code, format!("mouse_holds[{i}]")));
            }
        }

        for i in 0..mice.len() {
            for j in (i + 1)..mice.len() {
                if mice[i].0 == mice[j].0 {
                    out.push(format!("mouse conflict: {} <-> {}", mice[i].1, mice[j].1));
                }
            }
        }

        if let Some(j) = &self.joystick {
            let dirs = [
                ("up", &j.up),
                ("down", &j.down),
                ("left", &j.left),
                ("right", &j.right),
            ];
            for i in 0..dirs.len() {
                for k in (i + 1)..dirs.len() {
                    if dirs[i].1.eq_ignore_ascii_case(dirs[k].1) {
                        out.push(format!(
                            "joystick conflict: {} and {} use {}",
                            dirs[i].0, dirs[k].0, dirs[i].1
                        ));
                    }
                }
            }
        }

        out
    }

    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        if !(1..=16384).contains(&self.display.width) || !(1..=16384).contains(&self.display.height) {
            return Err("display size must be within 1..=16384".into());
        }

        if let (Some(k), Some(m)) = (&self.devices.keyboard, &self.devices.mouse) {
            if !k.is_empty() && !m.is_empty() && k == m {
                return Err("keyboard and mouse cannot use the same evdev device".into());
            }
        }

        let auto_lock_needs_grab = self
            .aim
            .as_ref()
            .map(|a| self.performance.auto_lock_on_aim && !a.is_continuous())
            .unwrap_or(false);

        if (self.performance.mouse_lock || auto_lock_needs_grab) && !self.performance.grab {
            return Err("mouse locking requires performance.grab=true".into());
        }

        if !(1..=99).contains(&self.performance.realtime_priority) {
            return Err("realtime_priority must be 1..99".into());
        }
        if !(1..=8).contains(&self.performance.fifo_write_retries) {
            return Err("fifo_write_retries must be 1..8".into());
        }
        if self.performance.fifo_write_wait_ms > 5 {
            return Err("fifo_write_wait_ms must be 0..5 ms".into());
        }
        if !(5..=2000).contains(&self.performance.fifo_reconnect_ms) {
            return Err("fifo_reconnect_ms must be 5..2000 ms".into());
        }
        if !(1..=255).contains(&self.touch.pressure)
            || !(1..=255).contains(&self.touch.major)
            || !(1..=255).contains(&self.touch.minor)
        {
            return Err("touch pressure/major/minor must be 1..255".into());
        }

        let mut used = [false; 16];
        let mut reserve = |slot: u8| -> Result<(), Box<dyn Error>> {
            let i = slot as usize;
            if i >= used.len() {
                return Err(format!("touch slot {slot} must be 0..15").into());
            }
            if used[i] {
                return Err(format!("duplicate touch slot {slot}").into());
            }
            used[i] = true;
            Ok(())
        };

        if let Some(j) = &self.joystick {
            for key in [&j.up, &j.down, &j.left, &j.right] {
                crate::input::key_code(key)?;
            }
            if !(0.0..=1.0).contains(&j.center_x)
                || !(0.0..=1.0).contains(&j.center_y)
                || !(0.0..1.0).contains(&j.radius)
                || j.slot >= 16
            {
                return Err("invalid joystick position/radius/slot".into());
            }
            reserve(j.slot)?;
        }

        if let Some(a) = &self.aim {
            if !a.is_continuous() {
                crate::input::button_code(&a.button)?;
            }
            if !(0.0..=1.0).contains(&a.center_x)
                || !(0.0..=1.0).contains(&a.center_y)
                || !(a.sensitivity > 0.0 && a.sensitivity <= 100.0)
                || !(a.scale_x > 0.0 && a.scale_x <= 20.0)
                || !(a.scale_y > 0.0 && a.scale_y <= 20.0)
                || !(0.0..=0.49).contains(&a.edge_margin)
                || a.slot >= 16
            {
                return Err("invalid aim parameters".into());
            }
            match a.mode.trim().to_ascii_lowercase().as_str() {
                "touch" | "relative" => {}
                _ => return Err("aim mode must be touch or relative".into()),
            }
            reserve(a.slot)?;
        }

        for x in &self.taps {
            crate::input::key_code(&x.key)?;
            if !(0.0..=1.0).contains(&x.x) || !(0.0..=1.0).contains(&x.y) {
                return Err("invalid keyboard tap coordinates".into());
            }
            reserve(x.slot)?;
        }

        for x in &self.holds {
            crate::input::key_code(&x.key)?;
            if !(0.0..=1.0).contains(&x.x) || !(0.0..=1.0).contains(&x.y) {
                return Err("invalid keyboard hold coordinates".into());
            }
            reserve(x.slot)?;
        }

        for x in &self.mouse_taps {
            crate::input::button_code(&x.button)?;
            if !(0.0..=1.0).contains(&x.x) || !(0.0..=1.0).contains(&x.y) {
                return Err("invalid mouse tap coordinates".into());
            }
            reserve(x.slot)?;
        }

        for x in &self.mouse_holds {
            crate::input::button_code(&x.button)?;
            if !(0.0..=1.0).contains(&x.x) || !(0.0..=1.0).contains(&x.y) {
                return Err("invalid mouse hold coordinates".into());
            }
            reserve(x.slot)?;
        }

        crate::input::key_code(&self.performance.mouse_toggle_key)?;

        if let Some(msg) = self.conflicts().into_iter().next() {
            return Err(msg.into());
        }

        Ok(())
    }

    pub fn validate_runtime(&self) -> Result<(), Box<dyn Error>> {
        self.validate()?;

        let keyboard_required = self.performance.mouse_lock
            || self.joystick.is_some()
            || !self.taps.is_empty()
            || !self.holds.is_empty();

        if keyboard_required && self.devices.keyboard.as_deref().unwrap_or("").is_empty() {
            return Err("a keyboard device is required for the configured keyboard controls".into());
        }

        let mouse_required = self.performance.mouse_lock
            || self.aim.is_some()
            || !self.mouse_taps.is_empty()
            || !self.mouse_holds.is_empty();

        if mouse_required && self.devices.mouse.as_deref().unwrap_or("").is_empty() {
            return Err("a mouse device is required for the configured mouse controls".into());
        }

        Ok(())
    }

    pub fn touch_fifo(&self) -> String {
        if let Ok(p) = env::var("WAYDROID_TOUCH_FIFO") {
            if !p.trim().is_empty() {
                return p;
            }
        }

        [
            "/dev/input/wl_touch_events",
            "/var/lib/waydroid/rootfs/dev/input/wl_touch_events",
            "/opt/waydroid/rootfs/dev/input/wl_touch_events",
        ]
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| (*p).to_string())
        .unwrap_or_else(|| "/dev/input/wl_touch_events".into())
    }

    pub fn pointer_fifo(&self) -> String {
        if let Ok(p) = env::var("WAYDROID_POINTER_FIFO") {
            if !p.trim().is_empty() {
                return p;
            }
        }

        [
            "/dev/input/wl_pointer_events",
            "/var/lib/waydroid/rootfs/dev/input/wl_pointer_events",
            "/opt/waydroid/rootfs/dev/input/wl_pointer_events",
        ]
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| (*p).to_string())
        .unwrap_or_else(|| "/dev/input/wl_pointer_events".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Config {
        Config {
            display: Display { width: 1920, height: 1080 },
            devices: Devices { keyboard: None, mouse: None },
            joystick: None,
            aim: None,
            taps: Vec::new(),
            holds: Vec::new(),
            mouse_taps: Vec::new(),
            mouse_holds: Vec::new(),
            performance: Performance::default(),
            touch: TouchSettings::default(),
        }
    }

    #[test]
    fn default_start_is_unlocked() {
        assert!(!Performance::default().mouse_lock);
        assert!(Performance::default().grab);
        assert!(Performance::default().auto_lock_on_aim);
    }

    #[test]
    fn continuous_aim_does_not_require_a_mouse_button() {
        let mut c = base();
        c.aim = Some(Aim {
            button: "ALWAYS".into(),
            center_x: 0.5,
            center_y: 0.5,
            sensitivity: 2.0,
            slot: 1,
            invert_x: false,
            invert_y: false,
            scale_x: 1.0,
            scale_y: 1.0,
            edge_margin: 0.12,
            mode: "relative".into(),
        });
        assert!(c.validate().is_ok());
        assert!(c.aim.as_ref().unwrap().is_continuous());
    }

    #[test]
    fn duplicate_slots_are_rejected() {
        let mut c = base();
        c.taps.push(Tap { key: "SPACE".into(), x: 0.8, y: 0.8, slot: 2 });
        c.holds.push(Hold { key: "F".into(), x: 0.7, y: 0.8, slot: 2 });
        assert!(c.validate().is_err());
    }

    #[test]
    fn toggle_key_conflict_is_rejected() {
        let mut c = base();
        c.performance.mouse_toggle_key = "W".into();
        c.joystick = Some(Joystick {
            up: "W".into(),
            down: "S".into(),
            left: "A".into(),
            right: "D".into(),
            center_x: 0.15,
            center_y: 0.76,
            radius: 0.085,
            normalize_diagonal: true,
            slot: 0,
        });
        assert!(c.conflicts().iter().any(|x| x.contains("keyboard conflict")));
    }

    #[test]
    fn auto_lock_requires_grab_for_button_aim() {
        let mut c = base();
        c.performance.grab = false;
        c.aim = Some(Aim {
            button: "MOUSE_RIGHT".into(),
            center_x: 0.5,
            center_y: 0.5,
            sensitivity: 2.0,
            slot: 1,
            invert_x: false,
            invert_y: false,
            scale_x: 1.0,
            scale_y: 1.0,
            edge_margin: 0.12,
            mode: "relative".into(),
        });
        assert!(c.validate().is_err());
    }
}
