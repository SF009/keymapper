#[path = "../config.rs"]
mod config;
#[path = "../control.rs"]
mod control;
#[path = "../input.rs"]
mod input;
#[path = "../touch.rs"]
mod touch;

use config::{
    Aim, Config, Devices, Display, Hold, Joystick, MouseHold, MouseTap, Performance, Tap,
    TouchSettings,
};
use gtk4::gdk::Display as GdkDisplay;
use gtk4::prelude::*;
use gtk4::{
    cairo, glib, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton,
    ComboBoxText, CssProvider, Dialog, DrawingArea, Entry, EventControllerKey, Frame,
    GestureClick, GestureDrag, Grid, Label, ListBox, ListBoxRow, Orientation, Paned,
    PolicyType, ScrolledWindow, Separator, SpinButton,
};
use std::{
    cell::{Cell, RefCell},
    env,
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const APP_ID: &str = "io.sf009.WaydroidKeymapper";

const APP_CSS: &str = r#"
window {
    background-color: #12151c;
    color: #e2e8f0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Cantarell, "Ubuntu", sans-serif;
}

headerbar {
    background-color: #181c26;
    border-bottom: 1px solid #283042;
    padding: 6px 10px;
}

.card {
    background-color: #181c26;
    border: 1px solid #283042;
    border-radius: 8px;
    padding: 10px;
    margin: 4px;
}

.section-title {
    font-size: 13px;
    font-weight: 700;
    color: #64b5f6;
    margin-bottom: 6px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.status-badge {
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 600;
}

.status-green {
    background-color: rgba(38, 194, 129, 0.18);
    color: #26c281;
    border: 1px solid rgba(38, 194, 129, 0.4);
}

.status-amber {
    background-color: rgba(245, 166, 35, 0.18);
    color: #f5a623;
    border: 1px solid rgba(245, 166, 35, 0.4);
}

.status-red {
    background-color: rgba(247, 85, 85, 0.18);
    color: #f75555;
    border: 1px solid rgba(247, 85, 85, 0.4);
}

.btn-primary {
    background: linear-gradient(135deg, #3a7afe, #2563eb);
    color: #ffffff;
    font-weight: 700;
    border-radius: 6px;
    padding: 6px 14px;
    border: none;
}
.btn-primary:hover {
    background: linear-gradient(135deg, #4c89fe, #3b82f6);
}

.btn-success {
    background: linear-gradient(135deg, #10b981, #059669);
    color: #ffffff;
    font-weight: 700;
    border-radius: 6px;
    padding: 6px 14px;
    border: none;
}
.btn-success:hover {
    background: linear-gradient(135deg, #34d399, #10b981);
}

.btn-accent {
    background-color: #242b3b;
    color: #93c5fd;
    border: 1px solid #3b82f6;
    border-radius: 6px;
    font-weight: 600;
}
.btn-accent:hover {
    background-color: #2e374d;
    color: #bfdbfe;
}

.btn-danger {
    background-color: #2b1f24;
    color: #f87171;
    border: 1px solid #ef4444;
    border-radius: 6px;
}
.btn-danger:hover {
    background-color: #3b242c;
    color: #fca5a5;
}

entry, spinbutton, combobox {
    background-color: #202634;
    color: #f1f5f9;
    border: 1px solid #333d52;
    border-radius: 6px;
    padding: 4px 8px;
}
entry:focus, spinbutton:focus, combobox:focus {
    border-color: #3b82f6;
    box-shadow: 0 0 0 1px #3b82f6;
}

list {
    background-color: #151821;
    border-radius: 6px;
}
list row {
    padding: 6px 10px;
    border-bottom: 1px solid #202634;
}
list row:selected {
    background-color: #2563eb;
    color: #ffffff;
}

.keycap {
    background-color: #283042;
    color: #38bdf8;
    font-family: monospace;
    font-weight: 700;
    border-radius: 4px;
    padding: 2px 6px;
    border: 1px solid #3b82f6;
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingRef {
    Tap(usize),
    Hold(usize),
    MouseTap(usize),
    MouseHold(usize),
    Aim,
    Joystick,
}

struct State {
    cfg: Config,
    profile_path: PathBuf,
    selected: Option<BindingRef>,
    dirty: bool,
}

#[derive(Clone)]
struct Ui {
    state: Rc<RefCell<State>>,
    profile_list: ListBox,
    bindings_box: GtkBox,
    canvas: DrawingArea,
    status: Label,
    profile_name: Entry,
    width: SpinButton,
    height: SpinButton,
    keyboard: ComboBoxText,
    mouse: ComboBoxText,
    aim_enabled: CheckButton,
    aim_button: Entry,
    aim_mode: ComboBoxText,
    aim_x: SpinButton,
    aim_y: SpinButton,
    aim_sensitivity: SpinButton,
    aim_slot: SpinButton,
    aim_invert_x: CheckButton,
    aim_invert_y: CheckButton,
    aim_scale_x: SpinButton,
    aim_scale_y: SpinButton,
    aim_edge_margin: SpinButton,
    joy_enabled: CheckButton,
    joy_up: Entry,
    joy_down: Entry,
    joy_left: Entry,
    joy_right: Entry,
    joy_x: SpinButton,
    joy_y: SpinButton,
    joy_radius: SpinButton,
    joy_slot: SpinButton,
    joy_normalize: CheckButton,
    grab: CheckButton,
    realtime: CheckButton,
    realtime_priority: SpinButton,
    fifo_write_retries: SpinButton,
    fifo_write_wait: SpinButton,
    fifo_reconnect: SpinButton,
    touch_pressure: SpinButton,
    touch_major: SpinButton,
    touch_minor: SpinButton,
    mouse_lock: CheckButton,
    mouse_toggle: Entry,
    runtime_status: Label,
    lock_status: Label,
    waydroid_status: Label,
    input_access: Label,
    selected_editor_box: GtkBox,
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn profiles_dir() -> PathBuf {
    home_dir().join(".config/waydroid-keymapper/profiles")
}

fn active_config_path() -> PathBuf {
    home_dir().join(".config/waydroid-keymapper/config.toml")
}

fn default_config() -> Config {
    Config {
        display: Display {
            width: 1920,
            height: 1080,
        },
        devices: Devices {
            keyboard: None,
            mouse: None,
        },
        joystick: Some(Joystick {
            up: "W".into(),
            down: "S".into(),
            left: "A".into(),
            right: "D".into(),
            center_x: 0.15,
            center_y: 0.76,
            radius: 0.085,
            normalize_diagonal: true,
            slot: 0,
        }),
        aim: Some(Aim {
            button: "ALWAYS".into(),
            center_x: 0.50,
            center_y: 0.50,
            sensitivity: 2.0,
            slot: 1,
            invert_x: false,
            invert_y: false,
            scale_x: 1.0,
            scale_y: 1.0,
            edge_margin: 0.12,
            mode: "relative".into(),
        }),
        taps: vec![
            Tap {
                key: "SPACE".into(),
                x: 0.86,
                y: 0.86,
                slot: 2,
            },
            Tap {
                key: "R".into(),
                x: 0.93,
                y: 0.18,
                slot: 3,
            },
        ],
        holds: vec![Hold {
            key: "F".into(),
            x: 0.78,
            y: 0.84,
            slot: 4,
        }],
        mouse_taps: vec![MouseTap {
            button: "MOUSE_RIGHT".into(),
            x: 0.88,
            y: 0.58,
            slot: 6,
        }],
        mouse_holds: vec![MouseHold {
            button: "MOUSE_LEFT".into(),
            x: 0.88,
            y: 0.78,
            slot: 5,
        }],
        performance: Performance {
            grab: true,
            realtime: true,
            realtime_priority: 10,
            mouse_lock: false,
            mouse_toggle_key: "F8".into(),
            fifo_write_retries: 3,
            fifo_write_wait_ms: 1,
            fifo_reconnect_ms: 25,
        },
        touch: TouchSettings::default(),
    }
}

fn preset_free_fire() -> Config {
    let mut cfg = default_config();
    cfg.aim = Some(Aim {
        button: "ALWAYS".into(),
        center_x: 0.50,
        center_y: 0.50,
        sensitivity: 2.0,
        slot: 1,
        invert_x: false,
        invert_y: false,
        scale_x: 1.0,
        scale_y: 1.0,
        edge_margin: 0.12,
        mode: "relative".into(),
    });
    cfg.taps = vec![
        Tap {
            key: "SPACE".into(),
            x: 0.88,
            y: 0.88,
            slot: 2,
        }, // Jump
        Tap {
            key: "C".into(),
            x: 0.78,
            y: 0.90,
            slot: 3,
        }, // Crouch
        Tap {
            key: "Z".into(),
            x: 0.70,
            y: 0.90,
            slot: 4,
        }, // Prone
        Tap {
            key: "R".into(),
            x: 0.92,
            y: 0.20,
            slot: 5,
        }, // Reload
        Tap {
            key: "1".into(),
            x: 0.70,
            y: 0.16,
            slot: 6,
        }, // Gun 1
        Tap {
            key: "2".into(),
            x: 0.78,
            y: 0.16,
            slot: 7,
        }, // Gun 2
        Tap {
            key: "3".into(),
            x: 0.86,
            y: 0.16,
            slot: 8,
        }, // Pistol/Melee
        Tap {
            key: "G".into(),
            x: 0.18,
            y: 0.55,
            slot: 9,
        }, // Gloo Wall
        Tap {
            key: "4".into(),
            x: 0.12,
            y: 0.55,
            slot: 10,
        }, // Medkit
        Tap {
            key: "TAB".into(),
            x: 0.08,
            y: 0.88,
            slot: 11,
        }, // Bag
    ];
    cfg.holds = vec![
        Hold {
            key: "SHIFT".into(),
            x: 0.28,
            y: 0.75,
            slot: 12,
        }, // Sprint
        Hold {
            key: "F".into(),
            x: 0.76,
            y: 0.76,
            slot: 13,
        }, // Interact/Loot
    ];
    cfg.mouse_taps = vec![
        MouseTap {
            button: "MOUSE_RIGHT".into(),
            x: 0.88,
            y: 0.58,
            slot: 14,
        }, // Scope / ADS
    ];
    cfg.mouse_holds = vec![
        MouseHold {
            button: "MOUSE_LEFT".into(),
            x: 0.88,
            y: 0.78,
            slot: 15,
        }, // Fire
    ];
    cfg
}

fn preset_pubg() -> Config {
    let mut cfg = default_config();
    cfg.aim = Some(Aim {
        button: "ALWAYS".into(),
        center_x: 0.50,
        center_y: 0.50,
        sensitivity: 2.2,
        slot: 1,
        invert_x: false,
        invert_y: false,
        scale_x: 1.0,
        scale_y: 1.0,
        edge_margin: 0.12,
        mode: "relative".into(),
    });
    cfg.taps = vec![
        Tap {
            key: "SPACE".into(),
            x: 0.88,
            y: 0.88,
            slot: 2,
        }, // Jump/Vault
        Tap {
            key: "C".into(),
            x: 0.78,
            y: 0.90,
            slot: 3,
        }, // Crouch
        Tap {
            key: "Z".into(),
            x: 0.70,
            y: 0.90,
            slot: 4,
        }, // Prone
        Tap {
            key: "Q".into(),
            x: 0.35,
            y: 0.45,
            slot: 5,
        }, // Peek Left
        Tap {
            key: "E".into(),
            x: 0.42,
            y: 0.45,
            slot: 6,
        }, // Peek Right
        Tap {
            key: "R".into(),
            x: 0.92,
            y: 0.20,
            slot: 7,
        }, // Reload
        Tap {
            key: "1".into(),
            x: 0.70,
            y: 0.16,
            slot: 8,
        }, // Primary
        Tap {
            key: "2".into(),
            x: 0.78,
            y: 0.16,
            slot: 9,
        }, // Secondary
        Tap {
            key: "M".into(),
            x: 0.92,
            y: 0.08,
            slot: 10,
        }, // Map
        Tap {
            key: "TAB".into(),
            x: 0.08,
            y: 0.88,
            slot: 11,
        }, // Backpack
    ];
    cfg.holds = vec![
        Hold {
            key: "SHIFT".into(),
            x: 0.28,
            y: 0.75,
            slot: 12,
        }, // Sprint
        Hold {
            key: "F".into(),
            x: 0.76,
            y: 0.76,
            slot: 13,
        }, // Drive/Interact
    ];
    cfg.mouse_taps = vec![MouseTap {
        button: "MOUSE_RIGHT".into(),
        x: 0.88,
        y: 0.58,
        slot: 14,
    }];
    cfg.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 15,
    }];
    cfg
}

fn preset_fps() -> Config {
    let mut cfg = default_config();
    cfg.taps = vec![
        Tap {
            key: "SPACE".into(),
            x: 0.86,
            y: 0.86,
            slot: 2,
        },
        Tap {
            key: "R".into(),
            x: 0.93,
            y: 0.18,
            slot: 3,
        },
        Tap {
            key: "1".into(),
            x: 0.72,
            y: 0.18,
            slot: 4,
        },
        Tap {
            key: "2".into(),
            x: 0.78,
            y: 0.18,
            slot: 5,
        },
    ];
    cfg.holds = vec![
        Hold {
            key: "SHIFT".into(),
            x: 0.28,
            y: 0.76,
            slot: 6,
        },
        Hold {
            key: "C".into(),
            x: 0.34,
            y: 0.88,
            slot: 7,
        },
        Hold {
            key: "F".into(),
            x: 0.78,
            y: 0.84,
            slot: 8,
        },
    ];
    cfg.mouse_taps = vec![MouseTap {
        button: "MOUSE_RIGHT".into(),
        x: 0.88,
        y: 0.58,
        slot: 9,
    }];
    cfg.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 10,
    }];
    cfg
}

fn preset_minimal() -> Config {
    let mut cfg = default_config();
    cfg.taps = vec![
        Tap {
            key: "SPACE".into(),
            x: 0.86,
            y: 0.86,
            slot: 2,
        },
        Tap {
            key: "R".into(),
            x: 0.93,
            y: 0.18,
            slot: 3,
        },
    ];
    cfg.holds = vec![Hold {
        key: "SHIFT".into(),
        x: 0.28,
        y: 0.76,
        slot: 4,
    }];
    cfg.mouse_taps = vec![MouseTap {
        button: "MOUSE_RIGHT".into(),
        x: 0.88,
        y: 0.58,
        slot: 6,
    }];
    cfg.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 5,
    }];
    cfg
}

#[derive(Clone, Copy)]
enum ShooterPreset {
    FreeFire,
    Pubg,
    Fps,
    Minimal,
}

fn apply_preset(ui: &Ui, preset: ShooterPreset) {
    sync_state_from_form(ui);
    let preset_cfg = match preset {
        ShooterPreset::FreeFire => preset_free_fire(),
        ShooterPreset::Pubg => preset_pubg(),
        ShooterPreset::Fps => preset_fps(),
        ShooterPreset::Minimal => preset_minimal(),
    };
    let mut st = ui.state.borrow_mut();
    let mut cfg = preset_cfg;
    cfg.display = st.cfg.display.clone();
    cfg.devices = st.cfg.devices.clone();
    cfg.performance = st.cfg.performance.clone();
    st.cfg = cfg;
    st.selected = None;
    st.dirty = true;
    drop(st);

    sync_form(ui);
    rebuild_bindings(ui);
    ui.canvas.queue_draw();
    set_status(
        ui,
        match preset {
            ShooterPreset::FreeFire => "🔥 Free Fire preset loaded — reposition & click Save",
            ShooterPreset::Pubg => "🎯 PUBG Mobile preset loaded — reposition & click Save",
            ShooterPreset::Fps => "🎮 FPS Shooter preset loaded — reposition & click Save",
            ShooterPreset::Minimal => "⚡ Minimal preset loaded — reposition & click Save",
        },
    );
}

fn ensure_profiles() -> Result<PathBuf, Box<dyn Error>> {
    let dir = profiles_dir();
    fs::create_dir_all(&dir)?;
    let default_path = dir.join("default.toml");
    if !default_path.exists() {
        let cfg = active_config_path();
        if cfg.exists() {
            match load_profile(&cfg) {
                Ok(existing) => fs::write(&default_path, toml::to_string_pretty(&existing)?)?,
                Err(_) => fs::write(&default_path, toml::to_string_pretty(&default_config())?)?,
            }
        } else {
            fs::write(&default_path, toml::to_string_pretty(&default_config())?)?;
        }
    }
    Ok(dir)
}

fn profile_files() -> Vec<PathBuf> {
    let dir = profiles_dir();
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    files.sort_by_key(|p| p.file_name().map(|x| x.to_os_string()));
    files
}

fn load_profile(path: &Path) -> Result<Config, Box<dyn Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

fn save_profile(path: &Path, cfg: &Config) -> Result<(), Box<dyn Error>> {
    cfg.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path, "save");
    fs::write(&tmp, toml::to_string_pretty(cfg)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("profile")
        .to_string()
}

fn safe_profile_name(name: &str) -> Option<String> {
    let safe = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('_').to_string();
    if safe.is_empty() || safe == "." || safe == ".." {
        None
    } else {
        Some(safe)
    }
}

fn clamp(v: f64) -> f32 {
    v.clamp(0., 1.) as f32
}

fn temp_path(path: &Path, tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path.file_name().and_then(|x| x.to_str()).unwrap_or("tmp");
    path.with_file_name(format!(".{name}.{tag}.{pid}.{nanos}.tmp"))
}

fn add_margins<W: gtk4::prelude::WidgetExt>(w: &W, m: i32) {
    w.set_margin_top(m);
    w.set_margin_bottom(m);
    w.set_margin_start(m);
    w.set_margin_end(m);
}

fn set_status(ui: &Ui, msg: &str) {
    ui.status.set_text(msg);
}

fn selected_position(cfg: &Config, sel: BindingRef) -> Option<(f32, f32)> {
    match sel {
        BindingRef::Tap(i) => cfg.taps.get(i).map(|x| (x.x, x.y)),
        BindingRef::Hold(i) => cfg.holds.get(i).map(|x| (x.x, x.y)),
        BindingRef::MouseTap(i) => cfg.mouse_taps.get(i).map(|x| (x.x, x.y)),
        BindingRef::MouseHold(i) => cfg.mouse_holds.get(i).map(|x| (x.x, x.y)),
        BindingRef::Aim => cfg.aim.as_ref().map(|x| (x.center_x, x.center_y)),
        BindingRef::Joystick => cfg.joystick.as_ref().map(|x| (x.center_x, x.center_y)),
    }
}

fn set_selected_position(cfg: &mut Config, sel: BindingRef, x: f32, y: f32) {
    match sel {
        BindingRef::Tap(i) => {
            if let Some(v) = cfg.taps.get_mut(i) {
                v.x = x;
                v.y = y;
            }
        }
        BindingRef::Hold(i) => {
            if let Some(v) = cfg.holds.get_mut(i) {
                v.x = x;
                v.y = y;
            }
        }
        BindingRef::MouseTap(i) => {
            if let Some(v) = cfg.mouse_taps.get_mut(i) {
                v.x = x;
                v.y = y;
            }
        }
        BindingRef::MouseHold(i) => {
            if let Some(v) = cfg.mouse_holds.get_mut(i) {
                v.x = x;
                v.y = y;
            }
        }
        BindingRef::Aim => {
            if let Some(v) = cfg.aim.as_mut() {
                v.center_x = x;
                v.center_y = y;
            }
        }
        BindingRef::Joystick => {
            if let Some(v) = cfg.joystick.as_mut() {
                v.center_x = x;
                v.center_y = y;
            }
        }
    }
}

fn marker_color(sel: BindingRef) -> (f64, f64, f64) {
    match sel {
        BindingRef::Aim => (0.0, 0.85, 1.0),      // Cyan
        BindingRef::Joystick => (0.0, 0.96, 0.60), // Emerald Green
        BindingRef::Tap(_) => (1.0, 0.75, 0.15),   // Gold
        BindingRef::Hold(_) => (0.25, 0.60, 1.0),  // Royal Blue
        BindingRef::MouseTap(_) => (0.85, 0.40, 1.0), // Purple
        BindingRef::MouseHold(_) => (1.0, 0.20, 0.50), // Hot Pink/Fire
    }
}

fn binding_label(sel: BindingRef, cfg: &Config) -> String {
    match sel {
        BindingRef::Tap(i) => cfg
            .taps
            .get(i)
            .map(|x| format!("⌨ {} [TAP] (Slot {})", x.key, x.slot))
            .unwrap_or_default(),
        BindingRef::Hold(i) => cfg
            .holds
            .get(i)
            .map(|x| format!("⌨ {} [HOLD] (Slot {})", x.key, x.slot))
            .unwrap_or_default(),
        BindingRef::MouseTap(i) => cfg
            .mouse_taps
            .get(i)
            .map(|x| format!("🖱 {} [TAP] (Slot {})", x.button, x.slot))
            .unwrap_or_default(),
        BindingRef::MouseHold(i) => cfg
            .mouse_holds
            .get(i)
            .map(|x| format!("🖱 {} [HOLD] (Slot {})", x.button, x.slot))
            .unwrap_or_default(),
        BindingRef::Aim => format!(
            "🎯 AIM • {} ({}) (Slot {})",
            cfg.aim
                .as_ref()
                .map(|x| x.button.as_str())
                .unwrap_or("ALWAYS"),
            cfg.aim.as_ref().map(|x| x.mode.as_str()).unwrap_or("relative"),
            cfg.aim.as_ref().map(|x| x.slot).unwrap_or(1)
        ),
        BindingRef::Joystick => format!(
            "🕹 JOYSTICK (WASD) (Slot {})",
            cfg.joystick.as_ref().map(|x| x.slot).unwrap_or(0)
        ),
    }
}

fn all_selectable(cfg: &Config) -> Vec<BindingRef> {
    let mut v = Vec::new();
    if cfg.joystick.is_some() {
        v.push(BindingRef::Joystick);
    }
    if cfg.aim.is_some() {
        v.push(BindingRef::Aim);
    }
    v.extend((0..cfg.taps.len()).map(BindingRef::Tap));
    v.extend((0..cfg.holds.len()).map(BindingRef::Hold));
    v.extend((0..cfg.mouse_taps.len()).map(BindingRef::MouseTap));
    v.extend((0..cfg.mouse_holds.len()).map(BindingRef::MouseHold));
    v
}

fn nearest_binding(cfg: &Config, x: f32, y: f32) -> Option<BindingRef> {
    let mut best = None;
    let mut best_d = 0.055_f32;
    for sel in all_selectable(cfg) {
        if let Some((px, py)) = selected_position(cfg, sel) {
            let d = ((px - x) * (px - x) + (py - y) * (py - y)).sqrt();
            if d < best_d {
                best_d = d;
                best = Some(sel);
            }
        }
    }
    best
}

fn next_available_slot(cfg: &Config) -> u8 {
    let mut used = [false; 16];
    if let Some(j) = &cfg.joystick {
        if (j.slot as usize) < 16 {
            used[j.slot as usize] = true;
        }
    }
    if let Some(a) = &cfg.aim {
        if (a.slot as usize) < 16 {
            used[a.slot as usize] = true;
        }
    }
    for x in &cfg.taps {
        if (x.slot as usize) < 16 {
            used[x.slot as usize] = true;
        }
    }
    for x in &cfg.holds {
        if (x.slot as usize) < 16 {
            used[x.slot as usize] = true;
        }
    }
    for x in &cfg.mouse_taps {
        if (x.slot as usize) < 16 {
            used[x.slot as usize] = true;
        }
    }
    for x in &cfg.mouse_holds {
        if (x.slot as usize) < 16 {
            used[x.slot as usize] = true;
        }
    }

    for (i, &u) in used.iter().enumerate() {
        if !u {
            return i as u8;
        }
    }
    15
}

fn draw_canvas(ui_state: &Rc<RefCell<State>>, _area: &DrawingArea, cr: &cairo::Context, w: i32, h: i32) {
    let st = ui_state.borrow();
    let cfg = &st.cfg;
    let pad = 16.;
    let cw = (w as f64 - 2. * pad).max(10.);
    let ch = (h as f64 - 2. * pad).max(10.);
    let scale = (cw / cfg.display.width.max(1) as f64).min(ch / cfg.display.height.max(1) as f64);
    let vw = cfg.display.width as f64 * scale;
    let vh = cfg.display.height as f64 * scale;
    let ox = (w as f64 - vw) / 2.;
    let oy = (h as f64 - vh) / 2.;

    // Canvas Background
    cr.set_source_rgb(0.07, 0.08, 0.11);
    cr.rectangle(0., 0., w as f64, h as f64);
    let _ = cr.fill();

    // Phone / Display Screen Simulation
    cr.set_source_rgb(0.11, 0.13, 0.18);
    cr.rectangle(ox, oy, vw, vh);
    let _ = cr.fill();

    // Subtle Grid Lines
    cr.set_source_rgba(0.20, 0.25, 0.35, 0.35);
    cr.set_line_width(1.0);
    for n in 1..10 {
        let gx = ox + vw * (n as f64 / 10.);
        let gy = oy + vh * (n as f64 / 10.);
        cr.move_to(gx, oy);
        cr.line_to(gx, oy + vh);
        cr.move_to(ox, gy);
        cr.line_to(ox + vw, gy);
    }
    let _ = cr.stroke();

    // Screen Center Guide Cross
    cr.set_source_rgba(0.35, 0.45, 0.65, 0.4);
    let cx = ox + vw * 0.5;
    let cy = oy + vh * 0.5;
    cr.move_to(cx - 20., cy);
    cr.line_to(cx + 20., cy);
    cr.move_to(cx, cy - 20.);
    cr.line_to(cx, cy + 20.);
    let _ = cr.stroke();

    // Screen Outer Border
    cr.set_source_rgb(0.24, 0.30, 0.42);
    cr.set_line_width(2.0);
    cr.rectangle(ox, oy, vw, vh);
    let _ = cr.stroke();

    let p = |x: f32, y: f32| (ox + vw * x as f64, oy + vh * y as f64);

    // Render Joystick
    if let Some(j) = &cfg.joystick {
        let (jx, jy) = p(j.center_x, j.center_y);
        let r = vw.min(vh) * j.radius as f64;
        let selected = st.selected == Some(BindingRef::Joystick);

        // Outer glow & fill
        cr.set_source_rgba(0.0, 0.96, 0.60, if selected { 0.28 } else { 0.14 });
        cr.arc(jx, jy, r, 0., std::f64::consts::TAU);
        let _ = cr.fill();

        // Border
        cr.set_source_rgb(0.0, 0.96, 0.60);
        cr.set_line_width(if selected { 3.0 } else { 1.8 });
        cr.arc(jx, jy, r, 0., std::f64::consts::TAU);
        let _ = cr.stroke();

        // Center Deadzone Circle
        cr.arc(jx, jy, 6., 0., std::f64::consts::TAU);
        let _ = cr.fill();

        // Direction labels
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.set_font_size(11.0);
        cr.move_to(jx - 4., jy - r + 14.);
        cr.show_text(&j.up);
        cr.move_to(jx - 4., jy + r - 5.);
        cr.show_text(&j.down);
        cr.move_to(jx - r + 5., jy + 4.);
        cr.show_text(&j.left);
        cr.move_to(jx + r - 14., jy + 4.);
        cr.show_text(&j.right);

        // Header label
        cr.set_source_rgb(0.0, 0.96, 0.60);
        cr.set_font_size(12.0);
        cr.move_to(jx - 24., jy - r - 6.);
        cr.show_text("🕹 WASD");
    }

    // Render Aim Crosshair
    if let Some(a) = &cfg.aim {
        let (ax, ay) = p(a.center_x, a.center_y);
        let selected = st.selected == Some(BindingRef::Aim);
        let (r, g, b) = marker_color(BindingRef::Aim);

        cr.set_source_rgba(r, g, b, if selected { 0.35 } else { 0.20 });
        cr.arc(ax, ay, 20., 0., std::f64::consts::TAU);
        let _ = cr.fill();

        cr.set_source_rgb(r, g, b);
        cr.set_line_width(if selected { 2.5 } else { 1.5 });
        cr.arc(ax, ay, 20., 0., std::f64::consts::TAU);
        let _ = cr.stroke();

        // Crosshair reticle lines
        cr.move_to(ax - 28., ay);
        cr.line_to(ax - 10., ay);
        cr.move_to(ax + 10., ay);
        cr.line_to(ax + 28., ay);
        cr.move_to(ax, ay - 28.);
        cr.line_to(ax, ay - 10.);
        cr.move_to(ax, ay + 10.);
        cr.line_to(ax, ay + 28.);
        let _ = cr.stroke();

        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.set_font_size(11.0);
        cr.move_to(ax + 24., ay + 4.);
        cr.show_text(&format!(
            "🎯 AIM [{}] ({:.1}x)",
            if a.mode == "relative" { "FPS" } else { "Touch" },
            a.sensitivity
        ));
    }

    // Helper to draw button markers
    let draw_marker = |sel: BindingRef, x: f32, y: f32, label: &str, tag: &str, slot: u8| {
        let (px, py) = p(x, y);
        let (r, g, b) = marker_color(sel);
        let selected = st.selected == Some(sel);
        let pill_w = 64.0;
        let pill_h = 24.0;
        let rx = px - pill_w / 2.;
        let ry = py - pill_h / 2.;

        // Halo when selected
        if selected {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
            cr.rectangle(rx - 4., ry - 4., pill_w + 8., pill_h + 8.);
            let _ = cr.fill();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(2.0);
            cr.rectangle(rx - 3., ry - 3., pill_w + 6., pill_h + 6.);
            let _ = cr.stroke();
        }

        // Main pill body
        cr.set_source_rgba(r, g, b, if selected { 0.90 } else { 0.75 });
        cr.rectangle(rx, ry, pill_w, pill_h);
        let _ = cr.fill();

        // Border
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(1.5);
        cr.rectangle(rx, ry, pill_w, pill_h);
        let _ = cr.stroke();

        // Key Name
        cr.set_source_rgb(0.05, 0.05, 0.08);
        cr.set_font_size(11.0);
        cr.move_to(rx + 6., ry + 16.);
        cr.show_text(label);

        // Tag & Slot below
        cr.set_source_rgb(0.9, 0.95, 1.0);
        cr.set_font_size(9.0);
        cr.move_to(rx, ry + pill_h + 11.);
        cr.show_text(&format!("{tag} [S{slot}]"));
    };

    for (i, x) in cfg.taps.iter().enumerate() {
        draw_marker(BindingRef::Tap(i), x.x, x.y, &x.key, "TAP", x.slot);
    }
    for (i, x) in cfg.holds.iter().enumerate() {
        draw_marker(BindingRef::Hold(i), x.x, x.y, &x.key, "HOLD", x.slot);
    }
    for (i, x) in cfg.mouse_taps.iter().enumerate() {
        draw_marker(
            BindingRef::MouseTap(i),
            x.x,
            x.y,
            &x.button.replace("MOUSE_", "M_"),
            "TAP",
            x.slot,
        );
    }
    for (i, x) in cfg.mouse_holds.iter().enumerate() {
        draw_marker(
            BindingRef::MouseHold(i),
            x.x,
            x.y,
            &x.button.replace("MOUSE_", "M_"),
            "FIRE/HOLD",
            x.slot,
        );
    }

    // Canvas footer instructions
    cr.set_source_rgb(0.55, 0.62, 0.75);
    cr.set_font_size(11.0);
    cr.move_to(ox, oy + vh + 18.);
    cr.show_text(
        "💡 Click to select • Drag to move • Double-click to add • Arrow keys to nudge • F8 toggles Lock",
    );
}

fn form_row(grid: &Grid, row: i32, label: &str, w: &impl gtk4::prelude::WidgetExt) {
    let l = Label::new(Some(label));
    l.set_halign(gtk4::Align::Start);
    l.add_css_class("form-label");
    grid.attach(&l, 0, row, 1, 1);
    grid.attach(w, 1, row, 1, 1);
}

fn fill_devices(combo: &ComboBoxText, selected: &Option<String>, mouse: bool) {
    combo.remove_all();
    combo.append(None, "(Auto-detect primary device)");

    let candidates: Vec<input::InputDeviceInfo> = input::list_input_devices()
        .into_iter()
        .filter(|d| if mouse { d.is_mouse } else { d.is_keyboard })
        .collect();

    for d in &candidates {
        let label = format!("{} — {}", d.name, d.path);
        combo.append(Some(&d.path), &label);
    }

    if let Some(s) = selected {
        let found = candidates.iter().any(|d| d.path == *s);
        if !found {
            combo.append(Some(s), &format!("⚠ Saved: {}", s));
        }
        if !combo.set_active_id(Some(s)) {
            combo.set_active(Some(0));
        }
    } else {
        combo.set_active(Some(0));
    }
}

fn make_spin(min: f64, max: f64, step: f64, digits: u32) -> SpinButton {
    let s = SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s
}

fn add_section(parent: &GtkBox, title: &str) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 6);
    outer.add_css_class("card");
    let l = Label::new(Some(title));
    l.add_css_class("section-title");
    l.set_halign(gtk4::Align::Start);
    outer.append(&l);
    parent.append(&outer);
    outer
}

fn rebuild_bindings(ui: &Ui) {
    while let Some(child) = ui.bindings_box.first_child() {
        ui.bindings_box.remove(&child);
    }
    let cfg = ui.state.borrow().cfg.clone();

    let mk_group = |title: &str| -> GtkBox {
        let b = GtkBox::new(Orientation::Vertical, 4);
        let l = Label::new(Some(title));
        l.add_css_class("section-title");
        l.set_halign(gtk4::Align::Start);
        b.append(&l);
        b
    };

    for (title, items, make_sel) in [
        ("Keyboard TAP", cfg.taps.len(), BindingRef::Tap as fn(usize) -> BindingRef),
        ("Keyboard HOLD", cfg.holds.len(), BindingRef::Hold as fn(usize) -> BindingRef),
        ("Mouse TAP", cfg.mouse_taps.len(), BindingRef::MouseTap as fn(usize) -> BindingRef),
        ("Mouse HOLD / FIRE", cfg.mouse_holds.len(), BindingRef::MouseHold as fn(usize) -> BindingRef),
    ] {
        if items == 0 {
            continue;
        }
        let group = mk_group(title);
        for i in 0..items {
            let sel = make_sel(i);
            let row = GtkBox::new(Orientation::Horizontal, 6);
            let text = Label::new(Some(&binding_label(sel, &cfg)));
            text.set_halign(gtk4::Align::Start);
            text.set_hexpand(true);
            let edit = Button::with_label("Edit");
            edit.add_css_class("btn-accent");
            let del = Button::with_label("✕");
            del.add_css_class("btn-danger");
            let ui2 = ui.clone();
            edit.connect_clicked(move |_| {
                open_binding_dialog(&ui2, Some(sel));
            });
            let ui3 = ui.clone();
            del.connect_clicked(move |_| {
                delete_binding(&ui3, sel);
            });
            row.append(&text);
            row.append(&edit);
            row.append(&del);
            group.append(&row);
        }
        ui.bindings_box.append(&group);
    }
    if let Some(j) = &cfg.joystick {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let text = Label::new(Some(&format!(
            "🕹 JOYSTICK • WASD • ({:.0}%, {:.0}%) • Slot {}",
            j.center_x * 100.,
            j.center_y * 100.,
            j.slot
        )));
        text.set_halign(gtk4::Align::Start);
        text.set_hexpand(true);
        let edit = Button::with_label("Edit");
        edit.add_css_class("btn-accent");
        let ui2 = ui.clone();
        edit.connect_clicked(move |_| {
            select_binding(&ui2, BindingRef::Joystick);
        });
        row.append(&text);
        row.append(&edit);
        ui.bindings_box.append(&row);
    }
    if let Some(a) = &cfg.aim {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let text = Label::new(Some(&format!(
            "🎯 AIM • {} • {} • {:.1}x • Slot {}",
            a.button, a.mode, a.sensitivity, a.slot
        )));
        text.set_halign(gtk4::Align::Start);
        text.set_hexpand(true);
        let edit = Button::with_label("Edit");
        edit.add_css_class("btn-accent");
        let ui2 = ui.clone();
        edit.connect_clicked(move |_| {
            select_binding(&ui2, BindingRef::Aim);
        });
        row.append(&text);
        row.append(&edit);
        ui.bindings_box.append(&row);
    }
    ui.bindings_box.queue_draw();
}

fn select_binding(ui: &Ui, sel: BindingRef) {
    ui.state.borrow_mut().selected = Some(sel);
    ui.canvas.queue_draw();
    set_status(
        ui,
        &format!("Selected: {}", binding_label(sel, &ui.state.borrow().cfg)),
    );
    update_selected_editor(ui);
}

fn update_selected_editor(ui: &Ui) {
    while let Some(child) = ui.selected_editor_box.first_child() {
        ui.selected_editor_box.remove(&child);
    }
    let st = ui.state.borrow();
    let Some(sel) = st.selected else {
        let hint = Label::new(Some("Click any control on the screen to view/edit details."));
        hint.set_wrap(true);
        hint.set_halign(gtk4::Align::Start);
        ui.selected_editor_box.append(&hint);
        return;
    };

    let title = Label::new(Some(&format!("Selected: {}", binding_label(sel, &st.cfg))));
    title.add_css_class("section-title");
    title.set_halign(gtk4::Align::Start);
    ui.selected_editor_box.append(&title);

    let g = Grid::new();
    g.set_row_spacing(6);
    g.set_column_spacing(8);

    if let Some((px, py)) = selected_position(&st.cfg, sel) {
        let x_spin = make_spin(0., 1., 0.01, 3);
        x_spin.set_value(px as f64);
        let y_spin = make_spin(0., 1., 0.01, 3);
        y_spin.set_value(py as f64);

        let ui_x = ui.clone();
        x_spin.connect_value_changed(move |s| {
            let mut state = ui_x.state.borrow_mut();
            if let Some(s_ref) = state.selected {
                let ny = selected_position(&state.cfg, s_ref).map(|p| p.1).unwrap_or(0.5);
                set_selected_position(&mut state.cfg, s_ref, s.value() as f32, ny);
                state.dirty = true;
                ui_x.canvas.queue_draw();
            }
        });

        let ui_y = ui.clone();
        y_spin.connect_value_changed(move |s| {
            let mut state = ui_y.state.borrow_mut();
            if let Some(s_ref) = state.selected {
                let nx = selected_position(&state.cfg, s_ref).map(|p| p.0).unwrap_or(0.5);
                set_selected_position(&mut state.cfg, s_ref, nx, s.value() as f32);
                state.dirty = true;
                ui_y.canvas.queue_draw();
            }
        });

        form_row(&g, 0, "X position", &x_spin);
        form_row(&g, 1, "Y position", &y_spin);
    }

    let del_btn = Button::with_label("🗑 Delete Selected");
    del_btn.add_css_class("btn-danger");
    let ui_del = ui.clone();
    del_btn.connect_clicked(move |_| {
        delete_binding(&ui_del, sel);
    });

    ui.selected_editor_box.append(&g);
    ui.selected_editor_box.append(&del_btn);
}

fn delete_binding(ui: &Ui, sel: BindingRef) {
    {
        let mut st = ui.state.borrow_mut();
        match sel {
            BindingRef::Tap(i) => {
                if i < st.cfg.taps.len() {
                    st.cfg.taps.remove(i);
                }
            }
            BindingRef::Hold(i) => {
                if i < st.cfg.holds.len() {
                    st.cfg.holds.remove(i);
                }
            }
            BindingRef::MouseTap(i) => {
                if i < st.cfg.mouse_taps.len() {
                    st.cfg.mouse_taps.remove(i);
                }
            }
            BindingRef::MouseHold(i) => {
                if i < st.cfg.mouse_holds.len() {
                    st.cfg.mouse_holds.remove(i);
                }
            }
            BindingRef::Aim => st.cfg.aim = None,
            BindingRef::Joystick => st.cfg.joystick = None,
        }
        st.selected = None;
        st.dirty = true;
    }
    rebuild_bindings(ui);
    sync_form(ui);
    update_selected_editor(ui);
    ui.canvas.queue_draw();
    set_status(ui, "Binding deleted — click Save to persist");
}

#[derive(Clone, Copy)]
enum EditType {
    KeyboardTap,
    KeyboardHold,
    MouseTap,
    MouseHold,
}

fn key_alias(name: &str) -> String {
    let n = name.to_ascii_uppercase();
    match n.as_str() {
        "SPACE" => "SPACE".into(),
        "RETURN" | "ENTER" => "ENTER".into(),
        "ESCAPE" | "ESC" => "ESC".into(),
        "TAB" => "TAB".into(),
        "SHIFT_L" | "SHIFT_R" | "SHIFT" => "SHIFT".into(),
        "CONTROL_L" | "CONTROL_R" | "CTRL" => "CTRL".into(),
        "ALT_L" | "ALT_R" | "ALT" => "ALT".into(),
        "BACKSPACE" => "BACKSPACE".into(),
        "DELETE" => "DELETE".into(),
        "INSERT" => "INSERT".into(),
        "HOME" => "HOME".into(),
        "END" => "END".into(),
        "PAGE_UP" => "PAGEUP".into(),
        "PAGE_DOWN" => "PAGEDOWN".into(),
        "UP" => "UP".into(),
        "DOWN" => "DOWN".into(),
        "LEFT" => "LEFT".into(),
        "RIGHT" => "RIGHT".into(),
        x => x.into(),
    }
}

fn attach_key_capture(entry: &Entry, button: &Button, status: &Label) {
    let armed = Rc::new(Cell::new(false));
    let a = armed.clone();
    let e = entry.clone();
    let s = status.clone();
    button.connect_clicked(move |_| {
        a.set(true);
        e.grab_focus();
        s.set_text("Press any key to capture…");
    });
    let controller = EventControllerKey::new();
    let a2 = armed.clone();
    let e2 = entry.clone();
    let s2 = status.clone();
    controller.connect_key_pressed(move |_, key, _, _| {
        if !a2.get() {
            return glib::Propagation::Proceed;
        }
        let name = key.name().map(|x| x.to_string()).unwrap_or_default();
        if !name.is_empty() {
            e2.set_text(&key_alias(&name));
            a2.set(false);
            s2.set_text("Key captured ✓");
        }
        glib::Propagation::Stop
    });
    entry.add_controller(controller);
}

fn attach_mouse_capture(entry: &Entry, button: &Button, status: &Label) {
    let armed = Rc::new(Cell::new(false));
    let a = armed.clone();
    let e = entry.clone();
    let s = status.clone();
    button.connect_clicked(move |_| {
        a.set(true);
        e.grab_focus();
        s.set_text("Click any mouse button to capture…");
    });

    let controller = GestureClick::new();
    controller.set_button(0);
    let a2 = armed.clone();
    let e2 = entry.clone();
    let s2 = status.clone();
    controller.connect_pressed(move |gesture, _, _, _| {
        if !a2.get() {
            return;
        }
        let token = match gesture.current_button() {
            1 => "MOUSE_LEFT",
            2 => "MOUSE_MIDDLE",
            3 => "MOUSE_RIGHT",
            8 => "MOUSE_SIDE",
            9 => "MOUSE_EXTRA",
            _ => return,
        };
        e2.set_text(token);
        a2.set(false);
        s2.set_text("Mouse button captured ✓");
    });
    entry.add_controller(controller);
}

fn open_binding_dialog(ui: &Ui, existing: Option<BindingRef>) {
    let kind = match existing {
        Some(BindingRef::Tap(_)) => EditType::KeyboardTap,
        Some(BindingRef::Hold(_)) => EditType::KeyboardHold,
        Some(BindingRef::MouseTap(_)) => EditType::MouseTap,
        Some(BindingRef::MouseHold(_)) => EditType::MouseHold,
        _ => return,
    };
    open_binding_dialog_inner(ui, existing, kind, 0.5, 0.5);
}

fn open_binding_dialog_inner(
    ui: &Ui,
    existing: Option<BindingRef>,
    kind: EditType,
    init_x: f32,
    init_y: f32,
) {
    let parent = ui
        .canvas
        .root()
        .and_then(|w| w.downcast::<ApplicationWindow>().ok());
    let Some(parent) = parent else { return };
    let dialog = Dialog::builder()
        .transient_for(&parent)
        .modal(true)
        .title(match kind {
            EditType::KeyboardTap => "Add Keyboard Tap",
            EditType::KeyboardHold => "Add Keyboard Hold",
            EditType::MouseTap => "Add Mouse Tap",
            EditType::MouseHold => "Add Mouse Hold / Fire",
        })
        .build();
    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    let save_btn = dialog.add_button("Save Binding", gtk4::ResponseType::Accept);
    save_btn.add_css_class("btn-primary");

    let box_ = GtkBox::new(Orientation::Vertical, 10);
    add_margins(&box_, 14);
    let grid = Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(10);

    let key_label = Label::new(Some(if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) {
        "Key"
    } else {
        "Mouse Button"
    }));
    key_label.set_halign(gtk4::Align::Start);
    let key_entry = Entry::new();
    let capture = Button::with_label("🎯 Capture Input");
    capture.add_css_class("btn-accent");
    let key_box = GtkBox::new(Orientation::Horizontal, 6);
    key_box.append(&key_entry);
    key_box.append(&capture);
    grid.attach(&key_label, 0, 0, 1, 1);
    grid.attach(&key_box, 1, 0, 1, 1);

    let x = make_spin(0., 1., 0.01, 3);
    let y = make_spin(0., 1., 0.01, 3);
    let slot = make_spin(0., 15., 1., 0);
    form_row(&grid, 1, "X Position (0..1)", &x);
    form_row(&grid, 2, "Y Position (0..1)", &y);
    form_row(&grid, 3, "Touch Slot (0..15)", &slot);
    box_.append(&grid);
    dialog.content_area().append(&box_);

    let cfg = ui.state.borrow().cfg.clone();
    if let Some(sel) = existing {
        match sel {
            BindingRef::Tap(i) => {
                if let Some(v) = cfg.taps.get(i) {
                    key_entry.set_text(&v.key);
                    x.set_value(v.x as f64);
                    y.set_value(v.y as f64);
                    slot.set_value(v.slot as f64);
                }
            }
            BindingRef::Hold(i) => {
                if let Some(v) = cfg.holds.get(i) {
                    key_entry.set_text(&v.key);
                    x.set_value(v.x as f64);
                    y.set_value(v.y as f64);
                    slot.set_value(v.slot as f64);
                }
            }
            BindingRef::MouseTap(i) => {
                if let Some(v) = cfg.mouse_taps.get(i) {
                    key_entry.set_text(&v.button);
                    x.set_value(v.x as f64);
                    y.set_value(v.y as f64);
                    slot.set_value(v.slot as f64);
                }
            }
            BindingRef::MouseHold(i) => {
                if let Some(v) = cfg.mouse_holds.get(i) {
                    key_entry.set_text(&v.button);
                    x.set_value(v.x as f64);
                    y.set_value(v.y as f64);
                    slot.set_value(v.slot as f64);
                }
            }
            _ => {}
        }
    } else {
        match kind {
            EditType::KeyboardTap | EditType::KeyboardHold => key_entry.set_text("SPACE"),
            EditType::MouseTap | EditType::MouseHold => key_entry.set_text("MOUSE_LEFT"),
        }
        x.set_value(init_x as f64);
        y.set_value(init_y as f64);
        slot.set_value(next_available_slot(&cfg) as f64);
    }

    let status = ui.status.clone();
    if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) {
        attach_key_capture(&key_entry, &capture, &status);
    } else {
        attach_mouse_capture(&key_entry, &capture, &status);
    }

    let ui2 = ui.clone();
    dialog.connect_response(move |d, response| {
        if response == gtk4::ResponseType::Accept {
            let token = key_entry.text().trim().to_string();
            let xx = x.value() as f32;
            let yy = y.value() as f32;
            let ss = slot.value() as u8;
            let result = if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) {
                input::key_code(&token).map(|_| ()).map_err(|e| e.to_string())
            } else {
                input::button_code(&token).map(|_| ()).map_err(|e| e.to_string())
            };
            match result {
                Ok(()) => {
                    let mut st = ui2.state.borrow_mut();
                    match (existing, kind) {
                        (Some(BindingRef::Tap(i)), EditType::KeyboardTap) => {
                            if let Some(v) = st.cfg.taps.get_mut(i) {
                                v.key = token.clone();
                                v.x = xx;
                                v.y = yy;
                                v.slot = ss;
                            }
                        }
                        (Some(BindingRef::Hold(i)), EditType::KeyboardHold) => {
                            if let Some(v) = st.cfg.holds.get_mut(i) {
                                v.key = token.clone();
                                v.x = xx;
                                v.y = yy;
                                v.slot = ss;
                            }
                        }
                        (Some(BindingRef::MouseTap(i)), EditType::MouseTap) => {
                            if let Some(v) = st.cfg.mouse_taps.get_mut(i) {
                                v.button = token.clone();
                                v.x = xx;
                                v.y = yy;
                                v.slot = ss;
                            }
                        }
                        (Some(BindingRef::MouseHold(i)), EditType::MouseHold) => {
                            if let Some(v) = st.cfg.mouse_holds.get_mut(i) {
                                v.button = token.clone();
                                v.x = xx;
                                v.y = yy;
                                v.slot = ss;
                            }
                        }
                        (None, EditType::KeyboardTap) => st.cfg.taps.push(Tap {
                            key: token.clone(),
                            x: xx,
                            y: yy,
                            slot: ss,
                        }),
                        (None, EditType::KeyboardHold) => st.cfg.holds.push(Hold {
                            key: token.clone(),
                            x: xx,
                            y: yy,
                            slot: ss,
                        }),
                        (None, EditType::MouseTap) => st.cfg.mouse_taps.push(MouseTap {
                            button: token.clone(),
                            x: xx,
                            y: yy,
                            slot: ss,
                        }),
                        (None, EditType::MouseHold) => st.cfg.mouse_holds.push(MouseHold {
                            button: token.clone(),
                            x: xx,
                            y: yy,
                            slot: ss,
                        }),
                        _ => {}
                    }
                    st.dirty = true;
                    drop(st);
                    rebuild_bindings(&ui2);
                    set_status(&ui2, "Binding updated — click Save to persist");
                    ui2.canvas.queue_draw();
                    d.close();
                }
                Err(e) => set_status(&ui2, &format!("Invalid binding: {e}")),
            }
        } else {
            d.close();
        }
    });
    dialog.show();
}

fn sync_form(ui: &Ui) {
    let st = ui.state.borrow();
    ui.profile_name.set_text(&display_name(&st.profile_path));
    ui.width.set_value(st.cfg.display.width as f64);
    ui.height.set_value(st.cfg.display.height as f64);
    ui.aim_enabled.set_active(st.cfg.aim.is_some());
    if let Some(a) = &st.cfg.aim {
        ui.aim_button.set_text(&a.button);
        ui.aim_mode.set_active_id(Some(a.mode.as_str()));
        ui.aim_x.set_value(a.center_x as f64);
        ui.aim_y.set_value(a.center_y as f64);
        ui.aim_sensitivity.set_value(a.sensitivity as f64);
        ui.aim_slot.set_value(a.slot as f64);
        ui.aim_invert_x.set_active(a.invert_x);
        ui.aim_invert_y.set_active(a.invert_y);
        ui.aim_scale_x.set_value(a.scale_x as f64);
        ui.aim_scale_y.set_value(a.scale_y as f64);
        ui.aim_edge_margin.set_value(a.edge_margin as f64);
    }
    ui.joy_enabled.set_active(st.cfg.joystick.is_some());
    if let Some(j) = &st.cfg.joystick {
        ui.joy_up.set_text(&j.up);
        ui.joy_down.set_text(&j.down);
        ui.joy_left.set_text(&j.left);
        ui.joy_right.set_text(&j.right);
        ui.joy_x.set_value(j.center_x as f64);
        ui.joy_y.set_value(j.center_y as f64);
        ui.joy_radius.set_value(j.radius as f64);
        ui.joy_slot.set_value(j.slot as f64);
        ui.joy_normalize.set_active(j.normalize_diagonal);
    }
    ui.grab.set_active(st.cfg.performance.grab);
    ui.realtime.set_active(st.cfg.performance.realtime);
    ui.realtime_priority
        .set_value(st.cfg.performance.realtime_priority as f64);
    ui.fifo_write_retries
        .set_value(st.cfg.performance.fifo_write_retries as f64);
    ui.fifo_write_wait
        .set_value(st.cfg.performance.fifo_write_wait_ms as f64);
    ui.fifo_reconnect
        .set_value(st.cfg.performance.fifo_reconnect_ms as f64);
    ui.touch_pressure
        .set_value(st.cfg.touch.pressure as f64);
    ui.touch_major.set_value(st.cfg.touch.major as f64);
    ui.touch_minor.set_value(st.cfg.touch.minor as f64);
    ui.mouse_lock.set_active(st.cfg.performance.mouse_lock);
    ui.mouse_toggle
        .set_text(&st.cfg.performance.mouse_toggle_key);
    fill_devices(&ui.keyboard, &st.cfg.devices.keyboard, false);
    fill_devices(&ui.mouse, &st.cfg.devices.mouse, true);
}

fn sync_state_from_form(ui: &Ui) {
    let mut st = ui.state.borrow_mut();
    let w = ui.width.value().round() as i32;
    let h = ui.height.value().round() as i32;
    st.cfg.display.width = w;
    st.cfg.display.height = h;
    st.cfg.devices.keyboard = ui.keyboard.active_id().map(|x| x.to_string());
    st.cfg.devices.mouse = ui.mouse.active_id().map(|x| x.to_string());
    st.cfg.performance.grab = ui.grab.is_active();
    st.cfg.performance.realtime = ui.realtime.is_active();
    st.cfg.performance.realtime_priority = ui.realtime_priority.value().round() as i32;
    st.cfg.performance.fifo_write_retries = ui.fifo_write_retries.value().round() as u8;
    st.cfg.performance.fifo_write_wait_ms = ui.fifo_write_wait.value().round() as u64;
    st.cfg.performance.fifo_reconnect_ms = ui.fifo_reconnect.value().round() as u64;
    st.cfg.performance.mouse_lock = ui.mouse_lock.is_active();
    st.cfg.performance.mouse_toggle_key = ui.mouse_toggle.text().trim().to_string();

    if ui.aim_enabled.is_active() {
        st.cfg.aim = Some(Aim {
            button: ui.aim_button.text().trim().to_string(),
            center_x: ui.aim_x.value() as f32,
            center_y: ui.aim_y.value() as f32,
            sensitivity: ui.aim_sensitivity.value() as f32,
            slot: ui.aim_slot.value() as u8,
            invert_x: ui.aim_invert_x.is_active(),
            invert_y: ui.aim_invert_y.is_active(),
            scale_x: ui.aim_scale_x.value() as f32,
            scale_y: ui.aim_scale_y.value() as f32,
            edge_margin: ui.aim_edge_margin.value() as f32,
            mode: ui
                .aim_mode
                .active_id()
                .map(|x| x.to_string())
                .unwrap_or_else(|| "relative".into()),
        });
    } else {
        st.cfg.aim = None;
    }

    if ui.joy_enabled.is_active() {
        st.cfg.joystick = Some(Joystick {
            up: ui.joy_up.text().trim().to_string(),
            down: ui.joy_down.text().trim().to_string(),
            left: ui.joy_left.text().trim().to_string(),
            right: ui.joy_right.text().trim().to_string(),
            center_x: ui.joy_x.value() as f32,
            center_y: ui.joy_y.value() as f32,
            radius: ui.joy_radius.value() as f32,
            normalize_diagonal: ui.joy_normalize.is_active(),
            slot: ui.joy_slot.value() as u8,
        });
    } else {
        st.cfg.joystick = None;
    }
    st.cfg.touch.pressure = ui.touch_pressure.value().round() as i32;
    st.cfg.touch.major = ui.touch_major.value().round() as i32;
    st.cfg.touch.minor = ui.touch_minor.value().round() as i32;
    st.dirty = true;
}

fn save_current(ui: &Ui) -> Result<(), String> {
    sync_state_from_form(ui);
    let name = ui.profile_name.text().trim().to_string();
    if name.is_empty() {
        return Err("Profile name cannot be empty".into());
    }
    let safe = safe_profile_name(&name).ok_or_else(|| "Invalid profile name".to_string())?;

    let mut st = ui.state.borrow_mut();
    let old_path = st.profile_path.clone();
    let new_path = profiles_dir().join(format!("{safe}.toml"));
    if new_path != old_path && new_path.exists() {
        return Err("A profile with that name already exists".into());
    }
    match save_profile(&new_path, &st.cfg) {
        Ok(()) => {
            if new_path != old_path {
                let _ = fs::remove_file(old_path);
            }
            st.profile_path = new_path;
            st.dirty = false;
            ui.profile_name.set_text(&safe);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

const USER_SERVICE: &str = "waydroid-keymapper.service";

fn user_bin_dir() -> PathBuf {
    home_dir().join(".local/bin")
}
fn user_service_dir() -> PathBuf {
    home_dir().join(".config/systemd/user")
}
fn daemon_install_path() -> PathBuf {
    user_bin_dir().join("waydroid-keymapper")
}
fn gui_install_path() -> PathBuf {
    user_bin_dir().join("keymapper-gui")
}
fn desktop_file_path() -> PathBuf {
    home_dir().join(".local/share/applications/waydroid-keymapper.desktop")
}

fn install_user_executable(src: &Path, dst: &Path) -> Result<(), String> {
    if src.canonicalize().ok() == dst.canonicalize().ok() {
        return Ok(());
    }
    let tmp = temp_path(dst, "install");
    fs::copy(src, &tmp).map_err(|e| format!("install {}: {e}", dst.display()))?;
    let mut perms = fs::metadata(&tmp).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmp, perms).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dst).map_err(|e| format!("activate {}: {e}", dst.display()))?;
    Ok(())
}

fn desktop_entry() -> String {
    let exe = gui_install_path()
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace(' ', "\\ ");
    format!(
        "[Desktop Entry]\nType=Application\nName=Waydroid Keymapper\nComment=Low-latency Waydroid keyboard and mouse profile editor\nExec={}\nIcon=input-gaming\nTerminal=false\nCategories=Utility;Game;\nKeywords=Waydroid;Android;Gaming;Keymapper;\n",
        exe
    )
}

fn udev_rules_text() -> &'static str {
    r#"SUBSYSTEM=="input", KERNEL=="event*", MODE="0660", GROUP="input", TAG+="uaccess"
"#
}

fn install_input_permissions() -> Result<String, String> {
    let dir = home_dir().join(".config/waydroid-keymapper");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = temp_path(&dir, "udev");
    fs::write(&tmp, udev_rules_text())
        .map_err(|e| format!("write temporary udev rules: {e}"))?;

    let install = Command::new("pkexec")
        .args([
            "install",
            "-Dm644",
            tmp.to_string_lossy().as_ref(),
            "/etc/udev/rules.d/99-waydroid-keymapper.rules",
        ])
        .output()
        .map_err(|e| format!("pkexec unavailable: {e}"))?;
    if !install.status.success() {
        let _ = fs::remove_file(&tmp);
        let err = String::from_utf8_lossy(&install.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "authentication cancelled or udev rule installation failed".into()
        } else {
            err
        });
    }

    let reload = Command::new("pkexec")
        .args(["udevadm", "control", "--reload-rules"])
        .output()
        .map_err(|e| format!("pkexec udevadm unavailable: {e}"))?;
    if !reload.status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&reload.stderr).trim().to_string());
    }

    let trigger = Command::new("pkexec")
        .args(["udevadm", "trigger", "--subsystem-match=input"])
        .output()
        .map_err(|e| format!("pkexec udevadm trigger unavailable: {e}"))?;
    let _ = fs::remove_file(&tmp);
    if !trigger.status.success() {
        return Err(String::from_utf8_lossy(&trigger.stderr).trim().to_string());
    }

    Ok("Input permissions repaired ✓".into())
}

fn daemon_source() -> Option<PathBuf> {
    let exe = env::current_exe().ok();
    let mut candidates = Vec::new();
    if let Some(e) = exe {
        if let Some(parent) = e.parent() {
            candidates.push(parent.join("waydroid-keymapper"));
        }
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("target/release/waydroid-keymapper"));
        candidates.push(cwd.join("waydroid-keymapper"));
    }
    candidates.push(daemon_install_path());
    for p in candidates {
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn service_unit() -> String {
    format!(
        r#"[Unit]
Description=Waydroid Rust Game Keymapper
After=graphical-session.target
Wants=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/waydroid-keymapper run %h/.config/waydroid-keymapper/config.toml
Restart=on-failure
RestartSec=1
Nice=0

[Install]
WantedBy=graphical-session.target
"#
    )
}

fn install_runtime() -> Result<String, String> {
    fs::create_dir_all(user_bin_dir()).map_err(|e| e.to_string())?;
    fs::create_dir_all(user_service_dir()).map_err(|e| e.to_string())?;

    let dst = daemon_install_path();
    if let Some(src) = daemon_source() {
        install_user_executable(&src, &dst)?;
    } else if !dst.is_file() {
        return Err(
            "waydroid-keymapper binary not found next to the GUI or in ~/.local/bin".into(),
        );
    }

    if let Ok(exe) = env::current_exe() {
        if exe.is_file() {
            install_user_executable(&exe, &gui_install_path())?;
        }
    }

    if !active_config_path().is_file() {
        let seed = profiles_dir().join("default.toml");
        if seed.is_file() {
            if let Some(parent) = active_config_path().parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let tmp = temp_path(&active_config_path(), "active");
            fs::copy(&seed, &tmp).map_err(|e| format!("seed active config: {e}"))?;
            fs::rename(&tmp, active_config_path())
                .map_err(|e| format!("activate default config: {e}"))?;
        }
    }

    if let Some(parent) = desktop_file_path().parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let desktop = desktop_file_path();
    let desktop_tmp = temp_path(&desktop, "desktop");
    fs::write(&desktop_tmp, desktop_entry())
        .map_err(|e| format!("write desktop launcher: {e}"))?;
    fs::rename(&desktop_tmp, &desktop)
        .map_err(|e| format!("activate desktop launcher: {e}"))?;

    let unit = user_service_dir().join(USER_SERVICE);
    let tmp = temp_path(&unit, "unit");
    fs::write(&tmp, service_unit()).map_err(|e| format!("write service: {e}"))?;
    fs::rename(&tmp, &unit).map_err(|e| format!("activate service: {e}"))?;

    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()
        .map_err(|e| format!("systemctl daemon-reload: {e}"))?;
    if !reload.status.success() {
        return Err(String::from_utf8_lossy(&reload.stderr).trim().to_string());
    }
    Ok("Runtime installed / repaired ✓".into())
}

fn systemctl_user(args: &[&str]) -> Result<String, String> {
    let out = Command::new("systemctl")
        .args(["--user"])
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        })
    }
}

fn service_action(action: &str) -> Result<String, String> {
    match action {
        "stop" => match systemctl_user(&["stop", USER_SERVICE]) {
            Ok(x) => Ok(x),
            Err(e) if e.contains("not loaded") || e.contains("not found") => Ok(String::new()),
            Err(e) => Err(e),
        },
        "start" => {
            install_runtime()?;
            systemctl_user(&["start", USER_SERVICE])
        }
        "restart" => {
            install_runtime()?;
            match systemctl_user(&["restart", USER_SERVICE]) {
                Ok(x) => Ok(x),
                Err(e) if e.contains("not loaded") || e.contains("not found") => {
                    systemctl_user(&["start", USER_SERVICE])
                }
                Err(e) => Err(e),
            }
        }
        "enable" => {
            install_runtime()?;
            systemctl_user(&["enable", USER_SERVICE])
        }
        "disable" => match systemctl_user(&["disable", "--now", USER_SERVICE]) {
            Ok(x) => Ok(x),
            Err(e) if e.contains("not loaded") || e.contains("not found") => Ok(String::new()),
            Err(e) => Err(e),
        },
        _ => Err("unknown service action".into()),
    }
}

fn runtime_service_state() -> String {
    let out = Command::new("systemctl")
        .args(["--user", "is-active", USER_SERVICE])
        .output();
    match out {
        Ok(o) if o.status.success() => "Running".into(),
        Ok(_) => {
            let failed = Command::new("systemctl")
                .args(["--user", "is-failed", USER_SERVICE])
                .output();
            if matches!(failed, Ok(ref x) if x.status.success()) {
                "Failed".into()
            } else if user_service_dir().join(USER_SERVICE).is_file() {
                "Stopped".into()
            } else {
                "Not installed".into()
            }
        }
        Err(_) => "systemctl unavailable".into(),
    }
}

fn run_background<F>(ui: &Ui, busy: &str, task: F)
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    let ui2 = ui.clone();
    set_status(ui, busy);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });
    glib::timeout_add_local(Duration::from_millis(50), move || {
        match rx.try_recv() {
            Ok(Ok(msg)) => {
                if msg.is_empty() {
                    set_status(&ui2, "Done ✓");
                } else {
                    set_status(&ui2, &msg);
                }
                update_runtime_status(&ui2);
                glib::ControlFlow::Break
            }
            Ok(Err(err)) => {
                set_status(&ui2, &err);
                update_runtime_status(&ui2);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                set_status(&ui2, "Background operation aborted");
                update_runtime_status(&ui2);
                glib::ControlFlow::Break
            }
        }
    });
}

fn update_runtime_status(ui: &Ui) {
    let svc = runtime_service_state();
    ui.runtime_status.set_text(&format!("Daemon: {svc}"));
    if svc == "Running" {
        ui.runtime_status.set_css_classes(&["status-badge", "status-green"]);
    } else if svc == "Stopped" {
        ui.runtime_status.set_css_classes(&["status-badge", "status-amber"]);
    } else {
        ui.runtime_status.set_css_classes(&["status-badge", "status-red"]);
    }

    let wd = waydroid_state();
    ui.waydroid_status.set_text(&format!("Waydroid: {wd}"));
    if wd.contains("RUNNING") || wd == "Running" {
        ui.waydroid_status.set_css_classes(&["status-badge", "status-green"]);
    } else {
        ui.waydroid_status.set_css_classes(&["status-badge", "status-amber"]);
    }

    match control::request("status") {
        Ok(reply) => {
            let locked = reply
                .split_whitespace()
                .find_map(|x| x.strip_prefix("locked="))
                .unwrap_or("0");
            let requested = reply
                .split_whitespace()
                .find_map(|x| x.strip_prefix("requested="))
                .unwrap_or(locked);
            let running = reply
                .split_whitespace()
                .find_map(|x| x.strip_prefix("running="))
                .unwrap_or("1");
            if running == "1" {
                if requested == "1" && locked == "1" {
                    ui.lock_status.set_text("🔒 Gaming Mode (Locked)");
                    ui.lock_status.set_css_classes(&["status-badge", "status-green"]);
                } else if requested == "1" || locked == "1" {
                    ui.lock_status.set_text("🔄 Toggling Mouse…");
                    ui.lock_status.set_css_classes(&["status-badge", "status-amber"]);
                } else {
                    ui.lock_status.set_text("🖱 Desktop Mode (Unlocked)");
                    ui.lock_status.set_css_classes(&["status-badge", "status-amber"]);
                }
            } else {
                ui.lock_status.set_text("Mouse: Offline");
                ui.lock_status.set_css_classes(&["status-badge", "status-red"]);
            }
        }
        Err(_) => {
            ui.lock_status.set_text("Mouse: Offline");
            ui.lock_status.set_css_classes(&["status-badge", "status-red"]);
        }
    }
}

fn diagnostics(ui: &Ui) {
    sync_state_from_form(ui);
    let cfg = ui.state.borrow().cfg.clone();
    let mut issues = Vec::new();

    if !cfg.conflicts().is_empty() {
        issues.push(format!("{} input conflict(s)", cfg.conflicts().len()));
    }
    if let Err(e) = cfg.validate() {
        issues.push(format!("Config: {e}"));
    }

    if let Some(k) = cfg.devices.keyboard.clone() {
        if let Err(e) = evdev::Device::open(&k) {
            issues.push(format!("Keyboard access: {e}"));
        }
    }

    if let Some(m) = cfg.devices.mouse.clone() {
        if let Err(e) = evdev::Device::open(&m) {
            issues.push(format!("Mouse access: {e}"));
        }
    }

    let touch = cfg.touch_fifo();
    if !Path::new(&touch).exists() {
        issues.push(format!("Touch FIFO not found: {touch}"));
    }

    match control::request("ping") {
        Ok(reply) if reply == "OK pong" => {}
        Ok(reply) => issues.push(format!("Daemon: {reply}")),
        Err(_) => issues.push("Daemon control socket offline".into()),
    }

    if issues.is_empty() {
        set_status(ui, "Diagnostics: All systems OK ✓ (Zero issues found)");
    } else {
        set_status(
            ui,
            &format!("Diagnostics: {} issue(s) • {}", issues.len(), issues.join(" | ")),
        );
    }
    update_runtime_status(ui);
}

fn runtime_control(ui: &Ui, command: &str) {
    match control::request(command) {
        Ok(reply) => {
            set_status(ui, &format!("Daemon: {reply}"));
            let ui2 = ui.clone();
            glib::timeout_add_local_once(Duration::from_millis(60), move || {
                update_runtime_status(&ui2);
            });
        }
        Err(e) => set_status(ui, &format!("Daemon control unavailable: {e}")),
    }
    update_runtime_status(ui);
}

fn waydroid_action(ui: &Ui, action: &str) {
    let action = action.to_string();
    run_background(ui, &format!("Waydroid {action} in progress…"), move || {
        match Command::new("waydroid").args(["session", &action]).output() {
            Ok(out) if out.status.success() => {
                let msg = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .replace('\n', " • ");
                Ok(if msg.is_empty() {
                    format!("Waydroid {action}: OK ✓")
                } else {
                    format!("Waydroid {action}: {msg}")
                })
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .trim()
                    .replace('\n', " • ");
                let err = if err.is_empty() {
                    String::from_utf8_lossy(&out.stdout).trim().to_string()
                } else {
                    err
                };
                Err(format!("Waydroid {action} failed: {err}"))
            }
            Err(e) => Err(format!("Waydroid command failed: {e}")),
        }
    });
}

fn waydroid_state() -> String {
    match Command::new("waydroid").arg("status").output() {
        Ok(o) => {
            let x = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if x.contains("RUNNING") || x.contains("Session:        RUNNING") {
                "Running".into()
            } else if x.contains("STOPPED") {
                "Stopped".into()
            } else if x.is_empty() {
                "Unknown".into()
            } else {
                x.replace('\n', " • ")
            }
        }
        Err(_) => "Unavailable".into(),
    }
}

fn apply_and_run(ui: &Ui) {
    sync_state_from_form(ui);
    {
        let st = ui.state.borrow();
        if let Err(e) = st.cfg.validate_runtime() {
            set_status(ui, &format!("Cannot run: {e}"));
            return;
        }
    }
    if let Err(e) = save_current(ui) {
        set_status(ui, &format!("Save failed: {e}"));
        return;
    }
    if let Err(e) = install_runtime() {
        set_status(ui, &format!("Runtime setup failed: {e}"));
        return;
    }

    let profile_path = ui.state.borrow().profile_path.clone();
    let active = active_config_path();
    let data = match fs::read_to_string(&profile_path) {
        Ok(x) => x,
        Err(e) => {
            set_status(ui, &format!("Read profile failed: {e}"));
            return;
        }
    };
    if let Some(parent) = active.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = active.with_extension("toml.tmp");
    if let Err(e) = fs::write(&tmp, data) {
        set_status(ui, &format!("Write active config failed: {e}"));
        return;
    }
    if let Err(e) = fs::rename(&tmp, &active) {
        let _ = fs::remove_file(&tmp);
        set_status(ui, &format!("Activate config failed: {e}"));
        return;
    }

    match service_action("restart") {
        Ok(_) => set_status(
            ui,
            "🚀 Profile applied & daemon started! Press F8 in game to Lock mouse.",
        ),
        Err(e) => set_status(ui, &format!("Profile applied; daemon restart failed: {e}")),
    }
    rebuild_profiles(ui);
    update_runtime_status(ui);
}

fn rebuild_profiles(ui: &Ui) {
    while let Some(child) = ui.profile_list.first_child() {
        ui.profile_list.remove(&child);
    }
    let files = profile_files();
    let current = ui.state.borrow().profile_path.clone();
    let mut selected = None;
    for p in files {
        let row = ListBoxRow::new();
        let label = Label::new(Some(&display_name(&p)));
        label.set_xalign(0.);
        add_margins(&label, 6);
        row.set_child(Some(&label));
        if p == current {
            selected = Some(row.clone());
        }
        ui.profile_list.append(&row);
    }
    if let Some(row) = selected {
        ui.profile_list.select_row(Some(&row));
    }
}

fn load_selected_profile(ui: &Ui, row: &ListBoxRow) {
    let index = row.index();
    let files = profile_files();
    let Some(path) = files.get(index as usize) else {
        return;
    };
    match load_profile(path) {
        Ok(cfg) => {
            let mut st = ui.state.borrow_mut();
            st.cfg = cfg;
            st.profile_path = path.clone();
            st.selected = None;
            st.dirty = false;
            drop(st);
            sync_form(ui);
            rebuild_bindings(ui);
            update_selected_editor(ui);
            ui.canvas.queue_draw();
            set_status(ui, &format!("Loaded profile '{}'", display_name(path)));
        }
        Err(e) => set_status(ui, &format!("Load failed: {e}")),
    }
}

fn ask_name(
    parent: &ApplicationWindow,
    title: &str,
    initial: &str,
    callback: impl Fn(String) + 'static,
) {
    let dialog = Dialog::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .build();
    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    let ok = dialog.add_button("OK", gtk4::ResponseType::Accept);
    ok.add_css_class("btn-primary");
    let entry = Entry::new();
    entry.set_text(initial);
    entry.set_activates_default(true);
    add_margins(&entry, 14);
    dialog.content_area().append(&entry);
    dialog.connect_response(move |d, r| {
        if r == gtk4::ResponseType::Accept {
            callback(entry.text().trim().to_string());
        }
        d.close();
    });
    dialog.show();
}

fn new_profile(ui: &Ui, app: &ApplicationWindow) {
    let ui2 = ui.clone();
    ask_name(app, "Create New Profile", "freefire", move |name| {
        let Some(safe) = safe_profile_name(&name) else {
            set_status(&ui2, "Invalid profile name");
            return;
        };
        let path = profiles_dir().join(format!("{safe}.toml"));
        if path.exists() {
            set_status(&ui2, "A profile with that name already exists");
            return;
        }
        let cfg = default_config();
        match save_profile(&path, &cfg) {
            Ok(()) => {
                let mut st = ui2.state.borrow_mut();
                st.cfg = cfg;
                st.profile_path = path;
                st.selected = None;
                st.dirty = false;
                drop(st);
                sync_form(&ui2);
                rebuild_bindings(&ui2);
                rebuild_profiles(&ui2);
                update_selected_editor(&ui2);
                ui2.canvas.queue_draw();
                set_status(&ui2, "New profile created ✓");
            }
            Err(e) => set_status(&ui2, &format!("Create failed: {e}")),
        }
    });
}

fn duplicate_profile(ui: &Ui, app: &ApplicationWindow) {
    sync_state_from_form(ui);
    let current = ui.state.borrow().profile_path.clone();
    let ui2 = ui.clone();
    ask_name(
        app,
        "Duplicate Profile",
        &format!("{}_copy", display_name(&current)),
        move |name| {
            let Some(safe) = safe_profile_name(&name) else {
                set_status(&ui2, "Invalid profile name");
                return;
            };
            let path = profiles_dir().join(format!("{safe}.toml"));
            if path.exists() {
                set_status(&ui2, "Profile already exists");
                return;
            }
            let cfg = ui2.state.borrow().cfg.clone();
            match save_profile(&path, &cfg) {
                Ok(()) => {
                    let mut st = ui2.state.borrow_mut();
                    st.cfg = cfg;
                    st.profile_path = path;
                    st.selected = None;
                    st.dirty = false;
                    drop(st);
                    sync_form(&ui2);
                    rebuild_bindings(&ui2);
                    rebuild_profiles(&ui2);
                    update_selected_editor(&ui2);
                    ui2.canvas.queue_draw();
                    set_status(&ui2, "Profile duplicated ✓");
                }
                Err(e) => set_status(&ui2, &format!("Duplicate failed: {e}")),
            }
        },
    );
}

fn delete_profile(ui: &Ui) {
    let current = ui.state.borrow().profile_path.clone();
    let files = profile_files();
    if files.len() <= 1 {
        set_status(ui, "Cannot delete the last remaining profile");
        return;
    }
    match fs::remove_file(&current) {
        Ok(()) => {
            let remaining = profile_files();
            let next = remaining.first().cloned().unwrap();
            match load_profile(&next) {
                Ok(cfg) => {
                    let mut st = ui.state.borrow_mut();
                    st.cfg = cfg;
                    st.profile_path = next;
                    st.selected = None;
                    st.dirty = false;
                    drop(st);
                    rebuild_profiles(ui);
                    sync_form(ui);
                    rebuild_bindings(ui);
                    update_selected_editor(ui);
                    ui.canvas.queue_draw();
                    set_status(ui, "Profile deleted");
                }
                Err(e) => set_status(ui, &format!("Load replacement failed: {e}")),
            }
        }
        Err(e) => set_status(ui, &format!("Delete failed: {e}")),
    }
}

fn validate_current(ui: &Ui) {
    sync_state_from_form(ui);
    let st = ui.state.borrow();
    let conflicts = st.cfg.conflicts();
    if !conflicts.is_empty() {
        set_status(
            ui,
            &format!("{} conflict(s): {}", conflicts.len(), conflicts.join(" | ")),
        );
        return;
    }
    match st.cfg.validate() {
        Ok(()) => set_status(ui, "Configuration is 100% valid ✓"),
        Err(e) => set_status(ui, &format!("Validation error: {e}")),
    }
}

fn build_ui(app: &Application) {
    if let Some(display) = GdkDisplay::default() {
        let provider = CssProvider::new();
        provider.load_from_data(APP_CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let dir = ensure_profiles().unwrap_or_else(|_| profiles_dir());
    let current = profile_files()
        .first()
        .cloned()
        .unwrap_or_else(|| dir.join("default.toml"));
    let cfg = load_profile(&current).unwrap_or_else(|_| default_config());
    let state = Rc::new(RefCell::new(State{
        cfg,
        profile_path: current,
        selected: None,
        dirty: false,
    }));

    let profile_list = ListBox::new();
    profile_list.set_selection_mode(gtk4::SelectionMode::Single);
    profile_list.set_vexpand(true);

    let canvas = DrawingArea::new();
    canvas.set_content_width(820);
    canvas.set_content_height(580);
    canvas.set_hexpand(true);
    canvas.set_vexpand(true);

    let status = Label::new(Some("Ready"));
    status.set_halign(gtk4::Align::Start);
    status.set_hexpand(true);
    add_margins(&status, 8);

    let profile_name = Entry::new();
    let width = make_spin(320., 16384., 1., 0);
    let height = make_spin(240., 16384., 1., 0);
    let keyboard = ComboBoxText::new();
    let mouse = ComboBoxText::new();

    let aim_enabled = CheckButton::with_label("Enable Aim / Camera Look");
    let aim_button = Entry::new();
    let aim_mode = ComboBoxText::new();
    aim_mode.append(Some("relative"), "Relative / FPS Mouse Aim (Unbounded)");
    aim_mode.append(Some("touch"), "Touch / Absolute Aim");
    let aim_x = make_spin(0., 1., 0.01, 3);
    let aim_y = make_spin(0., 1., 0.01, 3);
    let aim_sensitivity = make_spin(0.01, 20., 0.05, 2);
    let aim_slot = make_spin(0., 15., 1., 0);
    let aim_invert_x = CheckButton::with_label("Invert X Axis");
    let aim_invert_y = CheckButton::with_label("Invert Y Axis");
    let aim_scale_x = make_spin(0.01, 20., 0.05, 2);
    let aim_scale_y = make_spin(0.01, 20., 0.05, 2);
    let aim_edge_margin = make_spin(0., 0.49, 0.01, 2);

    let joy_enabled = CheckButton::with_label("Enable Analog Joystick (WASD)");
    let joy_up = Entry::new();
    let joy_down = Entry::new();
    let joy_left = Entry::new();
    let joy_right = Entry::new();
    let joy_x = make_spin(0., 1., 0.01, 3);
    let joy_y = make_spin(0., 1., 0.01, 3);
    let joy_radius = make_spin(0.01, 1., 0.005, 3);
    let joy_slot = make_spin(0., 15., 1., 0);
    let joy_normalize = CheckButton::with_label("Normalize Diagonal Speed");

    let grab = CheckButton::with_label("Exclusive Input Grab (Gaming Capture)");
    let realtime = CheckButton::with_label("Realtime Priority (SCHED_FIFO)");
    let realtime_priority = make_spin(1., 99., 1., 0);
    let fifo_write_retries = make_spin(1., 8., 1., 0);
    let fifo_write_wait = make_spin(0., 5., 1., 0);
    let fifo_reconnect = make_spin(5., 2000., 5., 0);
    let touch_pressure = make_spin(1., 255., 1., 0);
    let touch_major = make_spin(1., 255., 1., 0);
    let touch_minor = make_spin(1., 255., 1., 0);

    let mouse_lock = CheckButton::with_label("Lock mouse on daemon start");
    let mouse_toggle = Entry::new();
    mouse_toggle.set_text("F8");

    let runtime_status = Label::new(Some("Daemon: Checking…"));
    let lock_status = Label::new(Some("Mouse: Offline"));
    let waydroid_status = Label::new(Some("Waydroid: Checking…"));
    let input_access = Label::new(Some("Checking device access…"));

    runtime_status.add_css_class("status-badge");
    lock_status.add_css_class("status-badge");
    waydroid_status.add_css_class("status-badge");

    let bindings_box = GtkBox::new(Orientation::Vertical, 6);
    let selected_editor_box = GtkBox::new(Orientation::Vertical, 6);

    let ui = Ui {
        state: state.clone(),
        profile_list: profile_list.clone(),
        bindings_box: bindings_box.clone(),
        canvas: canvas.clone(),
        status: status.clone(),
        profile_name: profile_name.clone(),
        width: width.clone(),
        height: height.clone(),
        keyboard: keyboard.clone(),
        mouse: mouse.clone(),
        aim_enabled: aim_enabled.clone(),
        aim_button: aim_button.clone(),
        aim_mode: aim_mode.clone(),
        aim_x: aim_x.clone(),
        aim_y: aim_y.clone(),
        aim_sensitivity: aim_sensitivity.clone(),
        aim_slot: aim_slot.clone(),
        aim_invert_x: aim_invert_x.clone(),
        aim_invert_y: aim_invert_y.clone(),
        aim_scale_x: aim_scale_x.clone(),
        aim_scale_y: aim_scale_y.clone(),
        aim_edge_margin: aim_edge_margin.clone(),
        joy_enabled: joy_enabled.clone(),
        joy_up: joy_up.clone(),
        joy_down: joy_down.clone(),
        joy_left: joy_left.clone(),
        joy_right: joy_right.clone(),
        joy_x: joy_x.clone(),
        joy_y: joy_y.clone(),
        joy_radius: joy_radius.clone(),
        joy_slot: joy_slot.clone(),
        joy_normalize: joy_normalize.clone(),
        grab: grab.clone(),
        realtime: realtime.clone(),
        realtime_priority: realtime_priority.clone(),
        fifo_write_retries: fifo_write_retries.clone(),
        fifo_write_wait: fifo_write_wait.clone(),
        fifo_reconnect: fifo_reconnect.clone(),
        touch_pressure: touch_pressure.clone(),
        touch_major: touch_major.clone(),
        touch_minor: touch_minor.clone(),
        mouse_lock: mouse_lock.clone(),
        mouse_toggle: mouse_toggle.clone(),
        runtime_status: runtime_status.clone(),
        lock_status: lock_status.clone(),
        waydroid_status: waydroid_status.clone(),
        input_access: input_access.clone(),
        selected_editor_box: selected_editor_box.clone(),
    };

    let root = GtkBox::new(Orientation::Vertical, 0);

    // Top Header & Controls
    let top_bar = GtkBox::new(Orientation::Horizontal, 8);
    add_margins(&top_bar, 8);

    let title_box = GtkBox::new(Orientation::Vertical, 2);
    let title = Label::new(Some("🎮 Waydroid Keymapper"));
    title.add_css_class("title-2");
    title.set_halign(gtk4::Align::Start);
    let sub = Label::new(Some("Ultra Low-Latency Touch & Mouse Mapper for Waydroid"));
    sub.set_halign(gtk4::Align::Start);
    title_box.append(&title);
    title_box.append(&sub);
    top_bar.append(&title_box);

    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    top_bar.append(&spacer);

    // Live Status Badges
    top_bar.append(&runtime_status);
    top_bar.append(&lock_status);
    top_bar.append(&waydroid_status);

    // Top action buttons
    let save_btn = Button::with_label("💾 Save");
    save_btn.add_css_class("btn-accent");
    let apply_btn = Button::with_label("🚀 Apply & Run");
    apply_btn.add_css_class("btn-success");
    let toggle_lock_btn = Button::with_label("🔒 Toggle Lock (F8)");
    toggle_lock_btn.add_css_class("btn-primary");
    let diag_btn = Button::with_label("🔍 Diagnostics");
    diag_btn.add_css_class("btn-accent");

    top_bar.append(&save_btn);
    top_bar.append(&apply_btn);
    top_bar.append(&toggle_lock_btn);
    top_bar.append(&diag_btn);

    root.append(&top_bar);
    root.append(&Separator::new(Orientation::Horizontal));

    // Three-Panel Split
    let left = GtkBox::new(Orientation::Vertical, 6);
    add_margins(&left, 6);
    left.set_size_request(240, -1);

    // Profiles Card
    let prof_sec = add_section(&left, "Profiles");
    let prof_btn_box = GtkBox::new(Orientation::Horizontal, 4);
    let new_btn = Button::with_label("+ New");
    new_btn.add_css_class("btn-accent");
    let dup_btn = Button::with_label("📋 Copy");
    dup_btn.add_css_class("btn-accent");
    let del_btn = Button::with_label("🗑 Delete");
    del_btn.add_css_class("btn-danger");
    prof_btn_box.append(&new_btn);
    prof_btn_box.append(&dup_btn);
    prof_btn_box.append(&del_btn);
    prof_sec.append(&prof_btn_box);

    let profile_scroll = ScrolledWindow::new();
    profile_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    profile_scroll.set_child(Some(&profile_list));
    profile_scroll.set_min_content_height(140);
    prof_sec.append(&profile_scroll);

    // Presets Card
    let presets = add_section(&left, "Shooter Presets");
    let preset_grid = Grid::new();
    preset_grid.set_row_spacing(6);
    preset_grid.set_column_spacing(6);
    let free_fire_btn = Button::with_label("🔥 Free Fire");
    free_fire_btn.add_css_class("btn-accent");
    let pubg_btn = Button::with_label("🎯 PUBG Mobile");
    pubg_btn.add_css_class("btn-accent");
    let fps_btn = Button::with_label("🎮 FPS Standard");
    fps_btn.add_css_class("btn-accent");
    let minimal_btn = Button::with_label("⚡ Minimal");
    minimal_btn.add_css_class("btn-accent");

    preset_grid.attach(&free_fire_btn, 0, 0, 1, 1);
    preset_grid.attach(&pubg_btn, 1, 0, 1, 1);
    preset_grid.attach(&fps_btn, 0, 1, 1, 1);
    preset_grid.attach(&minimal_btn, 1, 1, 1, 1);
    presets.append(&preset_grid);

    // Service & Waydroid Controls
    let svc_sec = add_section(&left, "Daemon & Waydroid");
    let svc_b1 = GtkBox::new(Orientation::Horizontal, 4);
    let start_btn = Button::with_label("▶ Start");
    start_btn.add_css_class("btn-accent");
    let stop_btn = Button::with_label("⏹ Stop");
    stop_btn.add_css_class("btn-danger");
    let restart_btn = Button::with_label("🔄 Restart");
    restart_btn.add_css_class("btn-accent");
    svc_b1.append(&start_btn);
    svc_b1.append(&stop_btn);
    svc_b1.append(&restart_btn);
    svc_sec.append(&svc_b1);

    let wd_b = GtkBox::new(Orientation::Horizontal, 4);
    let wd_start = Button::with_label("Start Waydroid");
    wd_start.add_css_class("btn-accent");
    let wd_stop = Button::with_label("Stop Waydroid");
    wd_stop.add_css_class("btn-danger");
    wd_b.append(&wd_start);
    wd_b.append(&wd_stop);
    svc_sec.append(&wd_b);

    let fix_input = Button::with_label("🔑 Repair Input Permissions");
    fix_input.add_css_class("btn-accent");
    svc_sec.append(&fix_input);

    // Bindings List Card
    let bindings_sec = add_section(&left, "Configured Controls");
    let addbar = GtkBox::new(Orientation::Horizontal, 4);
    let add_key_tap = Button::with_label("+ Key TAP");
    add_key_tap.add_css_class("btn-accent");
    let add_key_hold = Button::with_label("+ Key HOLD");
    add_key_hold.add_css_class("btn-accent");
    let add_mouse_hold = Button::with_label("+ Fire / Click");
    add_mouse_hold.add_css_class("btn-accent");
    addbar.append(&add_key_tap);
    addbar.append(&add_key_hold);
    addbar.append(&add_mouse_hold);
    bindings_sec.append(&addbar);

    let bindings_scroll = ScrolledWindow::new();
    bindings_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    bindings_scroll.set_child(Some(&bindings_box));
    bindings_scroll.set_vexpand(true);
    bindings_scroll.set_min_content_height(180);
    bindings_sec.append(&bindings_scroll);

    // Center Panel (Visual Canvas)
    let center = GtkBox::new(Orientation::Vertical, 4);
    add_margins(&center, 6);
    center.append(&canvas);

    let bottom_bar = GtkBox::new(Orientation::Horizontal, 8);
    add_margins(&bottom_bar, 4);
    bottom_bar.append(&status);
    center.append(&bottom_bar);

    // Right Panel (Properties & Hardware)
    let right = GtkBox::new(Orientation::Vertical, 6);
    add_margins(&right, 6);
    right.set_size_request(280, -1);

    let settings_scroll = ScrolledWindow::new();
    settings_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    settings_scroll.set_child(Some(&right));
    settings_scroll.set_vexpand(true);

    // Selected Item Editor Card
    let sel_sec = add_section(&right, "Selected Control");
    sel_sec.append(&selected_editor_box);

    // Aim Settings Card
    let aim_sec = add_section(&right, "Aim / Camera Control");
    aim_sec.append(&aim_enabled);
    let ag = Grid::new();
    ag.set_row_spacing(6);
    ag.set_column_spacing(8);
    form_row(&ag, 0, "Trigger", &aim_button);
    form_row(&ag, 1, "Mode", &aim_mode);
    form_row(&ag, 2, "Center X", &aim_x);
    form_row(&ag, 3, "Center Y", &aim_y);
    form_row(&ag, 4, "Sensitivity", &aim_sensitivity);
    form_row(&ag, 5, "Touch Slot", &aim_slot);
    form_row(&ag, 6, "Scale X", &aim_scale_x);
    form_row(&ag, 7, "Scale Y", &aim_scale_y);
    ag.attach(&aim_invert_x, 0, 8, 2, 1);
    ag.attach(&aim_invert_y, 0, 9, 2, 1);
    aim_sec.append(&ag);

    // Joystick Settings Card
    let joy_sec = add_section(&right, "Joystick (WASD)");
    joy_sec.append(&joy_enabled);
    let jg = Grid::new();
    jg.set_row_spacing(6);
    jg.set_column_spacing(8);
    form_row(&jg, 0, "Up Key", &joy_up);
    form_row(&jg, 1, "Down Key", &joy_down);
    form_row(&jg, 2, "Left Key", &joy_left);
    form_row(&jg, 3, "Right Key", &joy_right);
    form_row(&jg, 4, "Center X", &joy_x);
    form_row(&jg, 5, "Center Y", &joy_y);
    form_row(&jg, 6, "Radius", &joy_radius);
    form_row(&jg, 7, "Touch Slot", &joy_slot);
    jg.attach(&joy_normalize, 0, 8, 2, 1);
    joy_sec.append(&jg);

    // Devices & Display Card
    let dev_sec = add_section(&right, "Input Devices & Screen");
    let dg = Grid::new();
    dg.set_row_spacing(6);
    dg.set_column_spacing(8);
    form_row(&dg, 0, "Keyboard", &keyboard);
    form_row(&dg, 1, "Mouse", &mouse);
    form_row(&dg, 2, "Screen Width", &width);
    form_row(&dg, 3, "Screen Height", &height);
    form_row(&dg, 4, "Toggle Key", &mouse_toggle);
    dev_sec.append(&dg);

    let refresh_dev = Button::with_label("🔄 Refresh Devices");
    refresh_dev.add_css_class("btn-accent");
    dev_sec.append(&refresh_dev);

    // Performance & Fine Tuning
    let perf_sec = add_section(&right, "Performance & Timings");
    perf_sec.append(&grab);
    perf_sec.append(&realtime);
    perf_sec.append(&mouse_lock);

    // Assemble Split Panes
    let paned_left = Paned::new(Orientation::Horizontal);
    paned_left.set_start_child(Some(&left));
    paned_left.set_end_child(Some(&center));
    paned_left.set_position(300);

    let main_paned = Paned::new(Orientation::Horizontal);
    main_paned.set_start_child(Some(&paned_left));
    main_paned.set_end_child(Some(&settings_scroll));
    main_paned.set_position(1180);

    root.append(&main_paned);

    let app_window = ApplicationWindow::builder()
        .application(app)
        .title("Waydroid Keymapper")
        .default_width(1520)
        .default_height(880)
        .build();
    app_window.set_child(Some(&root));

    // Canvas Draw Callback
    canvas.set_draw_func({
        let st = state.clone();
        move |area, cr, w, h| draw_canvas(&st, area, cr, w, h)
    });

    // Canvas Single Click (Select)
    let click = GestureClick::new();
    let ui_click = ui.clone();
    click.connect_released(move |_, _, x, y| {
        let st = ui_click.state.borrow();
        let pad = 16.;
        let cw = (ui_click.canvas.width() as f64 - 2. * pad).max(10.);
        let ch = (ui_click.canvas.height() as f64 - 2. * pad).max(10.);
        let scale = (cw / st.cfg.display.width.max(1) as f64)
            .min(ch / st.cfg.display.height.max(1) as f64);
        let vw = st.cfg.display.width as f64 * scale;
        let vh = st.cfg.display.height as f64 * scale;
        let ox = (ui_click.canvas.width() as f64 - vw) / 2.;
        let oy = (ui_click.canvas.height() as f64 - vh) / 2.;
        let nx = ((x - ox) / vw).clamp(0., 1.) as f32;
        let ny = ((y - oy) / vh).clamp(0., 1.) as f32;
        let sel = nearest_binding(&st.cfg, nx, ny);
        drop(st);
        if let Some(s) = sel {
            select_binding(&ui_click, s);
        }
    });
    canvas.add_controller(click);

    // Canvas Double Click (Add binding at position)
    let dclick = GestureClick::new();
    let ui_dclick = ui.clone();
    dclick.connect_pressed(move |gesture, n_press, x, y| {
        if n_press == 2 {
            let st = ui_dclick.state.borrow();
            let pad = 16.;
            let cw = (ui_dclick.canvas.width() as f64 - 2. * pad).max(10.);
            let ch = (ui_dclick.canvas.height() as f64 - 2. * pad).max(10.);
            let scale = (cw / st.cfg.display.width.max(1) as f64)
                .min(ch / st.cfg.display.height.max(1) as f64);
            let vw = st.cfg.display.width as f64 * scale;
            let vh = st.cfg.display.height as f64 * scale;
            let ox = (ui_dclick.canvas.width() as f64 - vw) / 2.;
            let oy = (ui_dclick.canvas.height() as f64 - vh) / 2.;
            let nx = ((x - ox) / vw).clamp(0., 1.) as f32;
            let ny = ((y - oy) / vh).clamp(0., 1.) as f32;
            drop(st);
            open_binding_dialog_inner(
                &ui_dclick,
                None,
                if gesture.current_button() == 1 {
                    EditType::KeyboardTap
                } else {
                    EditType::MouseTap
                },
                nx,
                ny,
            );
        }
    });
    canvas.add_controller(dclick);

    // Canvas Drag and Drop
    let drag = GestureDrag::new();
    let drag_sel = Rc::new(Cell::new(None::<BindingRef>));
    let start_pos = Rc::new(Cell::new((0., 0.)));
    {
        let ui_drag = ui.clone();
        let ds = drag_sel.clone();
        let ss = start_pos.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let st = ui_drag.state.borrow();
            let pad = 16.;
            let cw = (ui_drag.canvas.width() as f64 - 2. * pad).max(10.);
            let ch = (ui_drag.canvas.height() as f64 - 2. * pad).max(10.);
            let scale = (cw / st.cfg.display.width.max(1) as f64)
                .min(ch / st.cfg.display.height.max(1) as f64);
            let vw = st.cfg.display.width as f64 * scale;
            let vh = st.cfg.display.height as f64 * scale;
            let ox = (ui_drag.canvas.width() as f64 - vw) / 2.;
            let oy = (ui_drag.canvas.height() as f64 - vh) / 2.;
            let nx = ((x - ox) / vw).clamp(0., 1.) as f32;
            let ny = ((y - oy) / vh).clamp(0., 1.) as f32;
            let sel = nearest_binding(&st.cfg, nx, ny);
            ds.set(sel);
            ss.set((nx as f64, ny as f64));
            if let Some(s) = sel {
                drop(st);
                select_binding(&ui_drag, s);
            }
        });
    }
    {
        let ui_drag = ui.clone();
        let ds = drag_sel.clone();
        let ss = start_pos.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(sel) = ds.get() else { return };
            let st = ui_drag.state.borrow();
            let pad = 16.;
            let cw = (ui_drag.canvas.width() as f64 - 2. * pad).max(10.);
            let ch = (ui_drag.canvas.height() as f64 - 2. * pad).max(10.);
            let scale = (cw / st.cfg.display.width.max(1) as f64)
                .min(ch / st.cfg.display.height.max(1) as f64);
            let vw = st.cfg.display.width as f64 * scale;
            let vh = st.cfg.display.height as f64 * scale;
            let (sx, sy) = ss.get();
            drop(st);
            let nx = clamp(sx + dx / vw);
            let ny = clamp(sy + dy / vh);
            let mut state = ui_drag.state.borrow_mut();
            set_selected_position(&mut state.cfg, sel, nx, ny);
            state.dirty = true;
            state.selected = Some(sel);
            drop(state);
            ui_drag.canvas.queue_draw();
            update_selected_editor(&ui_drag);
        });
        let ds2 = drag_sel.clone();
        drag.connect_drag_end(move |_, _, _| ds2.set(None));
    }
    canvas.add_controller(drag);

    // Arrow keys nudge selected item & Delete key deletes
    let key_ctrl = EventControllerKey::new();
    let ui_key = ui.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        let name = key.name().map(|x| x.to_string()).unwrap_or_default();
        let mut st = ui_key.state.borrow_mut();
        if let Some(sel) = st.selected {
            if let Some((px, py)) = selected_position(&st.cfg, sel) {
                let step = 0.005_f32;
                let (nx, ny) = match name.as_str() {
                    "Left" => ((px - step).clamp(0., 1.), py),
                    "Right" => ((px + step).clamp(0., 1.), py),
                    "Up" => (px, (py - step).clamp(0., 1.)),
                    "Down" => (px, (py + step).clamp(0., 1.)),
                    "Delete" => {
                        drop(st);
                        delete_binding(&ui_key, sel);
                        return glib::Propagation::Stop;
                    }
                    _ => return glib::Propagation::Proceed,
                };
                set_selected_position(&mut st.cfg, sel, nx, ny);
                st.dirty = true;
                drop(st);
                ui_key.canvas.queue_draw();
                update_selected_editor(&ui_key);
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });
    app_window.add_controller(key_ctrl);

    // Profile list selection
    profile_list.connect_row_selected({
        let ui2 = ui.clone();
        move |_, row| {
            if let Some(r) = row {
                load_selected_profile(&ui2, r);
            }
        }
    });

    let initial = profile_files();
    for p in initial {
        let row = ListBoxRow::new();
        let label = Label::new(Some(&display_name(&p)));
        label.set_xalign(0.);
        add_margins(&label, 6);
        row.set_child(Some(&label));
        profile_list.append(&row);
        if p == ui.state.borrow().profile_path {
            profile_list.select_row(Some(&row));
        }
    }
    rebuild_bindings(&ui);
    sync_form(&ui);
    update_selected_editor(&ui);

    // Connect Action Buttons
    let ui_save = ui.clone();
    save_btn.connect_clicked(move |_| match save_current(&ui_save) {
        Ok(()) => {
            rebuild_profiles(&ui_save);
            set_status(&ui_save, "Profile saved successfully ✓");
        }
        Err(e) => set_status(&ui_save, &format!("Save failed: {e}")),
    });

    let ui_apply = ui.clone();
    apply_btn.connect_clicked(move |_| apply_and_run(&ui_apply));

    let ui_toggle = ui.clone();
    toggle_lock_btn.connect_clicked(move |_| runtime_control(&ui_toggle, "toggle"));

    let ui_diag = ui.clone();
    diag_btn.connect_clicked(move |_| diagnostics(&ui_diag));

    let ui_new = ui.clone();
    let win_new = app_window.clone();
    new_btn.connect_clicked(move |_| new_profile(&ui_new, &win_new));

    let ui_dup = ui.clone();
    let win_dup = app_window.clone();
    dup_btn.connect_clicked(move |_| duplicate_profile(&ui_dup, &win_dup));

    let ui_del = ui.clone();
    del_btn.connect_clicked(move |_| delete_profile(&ui_del));

    let ui_ff = ui.clone();
    free_fire_btn.connect_clicked(move |_| apply_preset(&ui_ff, ShooterPreset::FreeFire));

    let ui_pubg = ui.clone();
    pubg_btn.connect_clicked(move |_| apply_preset(&ui_pubg, ShooterPreset::Pubg));

    let ui_fps = ui.clone();
    fps_btn.connect_clicked(move |_| apply_preset(&ui_fps, ShooterPreset::Fps));

    let ui_min = ui.clone();
    minimal_btn.connect_clicked(move |_| apply_preset(&ui_min, ShooterPreset::Minimal));

    let ui_start = ui.clone();
    start_btn.connect_clicked(move |_| {
        run_background(&ui_start, "Starting daemon…", || service_action("start"));
    });

    let ui_stop = ui.clone();
    stop_btn.connect_clicked(move |_| {
        run_background(&ui_stop, "Stopping daemon…", || service_action("stop"));
    });

    let ui_rst = ui.clone();
    restart_btn.connect_clicked(move |_| {
        run_background(&ui_rst, "Restarting daemon…", || {
            service_action("restart")
        });
    });

    let ui_wd_start = ui.clone();
    wd_start.connect_clicked(move |_| waydroid_action(&ui_wd_start, "start"));

    let ui_wd_stop = ui.clone();
    wd_stop.connect_clicked(move |_| waydroid_action(&ui_wd_stop, "stop"));

    let ui_fix = ui.clone();
    fix_input.connect_clicked(move |_| {
        run_background(&ui_fix, "Repairing input permissions…", install_input_permissions);
    });

    let ui_ref_dev = ui.clone();
    refresh_dev.connect_clicked(move |_| {
        sync_state_from_form(&ui_ref_dev);
        let cfg = ui_ref_dev.state.borrow().cfg.clone();
        fill_devices(&ui_ref_dev.keyboard, &cfg.devices.keyboard, false);
        fill_devices(&ui_ref_dev.mouse, &cfg.devices.mouse, true);
        set_status(&ui_ref_dev, "Input devices refreshed ✓");
    });

    let ui_akt = ui.clone();
    add_key_tap.connect_clicked(move |_| {
        open_binding_dialog_inner(&ui_akt, None, EditType::KeyboardTap, 0.5, 0.5);
    });

    let ui_akh = ui.clone();
    add_key_hold.connect_clicked(move |_| {
        open_binding_dialog_inner(&ui_akh, None, EditType::KeyboardHold, 0.5, 0.5);
    });

    let ui_amh = ui.clone();
    add_mouse_hold.connect_clicked(move |_| {
        open_binding_dialog_inner(&ui_amh, None, EditType::MouseHold, 0.5, 0.5);
    });

    // Periodic Background Status Update
    {
        let ui_timer = ui.clone();
        glib::timeout_add_local(Duration::from_millis(600), move || {
            update_runtime_status(&ui_timer);
            glib::ControlFlow::Continue
        });
    }

    app_window.present();
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

#[cfg(test)]
mod profile_name_tests {
    use super::safe_profile_name;

    #[test]
    fn profile_name_is_sanitized() {
        assert_eq!(safe_profile_name("Free Fire").as_deref(), Some("Free_Fire"));
        assert_eq!(safe_profile_name("../escape").as_deref(), Some("escape"));
        assert!(safe_profile_name("___").is_none());
    }
}
