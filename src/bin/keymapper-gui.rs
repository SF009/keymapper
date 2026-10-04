use waydroid_keymapper::{config, control, input};

use config::{
    Aim, Config, Display, Devices, Hold, Joystick, MouseHold, MouseTap, Performance, Tap,
    TouchSettings,
};
use gtk4::prelude::*;
use gtk4::{
    cairo, glib, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, ComboBoxText,
    CssProvider, Dialog, DrawingArea, Entry, EventControllerKey, Grid, Label, ListBox, ListBoxRow,
    Orientation, Paned, PolicyType, ScrolledWindow, Separator, SpinButton,
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
const USER_SERVICE: &str = "waydroid-keymapper.service";

const APP_CSS: &str = r#"
window {
  background-color: #0b0f14;
  color: #e8edf5;
  font-family: "Cantarell", "Segoe UI", sans-serif;
}
.header {
  background-color: #101722;
  border-bottom: 1px solid #263140;
  padding: 10px 12px;
}
.title {
  font-size: 19px;
  font-weight: 800;
}
.subtitle { color: #8b98aa; font-size: 12px; }
.card {
  background-color: #111823;
  border: 1px solid #273344;
  border-radius: 12px;
  padding: 12px;
  margin: 6px;
}
.section-title {
  color: #8fd3ff;
  font-weight: 800;
  font-size: 12px;
  letter-spacing: 0.5px;
}
.help { color: #98a5b7; font-size: 11px; }
.muted { color: #7e8a9b; }
.badge-ok {
  background-color: #153523;
  color: #73e0a2;
  border: 1px solid #2d6f4b;
  border-radius: 8px;
  padding: 4px 9px;
  font-weight: 800;
}
.badge-warn {
  background-color: #342915;
  color: #f6c56c;
  border: 1px solid #77581f;
  border-radius: 8px;
  padding: 4px 9px;
  font-weight: 800;
}
.badge-danger {
  background-color: #351d23;
  color: #ff8896;
  border: 1px solid #6d2e3a;
  border-radius: 8px;
  padding: 4px 9px;
  font-weight: 800;
}
button {
  border-radius: 8px;
  min-height: 34px;
}
.primary {
  background-color: #2f7cf6;
  color: white;
  font-weight: 800;
}
.success {
  background-color: #1e7d4f;
  color: white;
  font-weight: 800;
}
.danger {
  background-color: #5c2932;
  color: #ffd9dd;
}
entry, spinbutton, combobox {
  background-color: #151e2a;
  color: #ecf2fa;
  border: 1px solid #2d3b4e;
  border-radius: 8px;
}
entry:focus, spinbutton:focus, combobox:focus {
  border-color: #4f98ff;
}
list, listview {
  background-color: #0e151e;
}
list row {
  border-radius: 7px;
  margin: 2px 0;
}
list row:selected {
  background-color: #244c7f;
}
.preview {
  background-color: #080c12;
  border-radius: 12px;
  border: 1px solid #263140;
}
button.flat {
  background: transparent;
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

#[derive(Clone, Copy, Debug)]
enum EditType {
    KeyboardTap,
    KeyboardHold,
    MouseTap,
    MouseHold,
}

#[derive(Clone, Copy, Debug)]
enum Preset {
    FreeFire,
    Pubg,
    Fps,
    Minimal,
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
    selected_editor: GtkBox,
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
    auto_lock_on_aim: CheckButton,
    mouse_toggle: Entry,
    runtime_status: Label,
    lock_status: Label,
    input_access: Label,
    waydroid_status: Label,
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

fn temp_path(path: &Path, tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = path.file_name().and_then(|x| x.to_str()).unwrap_or("tmp");
    path.with_file_name(format!(".{base}.{tag}.{pid}.{nanos}.tmp"))
}

fn default_config() -> Config {
    Config {
        display: Display { width: 1920, height: 1080 },
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
            button: "MOUSE_RIGHT".into(),
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
            Tap { key: "SPACE".into(), x: 0.86, y: 0.86, slot: 2 },
            Tap { key: "R".into(), x: 0.93, y: 0.18, slot: 3 },
        ],
        holds: vec![Hold {
            key: "F".into(),
            x: 0.78,
            y: 0.84,
            slot: 4,
        }],
        mouse_taps: vec![],
        mouse_holds: vec![MouseHold {
            button: "MOUSE_LEFT".into(),
            x: 0.88,
            y: 0.78,
            slot: 5,
        }],
        performance: Performance::default(),
        touch: TouchSettings::default(),
    }
}

fn preset_free_fire() -> Config {
    let mut c = default_config();
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
        mode: "touch".into(),
    });
    c.taps = vec![
        Tap { key: "SPACE".into(), x: 0.88, y: 0.88, slot: 2 },
        Tap { key: "C".into(), x: 0.78, y: 0.90, slot: 3 },
        Tap { key: "Z".into(), x: 0.70, y: 0.90, slot: 4 },
        Tap { key: "R".into(), x: 0.92, y: 0.20, slot: 5 },
        Tap { key: "1".into(), x: 0.70, y: 0.16, slot: 6 },
        Tap { key: "2".into(), x: 0.78, y: 0.16, slot: 7 },
        Tap { key: "G".into(), x: 0.18, y: 0.55, slot: 8 },
        Tap { key: "4".into(), x: 0.12, y: 0.55, slot: 9 },
        Tap { key: "TAB".into(), x: 0.08, y: 0.88, slot: 10 },
    ];
    c.holds = vec![
        Hold { key: "SHIFT".into(), x: 0.28, y: 0.75, slot: 11 },
        Hold { key: "F".into(), x: 0.76, y: 0.76, slot: 12 },
    ];
    c.mouse_taps = vec![];
    c.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 13,
    }];
    c
}

fn preset_pubg() -> Config {
    let mut c = preset_fps();
    c.aim.as_mut().unwrap().sensitivity = 2.2;
    c.taps.push(Tap { key: "Q".into(), x: 0.35, y: 0.45, slot: 14 });
    c.taps.push(Tap { key: "E".into(), x: 0.42, y: 0.45, slot: 15 });
    c
}

fn preset_fps() -> Config {
    let mut c = default_config();
    c.aim = Some(Aim {
        button: "MOUSE_RIGHT".into(),
        center_x: 0.5,
        center_y: 0.5,
        sensitivity: 2.1,
        slot: 1,
        invert_x: false,
        invert_y: false,
        scale_x: 1.0,
        scale_y: 1.0,
        edge_margin: 0.12,
        mode: "relative".into(),
    });
    c.taps = vec![
        Tap { key: "SPACE".into(), x: 0.86, y: 0.86, slot: 2 },
        Tap { key: "R".into(), x: 0.93, y: 0.18, slot: 3 },
        Tap { key: "1".into(), x: 0.72, y: 0.18, slot: 4 },
        Tap { key: "2".into(), x: 0.78, y: 0.18, slot: 5 },
    ];
    c.holds = vec![
        Hold { key: "SHIFT".into(), x: 0.28, y: 0.76, slot: 6 },
        Hold { key: "C".into(), x: 0.34, y: 0.88, slot: 7 },
        Hold { key: "F".into(), x: 0.78, y: 0.84, slot: 8 },
    ];
    c.mouse_taps = vec![];
    c.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 9,
    }];
    c
}

fn preset_minimal() -> Config {
    let mut c = default_config();
    c.taps = vec![
        Tap { key: "SPACE".into(), x: 0.86, y: 0.86, slot: 2 },
        Tap { key: "R".into(), x: 0.93, y: 0.18, slot: 3 },
    ];
    c.holds = vec![Hold {
        key: "SHIFT".into(),
        x: 0.28,
        y: 0.76,
        slot: 4,
    }];
    c.mouse_taps = vec![];
    c.mouse_holds = vec![MouseHold {
        button: "MOUSE_LEFT".into(),
        x: 0.88,
        y: 0.78,
        slot: 5,
    }];
    c
}

fn safe_profile_name(name: &str) -> Option<String> {
    let safe = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if safe.is_empty() || safe == "." || safe == ".." {
        None
    } else {
        Some(safe)
    }
}

fn profile_files() -> Vec<PathBuf> {
    let mut files = fs::read_dir(profiles_dir())
        .ok()
        .into_iter()
        .flat_map(|it| it.flatten().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    files.sort_by_key(|p| p.file_name().map(|x| x.to_os_string()));
    files
}

fn load_profile(path: &Path) -> Result<Config, Box<dyn Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

fn save_profile(path: &Path, cfg: &Config) -> Result<(), Box<dyn Error>> {
    cfg.validate()?;
    let parent = path
        .parent()
        .ok_or_else(|| "profile path has no parent directory")?;
    fs::create_dir_all(parent)?;
    let tmp = temp_path(path, "save");
    fs::write(&tmp, toml::to_string_pretty(cfg)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn ensure_profiles() -> Result<PathBuf, Box<dyn Error>> {
    let dir = profiles_dir();
    fs::create_dir_all(&dir)?;

    let default = dir.join("default.toml");
    if !default.exists() {
        let source = active_config_path();
        let cfg = load_profile(&source).unwrap_or_else(|_| default_config());
        save_profile(&default, &cfg)?;
    }

    Ok(dir)
}

fn add_margins<W: gtk4::prelude::WidgetExt>(widget: &W, margin: i32) {
    widget.set_margin_top(margin);
    widget.set_margin_bottom(margin);
    widget.set_margin_start(margin);
    widget.set_margin_end(margin);
}

fn section(parent: &GtkBox, title: &str) -> GtkBox {
    let box_ = GtkBox::new(Orientation::Vertical, 7);
    box_.add_css_class("card");
    let label = Label::new(Some(title));
    label.add_css_class("section-title");
    label.set_halign(gtk4::Align::Start);
    box_.append(&label);
    parent.append(&box_);
    box_
}

fn form_row(grid: &Grid, row: i32, label: &str, widget: &impl gtk4::prelude::WidgetExt) {
    let l = Label::new(Some(label));
    l.set_halign(gtk4::Align::Start);
    grid.attach(&l, 0, row, 1, 1);
    grid.attach(widget, 1, row, 1, 1);
}

fn spin(min: f64, max: f64, step: f64, digits: u32) -> SpinButton {
    let s = SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s
}

fn binding_label(sel: BindingRef, cfg: &Config) -> String {
    match sel {
        BindingRef::Tap(i) => cfg.taps.get(i).map(|x| format!("⌨ {}  ·  TAP  ·  slot {}", x.key, x.slot)).unwrap_or_default(),
        BindingRef::Hold(i) => cfg.holds.get(i).map(|x| format!("⌨ {}  ·  HOLD  ·  slot {}", x.key, x.slot)).unwrap_or_default(),
        BindingRef::MouseTap(i) => cfg.mouse_taps.get(i).map(|x| format!("🖱 {}  ·  TAP  ·  slot {}", x.button, x.slot)).unwrap_or_default(),
        BindingRef::MouseHold(i) => cfg.mouse_holds.get(i).map(|x| format!("🖱 {}  ·  HOLD/FIRE  ·  slot {}", x.button, x.slot)).unwrap_or_default(),
        BindingRef::Aim => cfg.aim.as_ref().map(|a| format!("🎯 AIM  ·  {}  ·  {}  ·  {:.2}×", a.button, a.mode, a.sensitivity)).unwrap_or_else(|| "🎯 AIM".into()),
        BindingRef::Joystick => cfg.joystick.as_ref().map(|j| format!("🕹 WASD  ·  {:.0}%/{:.0}%  ·  slot {}", j.center_x * 100., j.center_y * 100., j.slot)).unwrap_or_else(|| "🕹 JOYSTICK".into()),
    }
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
        BindingRef::Tap(i) => if let Some(v) = cfg.taps.get_mut(i) { v.x = x; v.y = y; },
        BindingRef::Hold(i) => if let Some(v) = cfg.holds.get_mut(i) { v.x = x; v.y = y; },
        BindingRef::MouseTap(i) => if let Some(v) = cfg.mouse_taps.get_mut(i) { v.x = x; v.y = y; },
        BindingRef::MouseHold(i) => if let Some(v) = cfg.mouse_holds.get_mut(i) { v.x = x; v.y = y; },
        BindingRef::Aim => if let Some(v) = cfg.aim.as_mut() { v.center_x = x; v.center_y = y; },
        BindingRef::Joystick => if let Some(v) = cfg.joystick.as_mut() { v.center_x = x; v.center_y = y; },
    }
}

fn all_bindings(cfg: &Config) -> Vec<BindingRef> {
    let mut v = Vec::new();
    if cfg.joystick.is_some() { v.push(BindingRef::Joystick); }
    if cfg.aim.is_some() { v.push(BindingRef::Aim); }
    v.extend((0..cfg.taps.len()).map(BindingRef::Tap));
    v.extend((0..cfg.holds.len()).map(BindingRef::Hold));
    v.extend((0..cfg.mouse_taps.len()).map(BindingRef::MouseTap));
    v.extend((0..cfg.mouse_holds.len()).map(BindingRef::MouseHold));
    v
}

fn nearest_binding(cfg: &Config, x: f32, y: f32) -> Option<BindingRef> {
    let mut best = None;
    let mut dist = 0.055_f32;
    for item in all_bindings(cfg) {
        if let Some((px, py)) = selected_position(cfg, item) {
            let d = ((px - x).powi(2) + (py - y).powi(2)).sqrt();
            if d < dist {
                dist = d;
                best = Some(item);
            }
        }
    }
    best
}

fn draw_canvas(state: &Rc<RefCell<State>>, _area: &DrawingArea, cr: &cairo::Context, w: i32, h: i32) {
    let st = state.borrow();
    let cfg = &st.cfg;

    cr.set_source_rgb(0.03, 0.04, 0.055);
    cr.rectangle(0.0, 0.0, w as f64, h as f64);
    let _ = cr.fill();

    let pad = 18.0;
    let usable_w = (w as f64 - 2.0 * pad).max(10.0);
    let usable_h = (h as f64 - 2.0 * pad).max(10.0);
    let scale = (usable_w / cfg.display.width.max(1) as f64)
        .min(usable_h / cfg.display.height.max(1) as f64);
    let vw = cfg.display.width as f64 * scale;
    let vh = cfg.display.height as f64 * scale;
    let ox = (w as f64 - vw) / 2.0;
    let oy = (h as f64 - vh) / 2.0;

    cr.set_source_rgb(0.07, 0.085, 0.11);
    cr.rectangle(ox, oy, vw, vh);
    let _ = cr.fill();

    cr.set_source_rgb(0.13, 0.15, 0.19);
    cr.set_line_width(1.0);
    for i in 1..10 {
        let gx = ox + vw * i as f64 / 10.0;
        let gy = oy + vh * i as f64 / 10.0;
        cr.move_to(gx, oy);
        cr.line_to(gx, oy + vh);
        cr.move_to(ox, gy);
        cr.line_to(ox + vw, gy);
    }
    let _ = cr.stroke();

    let point = |x: f32, y: f32| -> (f64, f64) {
        (ox + vw * x.clamp(0.0, 1.0) as f64, oy + vh * y.clamp(0.0, 1.0) as f64)
    };

    if let Some(j) = &cfg.joystick {
        let (x, y) = point(j.center_x, j.center_y);
        let radius = vw.min(vh) * j.radius as f64;
        cr.set_source_rgba(0.20, 0.92, 0.48, 0.16);
        cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
        cr.set_source_rgb(0.30, 1.0, 0.54);
        cr.set_line_width(2.0);
        cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();
        cr.arc(x, y, 5.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    }

    let draw_marker = |sel: BindingRef, x: f32, y: f32, label: &str| {
        let (px, py) = point(x, y);
        let selected = st.selected == Some(sel);
        let radius = if selected { 13.0 } else { 9.0 };

        let (r, g, b) = match sel {
            BindingRef::Aim => (0.25, 0.75, 1.0),
            BindingRef::Joystick => (0.30, 1.0, 0.54),
            BindingRef::Tap(_) => (1.0, 0.72, 0.25),
            BindingRef::Hold(_) => (0.45, 0.72, 1.0),
            BindingRef::MouseTap(_) => (0.72, 0.52, 1.0),
            BindingRef::MouseHold(_) => (1.0, 0.30, 0.48),
        };

        cr.set_source_rgba(r, g, b, 0.18);
        cr.arc(px, py, radius + 8.0, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        cr.set_source_rgb(r, g, b);
        cr.arc(px, py, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        if selected {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(2.0);
            cr.arc(px, py, radius + 4.0, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
        }

        cr.set_source_rgb(0.95, 0.97, 1.0);
        cr.move_to(px + radius + 5.0, py + 4.0);
        let _ = cr.show_text(label);
    };

    if let Some(a) = &cfg.aim {
        draw_marker(BindingRef::Aim, a.center_x, a.center_y, if a.mode.eq_ignore_ascii_case("relative") { "AIM / FPS" } else { "AIM / TOUCH" });
    }
    for (i, x) in cfg.taps.iter().enumerate() {
        draw_marker(BindingRef::Tap(i), x.x, x.y, &format!("⌨ {}", x.key));
    }
    for (i, x) in cfg.holds.iter().enumerate() {
        draw_marker(BindingRef::Hold(i), x.x, x.y, &format!("⇧ {}", x.key));
    }
    for (i, x) in cfg.mouse_taps.iter().enumerate() {
        draw_marker(BindingRef::MouseTap(i), x.x, x.y, &format!("🖱 {}", x.button));
    }
    for (i, x) in cfg.mouse_holds.iter().enumerate() {
        draw_marker(BindingRef::MouseHold(i), x.x, x.y, &format!("🔥 {}", x.button));
    }

    cr.set_source_rgb(0.56, 0.61, 0.69);
    cr.move_to(ox, oy + vh + 22.0);
    let _ = cr.show_text("Click = select   •   Drag = move   •   Double-click = add");
}

fn selected_editor(ui: &Ui) {
    while let Some(child) = ui.selected_editor.first_child() {
        ui.selected_editor.remove(&child);
    }

    let st = ui.state.borrow();
    let Some(sel) = st.selected else {
        let label = Label::new(Some("Select a control on the map to edit its position, slot, and binding."));
        label.add_css_class("help");
        label.set_wrap(true);
        label.set_halign(gtk4::Align::Start);
        ui.selected_editor.append(&label);
        return;
    };

    let heading = Label::new(Some(&binding_label(sel, &st.cfg)));
    heading.add_css_class("section-title");
    heading.set_halign(gtk4::Align::Start);
    ui.selected_editor.append(&heading);

    let grid = Grid::new();
    grid.set_row_spacing(6);
    grid.set_column_spacing(8);

    if let Some((x0, y0)) = selected_position(&st.cfg, sel) {
        let x = spin(0.0, 1.0, 0.005, 3);
        let y = spin(0.0, 1.0, 0.005, 3);
        x.set_value(x0 as f64);
        y.set_value(y0 as f64);

        let ui_x = ui.clone();
        x.connect_value_changed(move |s| {
            let mut state = ui_x.state.borrow_mut();
            if let Some(item) = state.selected {
                let yv = selected_position(&state.cfg, item).map(|p| p.1).unwrap_or(0.5);
                set_selected_position(&mut state.cfg, item, s.value() as f32, yv);
                state.dirty = true;
                drop(state);
                ui_x.canvas.queue_draw();
            }
        });

        let ui_y = ui.clone();
        y.connect_value_changed(move |s| {
            let mut state = ui_y.state.borrow_mut();
            if let Some(item) = state.selected {
                let xv = selected_position(&state.cfg, item).map(|p| p.0).unwrap_or(0.5);
                set_selected_position(&mut state.cfg, item, xv, s.value() as f32);
                state.dirty = true;
                drop(state);
                ui_y.canvas.queue_draw();
            }
        });

        form_row(&grid, 0, "X", &x);
        form_row(&grid, 1, "Y", &y);
    }

    let delete = Button::with_label("Delete selected");
    delete.add_css_class("danger");
    let ui2 = ui.clone();
    delete.connect_clicked(move |_| delete_binding(&ui2, sel));

    ui.selected_editor.append(&grid);
    ui.selected_editor.append(&delete);
}

fn select_binding(ui: &Ui, sel: BindingRef) {
    {
        let mut state = ui.state.borrow_mut();
        state.selected = Some(sel);
    }

    selected_editor(ui);
    ui.canvas.queue_draw();

    let label = {
        let state = ui.state.borrow();
        binding_label(sel, &state.cfg)
    };
    ui.status.set_text(&format!("Selected: {label}"));
}

fn delete_binding(ui: &Ui, sel: BindingRef) {
    {
        let mut st = ui.state.borrow_mut();
        match sel {
            BindingRef::Tap(i) => { if i < st.cfg.taps.len() { st.cfg.taps.remove(i); } }
            BindingRef::Hold(i) => { if i < st.cfg.holds.len() { st.cfg.holds.remove(i); } }
            BindingRef::MouseTap(i) => { if i < st.cfg.mouse_taps.len() { st.cfg.mouse_taps.remove(i); } }
            BindingRef::MouseHold(i) => { if i < st.cfg.mouse_holds.len() { st.cfg.mouse_holds.remove(i); } }
            BindingRef::Aim => st.cfg.aim = None,
            BindingRef::Joystick => st.cfg.joystick = None,
        }
        st.selected = None;
        st.dirty = true;
    }
    rebuild_bindings(ui);
    selected_editor(ui);
    ui.canvas.queue_draw();
    ui.status.set_text("Control deleted. Save to persist.");
}

fn rebuild_bindings(ui: &Ui) {
    while let Some(child) = ui.bindings_box.first_child() {
        ui.bindings_box.remove(&child);
    }

    let cfg = ui.state.borrow().cfg.clone();
    let groups: [(&str, Vec<BindingRef>); 4] = [
        ("Keyboard TAP", (0..cfg.taps.len()).map(BindingRef::Tap).collect()),
        ("Keyboard HOLD", (0..cfg.holds.len()).map(BindingRef::Hold).collect()),
        ("Mouse TAP", (0..cfg.mouse_taps.len()).map(BindingRef::MouseTap).collect()),
        ("Mouse HOLD / FIRE", (0..cfg.mouse_holds.len()).map(BindingRef::MouseHold).collect()),
    ];

    for (title, items) in groups {
        if items.is_empty() { continue; }
        let label = Label::new(Some(title));
        label.add_css_class("section-title");
        label.set_halign(gtk4::Align::Start);
        ui.bindings_box.append(&label);

        for item in items {
            let row = GtkBox::new(Orientation::Horizontal, 6);
            let text = Label::new(Some(&binding_label(item, &cfg)));
            text.set_hexpand(true);
            text.set_halign(gtk4::Align::Start);

            let edit = Button::with_label("Edit");
            edit.set_focus_on_click(false);
            let ui2 = ui.clone();
            edit.connect_clicked(move |_| open_binding_dialog(&ui2, Some(item)));

            let del = Button::with_label("×");
            del.add_css_class("danger");
            del.set_focus_on_click(false);
            let ui3 = ui.clone();
            del.connect_clicked(move |_| delete_binding(&ui3, item));

            row.append(&text);
            row.append(&edit);
            row.append(&del);
            ui.bindings_box.append(&row);
        }
    }

    for (label_text, sel) in [
        ("🕹 Joystick", BindingRef::Joystick),
        ("🎯 Aim", BindingRef::Aim),
    ] {
        let exists = match sel {
            BindingRef::Joystick => cfg.joystick.is_some(),
            BindingRef::Aim => cfg.aim.is_some(),
            _ => false,
        };
        if exists {
            let row = GtkBox::new(Orientation::Horizontal, 6);
            let label = Label::new(Some(&format!("{label_text}  ·  {}", binding_label(sel, &cfg))));
            label.set_hexpand(true);
            label.set_halign(gtk4::Align::Start);
            let edit = Button::with_label("Edit");
            let ui2 = ui.clone();
            edit.connect_clicked(move |_| select_binding(&ui2, sel));
            row.append(&label);
            row.append(&edit);
            ui.bindings_box.append(&row);
        }
    }
}

fn key_alias(name: &str) -> String {
    match name.to_ascii_uppercase().as_str() {
        "SPACE" => "SPACE",
        "RETURN" | "ENTER" => "ENTER",
        "ESCAPE" | "ESC" => "ESC",
        "CONTROL_L" | "CONTROL_R" => "CTRL",
        "SHIFT_L" | "SHIFT_R" => "SHIFT",
        "ALT_L" | "ALT_R" => "ALT",
        "PAGE_UP" => "PAGEUP",
        "PAGE_DOWN" => "PAGEDOWN",
        x => x,
    }
    .into()
}

fn capture_key(entry: &Entry, capture: &Button, status: &Label) {
    let armed = Rc::new(Cell::new(false));

    {
        let armed = armed.clone();
        let entry = entry.clone();
        let status = status.clone();
        capture.connect_clicked(move |_| {
            armed.set(true);
            entry.grab_focus();
            status.set_text("Press a keyboard key…");
        });
    }

    let controller = EventControllerKey::new();
    let armed2 = armed.clone();
    let entry2 = entry.clone();
    let status2 = status.clone();
    controller.connect_key_pressed(move |_, key, _, _| {
        if !armed2.get() {
            return glib::Propagation::Proceed;
        }
        if let Some(name) = key.name() {
            entry2.set_text(&key_alias(name.as_str()));
            armed2.set(false);
            status2.set_text("Keyboard key captured ✓");
        }
        glib::Propagation::Stop
    });
    entry.add_controller(controller);
}

fn capture_mouse(entry: &Entry, capture: &Button, status: &Label) {
    let armed = Rc::new(Cell::new(false));

    {
        let armed = armed.clone();
        let entry = entry.clone();
        let status = status.clone();
        capture.connect_clicked(move |_| {
            armed.set(true);
            entry.grab_focus();
            status.set_text("Click a mouse button on this entry…");
        });
    }

    let gesture = gtk4::GestureClick::new();
    gesture.set_button(0);
    let armed2 = armed.clone();
    let entry2 = entry.clone();
    let status2 = status.clone();
    gesture.connect_pressed(move |g, _, _, _| {
        if !armed2.get() {
            return;
        }
        let button = match g.current_button() {
            1 => "MOUSE_LEFT",
            2 => "MOUSE_MIDDLE",
            3 => "MOUSE_RIGHT",
            8 => "MOUSE_SIDE",
            9 => "MOUSE_EXTRA",
            _ => return,
        };
        entry2.set_text(button);
        armed2.set(false);
        status2.set_text("Mouse button captured ✓");
    });
    entry.add_controller(gesture);
}

fn next_slot(cfg: &Config) -> u8 {
    let mut used = [false; 16];
    for s in cfg
        .joystick
        .iter()
        .map(|x| x.slot)
        .chain(cfg.aim.iter().map(|x| x.slot))
        .chain(cfg.taps.iter().map(|x| x.slot))
        .chain(cfg.holds.iter().map(|x| x.slot))
        .chain(cfg.mouse_taps.iter().map(|x| x.slot))
        .chain(cfg.mouse_holds.iter().map(|x| x.slot))
    {
        if (s as usize) < used.len() {
            used[s as usize] = true;
        }
    }
    used.iter().position(|v| !*v).unwrap_or(15) as u8
}

fn open_binding_dialog(ui: &Ui, existing: Option<BindingRef>) {
    let kind = match existing {
        Some(BindingRef::Tap(_)) => EditType::KeyboardTap,
        Some(BindingRef::Hold(_)) => EditType::KeyboardHold,
        Some(BindingRef::MouseTap(_)) => EditType::MouseTap,
        Some(BindingRef::MouseHold(_)) => EditType::MouseHold,
        _ => return,
    };
    open_binding_dialog_at(ui, existing, kind, 0.5, 0.5);
}

fn open_binding_dialog_at(
    ui: &Ui,
    existing: Option<BindingRef>,
    kind: EditType,
    init_x: f32,
    init_y: f32,
) {
    let Some(parent) = ui
        .canvas
        .root()
        .and_then(|w| w.downcast::<ApplicationWindow>().ok())
    else {
        return;
    };

    let dialog = Dialog::builder()
        .transient_for(&parent)
        .modal(true)
        .title(match kind {
            EditType::KeyboardTap => "Keyboard TAP",
            EditType::KeyboardHold => "Keyboard HOLD",
            EditType::MouseTap => "Mouse TAP",
            EditType::MouseHold => "Mouse HOLD / FIRE",
        })
        .build();

    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    let save = dialog.add_button("Save", gtk4::ResponseType::Accept);
    save.add_css_class("primary");

    let content = GtkBox::new(Orientation::Vertical, 10);
    add_margins(&content, 14);

    let grid = Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(10);

    let key = Entry::new();
    let capture = Button::with_label("Capture");
    let row = GtkBox::new(Orientation::Horizontal, 6);
    row.append(&key);
    row.append(&capture);
    form_row(&grid, 0, if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) { "Key" } else { "Button" }, &row);

    let x = spin(0.0, 1.0, 0.005, 3);
    let y = spin(0.0, 1.0, 0.005, 3);
    let slot = spin(0.0, 15.0, 1.0, 0);
    form_row(&grid, 1, "X", &x);
    form_row(&grid, 2, "Y", &y);
    form_row(&grid, 3, "Slot", &slot);

    content.append(&grid);
    dialog.content_area().append(&content);

    let cfg = ui.state.borrow().cfg.clone();
    if let Some(item) = existing {
        match item {
            BindingRef::Tap(i) => if let Some(v) = cfg.taps.get(i) { key.set_text(&v.key); x.set_value(v.x as f64); y.set_value(v.y as f64); slot.set_value(v.slot as f64); },
            BindingRef::Hold(i) => if let Some(v) = cfg.holds.get(i) { key.set_text(&v.key); x.set_value(v.x as f64); y.set_value(v.y as f64); slot.set_value(v.slot as f64); },
            BindingRef::MouseTap(i) => if let Some(v) = cfg.mouse_taps.get(i) { key.set_text(&v.button); x.set_value(v.x as f64); y.set_value(v.y as f64); slot.set_value(v.slot as f64); },
            BindingRef::MouseHold(i) => if let Some(v) = cfg.mouse_holds.get(i) { key.set_text(&v.button); x.set_value(v.x as f64); y.set_value(v.y as f64); slot.set_value(v.slot as f64); },
            _ => {}
        }
    } else {
        key.set_text(if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) { "SPACE" } else { "MOUSE_LEFT" });
        x.set_value(init_x as f64);
        y.set_value(init_y as f64);
        slot.set_value(next_slot(&cfg) as f64);
    }

    if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) {
        capture_key(&key, &capture, &ui.status);
    } else {
        capture_mouse(&key, &capture, &ui.status);
    }

    let ui2 = ui.clone();
    dialog.connect_response(move |dialog, response| {
        if response != gtk4::ResponseType::Accept {
            dialog.close();
            return;
        }

        let token = key.text().trim().to_string();
        let xx = x.value() as f32;
        let yy = y.value() as f32;
        let ss = slot.value() as u8;

        let valid = if matches!(kind, EditType::KeyboardTap | EditType::KeyboardHold) {
            input::key_code(&token).map(|_| ())
        } else {
            input::button_code(&token).map(|_| ())
        };

        if let Err(e) = valid {
            ui2.status.set_text(&format!("Invalid input: {e}"));
            return;
        }

        {
            let mut st = ui2.state.borrow_mut();
            match (existing, kind) {
                (Some(BindingRef::Tap(i)), EditType::KeyboardTap) => if let Some(v) = st.cfg.taps.get_mut(i) { v.key = token.clone(); v.x = xx; v.y = yy; v.slot = ss; },
                (Some(BindingRef::Hold(i)), EditType::KeyboardHold) => if let Some(v) = st.cfg.holds.get_mut(i) { v.key = token.clone(); v.x = xx; v.y = yy; v.slot = ss; },
                (Some(BindingRef::MouseTap(i)), EditType::MouseTap) => if let Some(v) = st.cfg.mouse_taps.get_mut(i) { v.button = token.clone(); v.x = xx; v.y = yy; v.slot = ss; },
                (Some(BindingRef::MouseHold(i)), EditType::MouseHold) => if let Some(v) = st.cfg.mouse_holds.get_mut(i) { v.button = token.clone(); v.x = xx; v.y = yy; v.slot = ss; },
                (None, EditType::KeyboardTap) => st.cfg.taps.push(Tap { key: token.clone(), x: xx, y: yy, slot: ss }),
                (None, EditType::KeyboardHold) => st.cfg.holds.push(Hold { key: token.clone(), x: xx, y: yy, slot: ss }),
                (None, EditType::MouseTap) => st.cfg.mouse_taps.push(MouseTap { button: token.clone(), x: xx, y: yy, slot: ss }),
                (None, EditType::MouseHold) => st.cfg.mouse_holds.push(MouseHold { button: token.clone(), x: xx, y: yy, slot: ss }),
                _ => {}
            }
            st.dirty = true;
        }

        rebuild_bindings(&ui2);
        ui2.canvas.queue_draw();
        ui2.status.set_text("Binding saved in memory. Click Save to persist.");
        dialog.close();
    });

    dialog.show();
}

fn fill_devices(combo: &ComboBoxText, selected: &Option<String>, mouse: bool) {
    combo.remove_all();
    combo.append(None, "Auto-detect");

    let devices = input::list_input_devices()
        .into_iter()
        .filter(|d| if mouse { d.is_mouse } else { d.is_keyboard })
        .collect::<Vec<_>>();

    for d in &devices {
        combo.append(Some(&d.path), &format!("{}  ·  {}", d.name, d.path));
    }

    if let Some(path) = selected {
        if !devices.iter().any(|d| d.path == *path) {
            combo.append(Some(path), &format!("Saved path  ·  {path}"));
        }
        let _ = combo.set_active_id(Some(path));
    } else {
        combo.set_active(Some(0));
    }
}

fn sync_form(ui: &Ui) {
    let st = ui.state.borrow();

    ui.profile_name.set_text(
        st.profile_path
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("profile"),
    );
    ui.width.set_value(st.cfg.display.width as f64);
    ui.height.set_value(st.cfg.display.height as f64);

    fill_devices(&ui.keyboard, &st.cfg.devices.keyboard, false);
    fill_devices(&ui.mouse, &st.cfg.devices.mouse, true);

    ui.aim_enabled.set_active(st.cfg.aim.is_some());
    if let Some(a) = &st.cfg.aim {
        ui.aim_button.set_text(&a.button);
        let mode = if a.mode.eq_ignore_ascii_case("relative") { "relative" } else { "touch" };
        ui.aim_mode.set_active_id(Some(mode));
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
    ui.realtime_priority.set_value(st.cfg.performance.realtime_priority as f64);
    ui.fifo_write_retries.set_value(st.cfg.performance.fifo_write_retries as f64);
    ui.fifo_write_wait.set_value(st.cfg.performance.fifo_write_wait_ms as f64);
    ui.fifo_reconnect.set_value(st.cfg.performance.fifo_reconnect_ms as f64);
    ui.touch_pressure.set_value(st.cfg.touch.pressure as f64);
    ui.touch_major.set_value(st.cfg.touch.major as f64);
    ui.touch_minor.set_value(st.cfg.touch.minor as f64);

    ui.mouse_lock.set_active(st.cfg.performance.mouse_lock);
    ui.auto_lock_on_aim.set_active(st.cfg.performance.auto_lock_on_aim);
    ui.mouse_toggle.set_text(&st.cfg.performance.mouse_toggle_key);
}

fn sync_state_from_form(ui: &Ui) {
    let mut st = ui.state.borrow_mut();
    st.cfg.display.width = ui.width.value().round() as i32;
    st.cfg.display.height = ui.height.value().round() as i32;
    st.cfg.devices.keyboard = ui.keyboard.active_id().map(|x| x.to_string());
    st.cfg.devices.mouse = ui.mouse.active_id().map(|x| x.to_string());

    st.cfg.performance.grab = ui.grab.is_active();
    st.cfg.performance.realtime = ui.realtime.is_active();
    st.cfg.performance.realtime_priority = ui.realtime_priority.value().round() as i32;
    st.cfg.performance.fifo_write_retries = ui.fifo_write_retries.value().round() as u8;
    st.cfg.performance.fifo_write_wait_ms = ui.fifo_write_wait.value().round() as u64;
    st.cfg.performance.fifo_reconnect_ms = ui.fifo_reconnect.value().round() as u64;
    st.cfg.performance.mouse_lock = ui.mouse_lock.is_active();
    st.cfg.performance.auto_lock_on_aim = ui.auto_lock_on_aim.is_active();
    st.cfg.performance.mouse_toggle_key = ui.mouse_toggle.text().trim().to_string();

    st.cfg.aim = if ui.aim_enabled.is_active() {
        Some(Aim {
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
            mode: ui.aim_mode.active_id().map(|x| x.to_string()).unwrap_or_else(|| "relative".into()),
        })
    } else {
        None
    };

    st.cfg.joystick = if ui.joy_enabled.is_active() {
        Some(Joystick {
            up: ui.joy_up.text().trim().to_string(),
            down: ui.joy_down.text().trim().to_string(),
            left: ui.joy_left.text().trim().to_string(),
            right: ui.joy_right.text().trim().to_string(),
            center_x: ui.joy_x.value() as f32,
            center_y: ui.joy_y.value() as f32,
            radius: ui.joy_radius.value() as f32,
            normalize_diagonal: ui.joy_normalize.is_active(),
            slot: ui.joy_slot.value() as u8,
        })
    } else {
        None
    };

    st.cfg.touch.pressure = ui.touch_pressure.value().round() as i32;
    st.cfg.touch.major = ui.touch_major.value().round() as i32;
    st.cfg.touch.minor = ui.touch_minor.value().round() as i32;
    st.dirty = true;
}

fn current_profile_path(ui: &Ui) -> PathBuf {
    ui.state.borrow().profile_path.clone()
}

fn save_current(ui: &Ui) -> Result<(), String> {
    sync_state_from_form(ui);

    let name = safe_profile_name(ui.profile_name.text().trim())
        .ok_or_else(|| "Profile name is invalid".to_string())?;

    let new_path = profiles_dir().join(format!("{name}.toml"));
    let old_path = current_profile_path(ui);

    let st_cfg = ui.state.borrow().cfg.clone();
    st_cfg.validate().map_err(|e| e.to_string())?;

    if new_path != old_path && new_path.exists() {
        return Err("A profile with that name already exists".into());
    }

    save_profile(&new_path, &st_cfg).map_err(|e| e.to_string())?;

    let mut st = ui.state.borrow_mut();
    if new_path != old_path {
        let _ = fs::remove_file(&old_path);
    }
    st.profile_path = new_path;
    st.dirty = false;
    Ok(())
}

fn copy_active_config(cfg: &Config) -> Result<(), String> {
    cfg.validate_runtime().map_err(|e| e.to_string())?;
    let path = active_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let tmp = temp_path(&path, "activate");
    fs::write(&tmp, toml::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

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
    let parent = dst.parent().ok_or_else(|| "invalid install destination".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = temp_path(dst, "install");
    fs::copy(src, &tmp).map_err(|e| format!("copy {}: {e}", src.display()))?;
    let mut perms = fs::metadata(&tmp).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmp, perms).map_err(|e| e.to_string())?;
    fs::rename(tmp, dst).map_err(|e| format!("activate {}: {e}", dst.display()))
}

fn daemon_source() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("waydroid-keymapper"));
        }
    }

    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("target/release/waydroid-keymapper"));
        candidates.push(cwd.join("waydroid-keymapper"));
    }

    candidates.push(daemon_install_path());

    candidates.into_iter().find(|p| p.is_file())
}

fn desktop_entry() -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Waydroid Keymapper\nComment=Low-latency Waydroid gaming input mapper\nExec={}\nIcon=input-gaming\nTerminal=false\nCategories=Game;Utility;\nKeywords=Waydroid;Android;Gaming;Keymapper;\n",
        gui_install_path().to_string_lossy()
    )
}

fn service_unit() -> String {
    "[Unit]
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
"
    .into()
}

fn install_runtime() -> Result<String, String> {
    fs::create_dir_all(user_bin_dir()).map_err(|e| e.to_string())?;
    fs::create_dir_all(user_service_dir()).map_err(|e| e.to_string())?;
    fs::create_dir_all(profiles_dir()).map_err(|e| e.to_string())?;

    if let Some(src) = daemon_source() {
        install_user_executable(&src, &daemon_install_path())?;
    } else if !daemon_install_path().is_file() {
        return Err("Daemon binary not found. Build waydroid-keymapper first.".into());
    }

    if let Ok(exe) = env::current_exe() {
        if exe.is_file() {
            install_user_executable(&exe, &gui_install_path())?;
        }
    }

    let unit = user_service_dir().join(USER_SERVICE);
    let tmp = temp_path(&unit, "unit");
    fs::write(&tmp, service_unit()).map_err(|e| e.to_string())?;
    fs::rename(tmp, unit).map_err(|e| e.to_string())?;

    let desktop = desktop_file_path();
    if let Some(parent) = desktop.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let desktop_tmp = temp_path(&desktop, "desktop");
    fs::write(&desktop_tmp, desktop_entry()).map_err(|e| e.to_string())?;
    fs::rename(desktop_tmp, desktop).map_err(|e| e.to_string())?;

    systemctl_user(&["daemon-reload"])?;
    Ok("Installed / repaired ✓".into())
}

fn systemctl_user(args: &[&str]) -> Result<String, String> {
    let output = Command::new("systemctl")
        .args(["--user"])
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if err.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            err
        })
    }
}

fn service_action(action: &str) -> Result<String, String> {
    match action {
        "stop" => match systemctl_user(&["stop", USER_SERVICE]) {
            Ok(_) => Ok("Daemon stopped ✓".into()),
            Err(e) if e.contains("not found") || e.contains("not loaded") => Ok("Daemon already stopped".into()),
            Err(e) => Err(e),
        },
        "start" => {
            install_runtime()?;
            systemctl_user(&["start", USER_SERVICE])?;
            Ok("Daemon started ✓".into())
        }
        "restart" => {
            install_runtime()?;
            match systemctl_user(&["restart", USER_SERVICE]) {
                Ok(_) => Ok("Daemon restarted ✓".into()),
                Err(e) if e.contains("not found") || e.contains("not loaded") => {
                    systemctl_user(&["start", USER_SERVICE])?;
                    Ok("Daemon started ✓".into())
                }
                Err(e) => Err(e),
            }
        }
        "enable" => {
            install_runtime()?;
            systemctl_user(&["enable", USER_SERVICE])?;
            Ok("Daemon enabled at login ✓".into())
        }
        "disable" => match systemctl_user(&["disable", "--now", USER_SERVICE]) {
            Ok(_) => Ok("Daemon disabled".into()),
            Err(e) if e.contains("not found") || e.contains("not loaded") => Ok("Daemon was not enabled".into()),
            Err(e) => Err(e),
        },
        _ => Err("Unknown service action".into()),
    }
}

fn runtime_service_state() -> String {
    match Command::new("systemctl").args(["--user", "is-active", USER_SERVICE]).output() {
        Ok(o) if o.status.success() => "Running".into(),
        Ok(_) => {
            if user_service_dir().join(USER_SERVICE).is_file() {
                "Stopped".into()
            } else {
                "Not installed".into()
            }
        }
        Err(_) => "systemctl unavailable".into(),
    }
}

fn device_access(path: Option<String>) -> String {
    let Some(path) = path else { return "Auto-detect".into() };
    match fs::OpenOptions::new().read(true).open(&path) {
        Ok(_) => format!("OK · {path}"),
        Err(e) => format!("No access · {path} · {e}"),
    }
}

fn update_runtime_status(ui: &Ui) {
    let service = runtime_service_state();
    ui.runtime_status.set_text(&format!("Service · {service}"));

    let (kbd, mouse) = {
        let st = ui.state.borrow();
        (
            st.cfg.devices.keyboard.clone(),
            st.cfg.devices.mouse.clone(),
        )
    };
    ui.input_access.set_text(&format!(
        "Keyboard  ·  {}\nMouse     ·  {}",
        device_access(kbd),
        device_access(mouse)
    ));

    ui.waydroid_status.set_text(&format!("Waydroid · {}", waydroid_state()));

    match control::request("status") {
        Ok(reply) => {
            let locked = reply
                .split_whitespace()
                .find_map(|x| x.strip_prefix("locked="))
                .unwrap_or("0");
            let owner = reply
                .split_whitespace()
                .find_map(|x| x.strip_prefix("owner="))
                .unwrap_or("none");

            if locked == "1" {
                ui.lock_status.set_text(match owner {
                    "aim" => "Mouse · 🎯 AIM AUTO-LOCK",
                    "manual" => "Mouse · 🔒 MANUAL LOCK",
                    _ => "Mouse · 🔒 LOCKED",
                });
                ui.lock_status.set_css_classes(&["badge-ok"]);
            } else {
                ui.lock_status.set_text("Mouse · 🖱 UNLOCKED");
                ui.lock_status.set_css_classes(&["badge-warn"]);
            }
        }
        Err(_) => {
            ui.lock_status.set_text("Mouse · daemon offline");
            ui.lock_status.set_css_classes(&["badge-danger"]);
        }
    }
}

fn run_background<F>(ui: &Ui, label: &str, task: F)
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    ui.status.set_text(label);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });

    let ui2 = ui.clone();
    glib::timeout_add_local(Duration::from_millis(60), move || {
        match rx.try_recv() {
            Ok(Ok(message)) => {
                ui2.status.set_text(&message);
                update_runtime_status(&ui2);
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                ui2.status.set_text(&error);
                update_runtime_status(&ui2);
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                ui2.status.set_text("Background operation ended unexpectedly");
                glib::ControlFlow::Break
            }
        }
    });
}

fn validate_current(ui: &Ui) {
    sync_state_from_form(ui);
    let st = ui.state.borrow();

    if !st.cfg.conflicts().is_empty() {
        ui.status.set_text(&format!(
            "Validation failed · {} conflict(s)",
            st.cfg.conflicts().len()
        ));
        return;
    }

    match st.cfg.validate() {
        Ok(_) => ui.status.set_text("Configuration valid ✓"),
        Err(e) => ui.status.set_text(&format!("Validation failed · {e}")),
    }
}

fn diagnostics(ui: &Ui) {
    sync_state_from_form(ui);
    let cfg = ui.state.borrow().cfg.clone();
    let mut issues = Vec::new();

    if let Err(e) = cfg.validate_runtime() {
        issues.push(format!("config: {e}"));
    }
    if let Some(path) = &cfg.devices.keyboard {
        if fs::OpenOptions::new().read(true).open(path).is_err() {
            issues.push("keyboard device not accessible".into());
        }
    }
    if let Some(path) = &cfg.devices.mouse {
        if fs::OpenOptions::new().read(true).open(path).is_err() {
            issues.push("mouse device not accessible".into());
        }
    }
    if !Path::new(&cfg.touch_fifo()).exists() {
        issues.push(format!("touch FIFO missing: {}", cfg.touch_fifo()));
    }
    if cfg.aim.as_ref().is_some_and(|a| a.mode.eq_ignore_ascii_case("relative"))
        && !Path::new(&cfg.pointer_fifo()).exists()
    {
        issues.push(format!("pointer FIFO missing: {}", cfg.pointer_fifo()));
    }

    match control::request("ping") {
        Ok(reply) if reply == "OK pong" => {}
        Ok(reply) => issues.push(format!("daemon: {reply}")),
        Err(_) => issues.push("daemon socket offline".into()),
    }

    if issues.is_empty() {
        ui.status.set_text("Diagnostics · no issues found ✓");
    } else {
        ui.status.set_text(&format!("Diagnostics · {} issue(s): {}", issues.len(), issues.join(" · ")));
    }

    update_runtime_status(ui);
}

fn runtime_control(ui: &Ui, command: &str) {
    match control::request(command) {
        Ok(reply) => ui.status.set_text(&reply),
        Err(e) => ui.status.set_text(&format!("Daemon control unavailable · {e}")),
    }
    update_runtime_status(ui);
}

fn waydroid_state() -> String {
    match Command::new("waydroid").arg("status").output() {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout).trim().replace('\n', " · ");
            if text.contains("RUNNING") {
                "Running".into()
            } else if text.contains("STOPPED") {
                "Stopped".into()
            } else if text.is_empty() {
                "Unknown".into()
            } else {
                text
            }
        }
        Err(_) => "Unavailable".into(),
    }
}

fn waydroid_action(ui: &Ui, action: &str) {
    let action = action.to_string();
    run_background(ui, &format!("Waydroid {action}…"), move || {
        let output = Command::new("waydroid")
            .args(["session", &action])
            .output()
            .map_err(|e| format!("Waydroid command failed · {e}"))?;

        if output.status.success() {
            Ok(format!("Waydroid {action} ✓"))
        } else {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if err.is_empty() {
                format!("Waydroid {action} failed")
            } else {
                err
            })
        }
    });
}

fn apply_preset(ui: &Ui, preset: Preset) {
    sync_state_from_form(ui);
    let (cfg, label) = match preset {
        Preset::FreeFire => (preset_free_fire(), "Free Fire"),
        Preset::Pubg => (preset_pubg(), "PUBG"),
        Preset::Fps => (preset_fps(), "FPS / BR"),
        Preset::Minimal => (preset_minimal(), "Minimal"),
    };

    let mut new_cfg = cfg;
    let st = ui.state.borrow();
    new_cfg.display = st.cfg.display.clone();
    new_cfg.devices = st.cfg.devices.clone();
    new_cfg.performance = st.cfg.performance.clone();
    new_cfg.performance.mouse_lock = false;
    new_cfg.performance.auto_lock_on_aim = true;
    drop(st);

    {
        let mut st = ui.state.borrow_mut();
        st.cfg = new_cfg;
        st.selected = None;
        st.dirty = true;
    }

    sync_form(ui);
    rebuild_bindings(ui);
    selected_editor(ui);
    ui.canvas.queue_draw();
    ui.status.set_text(&format!("{label} preset loaded · review then Save"));
}

fn save_and_apply(ui: &Ui) {
    sync_state_from_form(ui);
    let cfg = ui.state.borrow().cfg.clone();

    if let Err(e) = cfg.validate_runtime() {
        ui.status.set_text(&format!("Cannot run · {e}"));
        return;
    }

    if let Err(e) = save_current(ui) {
        ui.status.set_text(&format!("Save failed · {e}"));
        return;
    }

    if let Err(e) = copy_active_config(&cfg) {
        ui.status.set_text(&format!("Active config failed · {e}"));
        return;
    }

    run_background(ui, "Installing runtime…", || service_action("restart"));
}

fn rebuild_profiles(ui: &Ui) {
    while let Some(child) = ui.profile_list.first_child() {
        ui.profile_list.remove(&child);
    }

    let current = current_profile_path(ui);
    let files = profile_files();

    let mut selected_row = None;
    for path in files {
        let row = ListBoxRow::new();
        let label = Label::new(Some(
            path.file_stem().and_then(|x| x.to_str()).unwrap_or("profile"),
        ));
        label.set_halign(gtk4::Align::Start);
        add_margins(&label, 7);
        row.set_child(Some(&label));

        if path == current {
            selected_row = Some(row.clone());
        }
        ui.profile_list.append(&row);
    }

    if let Some(row) = selected_row {
        ui.profile_list.select_row(Some(&row));
    }
}

fn load_selected_profile(ui: &Ui, row: &ListBoxRow) {
    let files = profile_files();
    let index = row.index();
    let Some(path) = files.get(index as usize) else { return };

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
            selected_editor(ui);
            ui.canvas.queue_draw();
            ui.status.set_text(&format!(
                "Loaded profile · {}",
                path.file_stem().and_then(|x| x.to_str()).unwrap_or("profile")
            ));
        }
        Err(e) => ui.status.set_text(&format!("Load failed · {e}")),
    }
}

fn ask_name(parent: &ApplicationWindow, title: &str, initial: &str, callback: impl Fn(String) + 'static) {
    let dialog = Dialog::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .build();
    dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
    dialog.add_button("OK", gtk4::ResponseType::Accept);

    let entry = Entry::new();
    entry.set_text(initial);
    add_margins(&entry, 14);
    dialog.content_area().append(&entry);

    dialog.connect_response(move |d, response| {
        if response == gtk4::ResponseType::Accept {
            callback(entry.text().trim().to_string());
        }
        d.close();
    });
    dialog.show();
}

fn new_profile(ui: &Ui, parent: &ApplicationWindow) {
    let ui2 = ui.clone();
    ask_name(parent, "New profile", "freefire", move |name| {
        let Some(safe) = safe_profile_name(&name) else {
            ui2.status.set_text("Invalid profile name");
            return;
        };

        let path = profiles_dir().join(format!("{safe}.toml"));
        if path.exists() {
            ui2.status.set_text("Profile already exists");
            return;
        }

        let cfg = preset_free_fire();
        match save_profile(&path, &cfg) {
            Ok(_) => {
                let mut st = ui2.state.borrow_mut();
                st.cfg = cfg;
                st.profile_path = path;
                st.selected = None;
                st.dirty = false;
                drop(st);

                rebuild_profiles(&ui2);
                sync_form(&ui2);
                rebuild_bindings(&ui2);
                selected_editor(&ui2);
                ui2.canvas.queue_draw();
                ui2.status.set_text("New profile created ✓");
            }
            Err(e) => ui2.status.set_text(&format!("Create failed · {e}")),
        }
    });
}

fn duplicate_profile(ui: &Ui, parent: &ApplicationWindow) {
    sync_state_from_form(ui);
    let initial = format!(
        "{}_copy",
        current_profile_path(ui)
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("profile")
    );
    let cfg = ui.state.borrow().cfg.clone();
    let ui2 = ui.clone();

    ask_name(parent, "Duplicate profile", &initial, move |name| {
        let Some(safe) = safe_profile_name(&name) else {
            ui2.status.set_text("Invalid profile name");
            return;
        };
        let path = profiles_dir().join(format!("{safe}.toml"));
        if path.exists() {
            ui2.status.set_text("Profile already exists");
            return;
        }

        match save_profile(&path, &cfg) {
            Ok(_) => {
                let mut st = ui2.state.borrow_mut();
                st.cfg = cfg.clone();
                st.profile_path = path;
                st.selected = None;
                st.dirty = false;
                drop(st);

                rebuild_profiles(&ui2);
                sync_form(&ui2);
                rebuild_bindings(&ui2);
                selected_editor(&ui2);
                ui2.canvas.queue_draw();
                ui2.status.set_text("Profile duplicated ✓");
            }
            Err(e) => ui2.status.set_text(&format!("Duplicate failed · {e}")),
        }
    });
}

fn delete_profile(ui: &Ui) {
    let current = current_profile_path(ui);
    let files = profile_files();

    if files.len() <= 1 {
        ui.status.set_text("Keep at least one profile");
        return;
    }

    match fs::remove_file(&current) {
        Ok(_) => {
            rebuild_profiles(ui);
            if let Some(path) = profile_files().first().cloned() {
                if let Ok(cfg) = load_profile(&path) {
                    let mut st = ui.state.borrow_mut();
                    st.cfg = cfg;
                    st.profile_path = path;
                    st.selected = None;
                    st.dirty = false;
                }
            }
            sync_form(ui);
            rebuild_bindings(ui);
            selected_editor(ui);
            ui.canvas.queue_draw();
            ui.status.set_text("Profile deleted");
        }
        Err(e) => ui.status.set_text(&format!("Delete failed · {e}")),
    }
}

fn install_input_permissions() -> Result<String, String> {
    let base = home_dir().join(".config/waydroid-keymapper");
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    let tmp = temp_path(&base, "udev");
    fs::write(
        &tmp,
        "SUBSYSTEM==\"input\", KERNEL==\"event*\", MODE=\"0660\", TAG+=\"uaccess\"\n",
    )
    .map_err(|e| e.to_string())?;

    let install = Command::new("pkexec")
        .args([
            "install",
            "-Dm644",
            tmp.to_string_lossy().as_ref(),
            "/etc/udev/rules.d/99-waydroid-keymapper.rules",
        ])
        .output()
        .map_err(|e| format!("pkexec unavailable · {e}"))?;

    let _ = fs::remove_file(&tmp);

    if !install.status.success() {
        return Err(String::from_utf8_lossy(&install.stderr).trim().into());
    }

    let reload = Command::new("pkexec")
        .args(["udevadm", "control", "--reload-rules"])
        .status()
        .map_err(|e| e.to_string())?;
    if !reload.success() {
        return Err("udevadm reload failed".into());
    }

    let trigger = Command::new("pkexec")
        .args(["udevadm", "trigger", "--subsystem-match=input"])
        .status()
        .map_err(|e| e.to_string())?;
    if !trigger.success() {
        return Err("udevadm trigger failed".into());
    }

    Ok("Input permissions repaired ✓".into())
}

fn build_ui(app: &Application) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let provider = CssProvider::new();
        provider.load_from_data(APP_CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let _ = ensure_profiles();
    let profile_path = profile_files()
        .first()
        .cloned()
        .unwrap_or_else(|| profiles_dir().join("default.toml"));

    let cfg = load_profile(&profile_path).unwrap_or_else(|_| default_config());
    let state = Rc::new(RefCell::new(State {
        cfg,
        profile_path,
        selected: None,
        dirty: false,
    }));

    let profile_list = ListBox::new();
    profile_list.set_selection_mode(gtk4::SelectionMode::Single);
    profile_list.set_vexpand(true);

    let bindings_box = GtkBox::new(Orientation::Vertical, 6);
    let selected_editor_box = GtkBox::new(Orientation::Vertical, 7);

    let canvas = DrawingArea::new();
    canvas.set_content_width(860);
    canvas.set_content_height(620);
    canvas.set_hexpand(true);
    canvas.set_vexpand(true);
    canvas.add_css_class("preview");

    let status = Label::new(Some("Ready"));
    status.set_halign(gtk4::Align::Start);
    status.add_css_class("muted");

    let profile_name = Entry::new();
    let width = spin(320.0, 16384.0, 1.0, 0);
    let height = spin(240.0, 16384.0, 1.0, 0);
    let keyboard = ComboBoxText::new();
    let mouse = ComboBoxText::new();

    let aim_enabled = CheckButton::with_label("Enable aim");
    let aim_button = Entry::new();
    let aim_mode = ComboBoxText::new();
    aim_mode.append(Some("touch"), "Touch / absolute");
    aim_mode.append(Some("relative"), "Relative / FPS");
    let aim_x = spin(0.0, 1.0, 0.01, 3);
    let aim_y = spin(0.0, 1.0, 0.01, 3);
    let aim_sensitivity = spin(0.01, 20.0, 0.05, 2);
    let aim_slot = spin(0.0, 15.0, 1.0, 0);
    let aim_invert_x = CheckButton::with_label("Invert X");
    let aim_invert_y = CheckButton::with_label("Invert Y");
    let aim_scale_x = spin(0.01, 20.0, 0.05, 2);
    let aim_scale_y = spin(0.01, 20.0, 0.05, 2);
    let aim_edge_margin = spin(0.0, 0.49, 0.01, 2);

    let joy_enabled = CheckButton::with_label("Enable WASD joystick");
    let joy_up = Entry::new();
    let joy_down = Entry::new();
    let joy_left = Entry::new();
    let joy_right = Entry::new();
    let joy_x = spin(0.0, 1.0, 0.01, 3);
    let joy_y = spin(0.0, 1.0, 0.01, 3);
    let joy_radius = spin(0.01, 0.9, 0.005, 3);
    let joy_slot = spin(0.0, 15.0, 1.0, 0);
    let joy_normalize = CheckButton::with_label("Normalize diagonals");

    let grab = CheckButton::with_label("Exclusive evdev grab");
    let realtime = CheckButton::with_label("Best-effort realtime");
    let realtime_priority = spin(1.0, 99.0, 1.0, 0);
    let fifo_write_retries = spin(1.0, 8.0, 1.0, 0);
    let fifo_write_wait = spin(0.0, 5.0, 1.0, 0);
    let fifo_reconnect = spin(5.0, 2000.0, 5.0, 0);
    let touch_pressure = spin(1.0, 255.0, 1.0, 0);
    let touch_major = spin(1.0, 255.0, 1.0, 0);
    let touch_minor = spin(1.0, 255.0, 1.0, 0);

    let mouse_lock = CheckButton::with_label("Lock mouse on startup");
    let auto_lock_on_aim = CheckButton::with_label("Auto-lock while physical Aim is held");
    let mouse_toggle = Entry::new();

    let runtime_status = Label::new(Some("Service · checking…"));
    let lock_status = Label::new(Some("Mouse · checking…"));
    let input_access = Label::new(Some("Checking input devices…"));
    let waydroid_status = Label::new(Some("Waydroid · checking…"));

    runtime_status.set_halign(gtk4::Align::Start);
    lock_status.set_halign(gtk4::Align::Start);
    input_access.set_halign(gtk4::Align::Start);
    input_access.set_wrap(true);
    waydroid_status.set_halign(gtk4::Align::Start);

    let ui = Ui {
        state: state.clone(),
        profile_list: profile_list.clone(),
        bindings_box: bindings_box.clone(),
        selected_editor: selected_editor_box.clone(),
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
        auto_lock_on_aim: auto_lock_on_aim.clone(),
        mouse_toggle: mouse_toggle.clone(),
        runtime_status: runtime_status.clone(),
        lock_status: lock_status.clone(),
        input_access: input_access.clone(),
        waydroid_status: waydroid_status.clone(),
    };

    let root = GtkBox::new(Orientation::Vertical, 0);

    let header = GtkBox::new(Orientation::Horizontal, 10);
    header.add_css_class("header");

    let title_box = GtkBox::new(Orientation::Vertical, 1);
    let title = Label::new(Some("Waydroid Keymapper"));
    title.add_css_class("title");
    title.set_halign(gtk4::Align::Start);
    let subtitle = Label::new(Some("Low-latency shooter input · safe mouse ownership · GTK4 editor"));
    subtitle.add_css_class("subtitle");
    subtitle.set_halign(gtk4::Align::Start);
    title_box.append(&title);
    title_box.append(&subtitle);
    title_box.set_hexpand(true);

    let validate = Button::with_label("Validate");
    let save = Button::with_label("Save");
    save.add_css_class("primary");
    let apply = Button::with_label("Apply & Run");
    apply.add_css_class("success");

    header.append(&title_box);
    header.append(&validate);
    header.append(&save);
    header.append(&apply);
    root.append(&header);

    let main_paned = Paned::new(Orientation::Horizontal);
    main_paned.set_wide_handle(true);
    main_paned.set_position(250);

    let left = GtkBox::new(Orientation::Vertical, 8);
    add_margins(&left, 8);

    let profiles_card = section(&left, "Profiles");
    let profile_buttons = GtkBox::new(Orientation::Horizontal, 5);
    let new_btn = Button::with_label("New");
    let duplicate_btn = Button::with_label("Duplicate");
    let delete_btn = Button::with_label("Delete");
    profile_buttons.append(&new_btn);
    profile_buttons.append(&duplicate_btn);
    profile_buttons.append(&delete_btn);
    profiles_card.append(&profile_buttons);

    let profile_scroll = ScrolledWindow::new();
    profile_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    profile_scroll.set_vexpand(true);
    profile_scroll.set_child(Some(&profile_list));
    profiles_card.append(&profile_scroll);

    let controls_card = section(&left, "Mappings");
    let add_row1 = GtkBox::new(Orientation::Horizontal, 5);
    let add_tap = Button::with_label("+ Key TAP");
    let add_hold = Button::with_label("+ Key HOLD");
    add_row1.append(&add_tap);
    add_row1.append(&add_hold);
    controls_card.append(&add_row1);

    let add_row2 = GtkBox::new(Orientation::Horizontal, 5);
    let add_mouse_tap = Button::with_label("+ Mouse TAP");
    let add_mouse_hold = Button::with_label("+ Mouse HOLD");
    add_row2.append(&add_mouse_tap);
    add_row2.append(&add_mouse_hold);
    controls_card.append(&add_row2);

    let binding_scroll = ScrolledWindow::new();
    binding_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    binding_scroll.set_vexpand(true);
    binding_scroll.set_child(Some(&bindings_box));
    controls_card.append(&binding_scroll);

    let center = GtkBox::new(Orientation::Vertical, 7);
    add_margins(&center, 8);
    center.append(&canvas);

    // The editor card belongs inside the scrolled window. Do not first
    // append it to center because GTK4 widgets cannot have two parents.
    let editor_card = GtkBox::new(Orientation::Vertical, 7);
    editor_card.add_css_class("card");
    let editor_title = Label::new(Some("Selected control"));
    editor_title.add_css_class("section-title");
    editor_title.set_halign(gtk4::Align::Start);
    editor_card.append(&editor_title);
    editor_card.append(&selected_editor_box);

    let editor_scroll = ScrolledWindow::new();
    editor_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    editor_scroll.set_child(Some(&editor_card));
    editor_scroll.set_min_content_height(145);
    center.append(&editor_scroll);
    center.append(&status);

    let right = GtkBox::new(Orientation::Vertical, 5);
    add_margins(&right, 8);
    let settings_scroll = ScrolledWindow::new();
    settings_scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
    settings_scroll.set_min_content_width(360);
    settings_scroll.set_child(Some(&right));

    let preset = section(&right, "Shooter presets");
    let preset_help = Label::new(Some("Safe defaults: mouse starts UNLOCKED. F8 toggles manual lock. RMB auto-lock only happens after a real RMB press."));
    preset_help.add_css_class("help");
    preset_help.set_wrap(true);
    preset_help.set_halign(gtk4::Align::Start);
    preset.append(&preset_help);

    let preset_row1 = GtkBox::new(Orientation::Horizontal, 5);
    let freefire = Button::with_label("Free Fire");
    let pubg = Button::with_label("PUBG");
    let fps = Button::with_label("FPS / BR");
    let minimal = Button::with_label("Minimal");
    preset_row1.append(&freefire);
    preset_row1.append(&pubg);
    preset_row1.append(&fps);
    preset_row1.append(&minimal);
    preset.append(&preset_row1);

    let general = section(&right, "Profile & display");
    let gg = Grid::new();
    gg.set_row_spacing(7);
    gg.set_column_spacing(8);
    form_row(&gg, 0, "Profile", &profile_name);
    form_row(&gg, 1, "Width", &width);
    form_row(&gg, 2, "Height", &height);
    general.append(&gg);

    let devices_card = section(&right, "Input devices");
    let dg = Grid::new();
    dg.set_row_spacing(7);
    dg.set_column_spacing(8);
    form_row(&dg, 0, "Keyboard", &keyboard);
    form_row(&dg, 1, "Mouse", &mouse);
    devices_card.append(&dg);
    let device_row = GtkBox::new(Orientation::Horizontal, 5);
    let refresh = Button::with_label("Refresh devices");
    let repair = Button::with_label("Repair permissions");
    device_row.append(&refresh);
    device_row.append(&repair);
    devices_card.append(&device_row);
    devices_card.append(&input_access);

    let aim_card = section(&right, "Aim");
    aim_card.append(&aim_enabled);
    let ag = Grid::new();
    ag.set_row_spacing(7);
    ag.set_column_spacing(8);
    form_row(&ag, 0, "Button", &aim_button);
    form_row(&ag, 1, "Mode", &aim_mode);
    form_row(&ag, 2, "Center X", &aim_x);
    form_row(&ag, 3, "Center Y", &aim_y);
    form_row(&ag, 4, "Sensitivity", &aim_sensitivity);
    form_row(&ag, 5, "Slot", &aim_slot);
    form_row(&ag, 6, "X scale", &aim_scale_x);
    form_row(&ag, 7, "Y scale", &aim_scale_y);
    form_row(&ag, 8, "Edge margin", &aim_edge_margin);
    ag.attach(&aim_invert_x, 1, 9, 1, 1);
    ag.attach(&aim_invert_y, 1, 10, 1, 1);
    aim_card.append(&ag);
    let aim_help = Label::new(Some("Relative mode = unbounded FPS camera. Touch mode = Android multitouch aim with edge recentering."));
    aim_help.add_css_class("help");
    aim_help.set_wrap(true);
    aim_help.set_halign(gtk4::Align::Start);
    aim_card.append(&aim_help);

    let joystick_card = section(&right, "WASD joystick");
    joystick_card.append(&joy_enabled);
    let jg = Grid::new();
    jg.set_row_spacing(7);
    jg.set_column_spacing(8);
    form_row(&jg, 0, "Up", &joy_up);
    form_row(&jg, 1, "Down", &joy_down);
    form_row(&jg, 2, "Left", &joy_left);
    form_row(&jg, 3, "Right", &joy_right);
    form_row(&jg, 4, "Center X", &joy_x);
    form_row(&jg, 5, "Center Y", &joy_y);
    form_row(&jg, 6, "Radius", &joy_radius);
    form_row(&jg, 7, "Slot", &joy_slot);
    jg.attach(&joy_normalize, 1, 8, 1, 1);
    joystick_card.append(&jg);

    let performance_card = section(&right, "Runtime & latency");
    performance_card.append(&grab);
    performance_card.append(&realtime);
    performance_card.append(&mouse_lock);
    performance_card.append(&auto_lock_on_aim);
    let pg = Grid::new();
    pg.set_row_spacing(7);
    pg.set_column_spacing(8);
    let toggle_box = GtkBox::new(Orientation::Horizontal, 5);
    toggle_box.append(&mouse_toggle);
    let capture_toggle = Button::with_label("Capture");
    toggle_box.append(&capture_toggle);
    form_row(&pg, 0, "Lock key", &toggle_box);
    form_row(&pg, 1, "RT priority", &realtime_priority);
    form_row(&pg, 2, "FIFO retries", &fifo_write_retries);
    form_row(&pg, 3, "FIFO wait ms", &fifo_write_wait);
    form_row(&pg, 4, "Reconnect ms", &fifo_reconnect);
    performance_card.append(&pg);
    let performance_help = Label::new(Some("Startup lock is OFF by default. Emergency unlock: Ctrl+Alt+F12. The GUI control buttons also override the runtime lock safely."));
    performance_help.add_css_class("help");
    performance_help.set_wrap(true);
    performance_help.set_halign(gtk4::Align::Start);
    performance_card.append(&performance_help);

    let touch_card = section(&right, "Android touch tuning");
    let tg = Grid::new();
    tg.set_row_spacing(7);
    tg.set_column_spacing(8);
    form_row(&tg, 0, "Pressure", &touch_pressure);
    form_row(&tg, 1, "Major", &touch_major);
    form_row(&tg, 2, "Minor", &touch_minor);
    touch_card.append(&tg);

    let runtime_card = section(&right, "Daemon & Waydroid");
    runtime_card.append(&runtime_status);
    runtime_card.append(&lock_status);
    runtime_card.append(&waydroid_status);

    let rr1 = GtkBox::new(Orientation::Horizontal, 5);
    let install = Button::with_label("Install / Repair");
    let start = Button::with_label("Start");
    let stop = Button::with_label("Stop");
    let restart = Button::with_label("Restart");
    rr1.append(&install);
    rr1.append(&start);
    rr1.append(&stop);
    rr1.append(&restart);
    runtime_card.append(&rr1);

    let rr2 = GtkBox::new(Orientation::Horizontal, 5);
    let enable = Button::with_label("Enable login");
    let disable = Button::with_label("Disable login");
    let diagnostic = Button::with_label("Diagnostics");
    rr2.append(&enable);
    rr2.append(&disable);
    rr2.append(&diagnostic);
    runtime_card.append(&rr2);

    let rr3 = GtkBox::new(Orientation::Horizontal, 5);
    let lock = Button::with_label("🔒 Lock");
    let unlock = Button::with_label("🖱 Unlock");
    let toggle = Button::with_label("Toggle");
    rr3.append(&lock);
    rr3.append(&unlock);
    rr3.append(&toggle);
    runtime_card.append(&rr3);

    let rr4 = GtkBox::new(Orientation::Horizontal, 5);
    let wd_start = Button::with_label("Start Waydroid");
    let wd_stop = Button::with_label("Stop Waydroid");
    rr4.append(&wd_start);
    rr4.append(&wd_stop);
    runtime_card.append(&rr4);

    main_paned.set_start_child(Some(&left));
    main_paned.set_resize_start_child(false);
    main_paned.set_shrink_start_child(false);

    let center_right = Paned::new(Orientation::Horizontal);
    center_right.set_wide_handle(true);
    center_right.set_position(900);
    center_right.set_start_child(Some(&center));
    center_right.set_end_child(Some(&settings_scroll));
    center_right.set_resize_start_child(true);
    center_right.set_shrink_start_child(false);
    center_right.set_resize_end_child(false);
    main_paned.set_end_child(Some(&center_right));

    root.append(&main_paned);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Waydroid Keymapper")
        .default_width(1600)
        .default_height(920)
        .build();
    window.set_child(Some(&root));

    canvas.set_draw_func({
        let state = state.clone();
        move |area, cr, w, h| draw_canvas(&state, area, cr, w, h)
    });

    let pointer_state = state.clone();
    let pointer_to_norm: Rc<dyn Fn(f64, f64) -> (f32, f32)> = {
        let canvas = canvas.clone();
        Rc::new(move |x: f64, y: f64| -> (f32, f32) {
            let st = pointer_state.borrow();
            let pad = 18.0;
            let cw = (canvas.width() as f64 - 2.0 * pad).max(10.0);
            let ch = (canvas.height() as f64 - 2.0 * pad).max(10.0);
            let scale = (cw / st.cfg.display.width.max(1) as f64)
                .min(ch / st.cfg.display.height.max(1) as f64);
            let vw = st.cfg.display.width as f64 * scale;
            let vh = st.cfg.display.height as f64 * scale;
            let ox = (canvas.width() as f64 - vw) / 2.0;
            let oy = (canvas.height() as f64 - vh) / 2.0;
            (
                ((x - ox) / vw).clamp(0.0, 1.0) as f32,
                ((y - oy) / vh).clamp(0.0, 1.0) as f32,
            )
        })
    };

    let click = gtk4::GestureClick::new();
    let ui_click = ui.clone();
    let ptn_click = pointer_to_norm.clone();
    click.connect_released(move |_, _, x, y| {
        let (nx, ny) = ptn_click(x, y);
        // Keep the RefCell borrow scoped to the lookup. Otherwise GTK's
        // callback can still hold the immutable borrow when select_binding()
        // requests a mutable borrow, causing "RefCell already borrowed".
        let selected = {
            let state = ui_click.state.borrow();
            nearest_binding(&state.cfg, nx, ny)
        };
        if let Some(sel) = selected {
            select_binding(&ui_click, sel);
        }
    });
    canvas.add_controller(click);

    let double = gtk4::GestureClick::new();
    let ui_double = ui.clone();
    let ptn_double = pointer_to_norm.clone();
    double.connect_pressed(move |gesture, presses, x, y| {
        if presses != 2 {
            return;
        }
        let (nx, ny) = ptn_double(x, y);
        let kind = if gesture.current_button() == 3 {
            EditType::MouseTap
        } else {
            EditType::KeyboardTap
        };
        open_binding_dialog_at(&ui_double, None, kind, nx, ny);
    });
    canvas.add_controller(double);

    let drag = gtk4::GestureDrag::new();
    let drag_item = Rc::new(Cell::new(None::<BindingRef>));
    {
        let ui_drag = ui.clone();
        let ptn = pointer_to_norm.clone();
        let drag_item = drag_item.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let (nx, ny) = ptn(x, y);
            let selected = nearest_binding(&ui_drag.state.borrow().cfg, nx, ny);
            drag_item.set(selected);
            if let Some(sel) = selected {
                select_binding(&ui_drag, sel);
            }
        });
    }
    {
        let ui_drag = ui.clone();
        let drag_item_update = drag_item.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(sel) = drag_item_update.get() else { return };

            // Recompute from actual widget dimensions instead of relying on the
            // absolute pointer position of the gesture.
            let st = ui_drag.state.borrow();
            let pad = 18.0;
            let cw = (ui_drag.canvas.width() as f64 - 2.0 * pad).max(10.0);
            let ch = (ui_drag.canvas.height() as f64 - 2.0 * pad).max(10.0);
            let scale = (cw / st.cfg.display.width.max(1) as f64)
                .min(ch / st.cfg.display.height.max(1) as f64);
            let vw = st.cfg.display.width as f64 * scale;
            let vh = st.cfg.display.height as f64 * scale;
            drop(st);

            let mut state = ui_drag.state.borrow_mut();
            if let Some((px, py)) = selected_position(&state.cfg, sel) {
                let nx = (px + dx as f32 / vw as f32).clamp(0.0, 1.0);
                let ny = (py + dy as f32 / vh as f32).clamp(0.0, 1.0);
                set_selected_position(&mut state.cfg, sel, nx, ny);
                state.dirty = true;
            }
            drop(state);
            ui_drag.canvas.queue_draw();
            selected_editor(&ui_drag);
        });
        let drag_item_end = drag_item.clone();
        drag.connect_drag_end(move |_, _, _| drag_item_end.set(None));
    }
    canvas.add_controller(drag);

    let key_controller = EventControllerKey::new();
    let ui_key = ui.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| {
        let name = key.name().map(|x| x.to_string()).unwrap_or_default();
        let Some(sel) = ui_key.state.borrow().selected else {
            return glib::Propagation::Proceed;
        };
        if name == "Delete" {
            delete_binding(&ui_key, sel);
            return glib::Propagation::Stop;
        }
        let delta = 0.005_f32;
        let mut st = ui_key.state.borrow_mut();
        if let Some((x, y)) = selected_position(&st.cfg, sel) {
            let (nx, ny) = match name.as_str() {
                "Left" => ((x - delta).clamp(0.0, 1.0), y),
                "Right" => ((x + delta).clamp(0.0, 1.0), y),
                "Up" => (x, (y - delta).clamp(0.0, 1.0)),
                "Down" => (x, (y + delta).clamp(0.0, 1.0)),
                _ => return glib::Propagation::Proceed,
            };
            set_selected_position(&mut st.cfg, sel, nx, ny);
            st.dirty = true;
            drop(st);
            ui_key.canvas.queue_draw();
            selected_editor(&ui_key);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    profile_list.connect_row_selected({
        let ui = ui.clone();
        move |_, row| {
            if let Some(row) = row {
                load_selected_profile(&ui, row);
            }
        }
    });

    {
        let ui = ui.clone();
        let window_for_new = window.clone();
        new_btn.connect_clicked(move |_| new_profile(&ui, &window_for_new));
    }
    {
        let ui = ui.clone();
        let window = window.clone();
        duplicate_btn.connect_clicked(move |_| duplicate_profile(&ui, &window));
    }
    {
        let ui = ui.clone();
        delete_btn.connect_clicked(move |_| delete_profile(&ui));
    }
    {
        let ui = ui.clone();
        validate.connect_clicked(move |_| validate_current(&ui));
    }
    {
        let ui = ui.clone();
        save.connect_clicked(move |_| match save_current(&ui) {
            Ok(_) => {
                rebuild_profiles(&ui);
                ui.status.set_text("Profile saved ✓");
            }
            Err(e) => ui.status.set_text(&format!("Save failed · {e}")),
        });
    }
    {
        let ui = ui.clone();
        apply.connect_clicked(move |_| save_and_apply(&ui));
    }

    for (button, preset) in [
        (freefire.clone(), Preset::FreeFire),
        (pubg.clone(), Preset::Pubg),
        (fps.clone(), Preset::Fps),
        (minimal.clone(), Preset::Minimal),
    ] {
        let ui = ui.clone();
        button.connect_clicked(move |_| apply_preset(&ui, preset));
    }

    {
        let ui = ui.clone();
        add_tap.connect_clicked(move |_| open_binding_dialog_at(&ui, None, EditType::KeyboardTap, 0.5, 0.5));
    }
    {
        let ui = ui.clone();
        add_hold.connect_clicked(move |_| open_binding_dialog_at(&ui, None, EditType::KeyboardHold, 0.5, 0.5));
    }
    {
        let ui = ui.clone();
        add_mouse_tap.connect_clicked(move |_| open_binding_dialog_at(&ui, None, EditType::MouseTap, 0.5, 0.5));
    }
    {
        let ui = ui.clone();
        add_mouse_hold.connect_clicked(move |_| open_binding_dialog_at(&ui, None, EditType::MouseHold, 0.5, 0.5));
    }

    {
        let ui = ui.clone();
        refresh.connect_clicked(move |_| {
            sync_state_from_form(&ui);
            let cfg = ui.state.borrow().cfg.clone();
            fill_devices(&ui.keyboard, &cfg.devices.keyboard, false);
            fill_devices(&ui.mouse, &cfg.devices.mouse, true);
            ui.status.set_text("Devices refreshed ✓");
        });
    }

    {
        let ui = ui.clone();
        repair.connect_clicked(move |_| {
            run_background(&ui, "Repairing input permissions…", install_input_permissions);
        });
    }

    {
        let ui = ui.clone();
        install.connect_clicked(move |_| run_background(&ui, "Installing runtime…", install_runtime));
    }
    {
        let ui = ui.clone();
        start.connect_clicked(move |_| run_background(&ui, "Starting daemon…", || service_action("start")));
    }
    {
        let ui = ui.clone();
        stop.connect_clicked(move |_| run_background(&ui, "Stopping daemon…", || service_action("stop")));
    }
    {
        let ui = ui.clone();
        restart.connect_clicked(move |_| run_background(&ui, "Restarting daemon…", || service_action("restart")));
    }
    {
        let ui = ui.clone();
        enable.connect_clicked(move |_| run_background(&ui, "Enabling login service…", || service_action("enable")));
    }
    {
        let ui = ui.clone();
        disable.connect_clicked(move |_| run_background(&ui, "Disabling login service…", || service_action("disable")));
    }
    {
        let ui = ui.clone();
        diagnostic.connect_clicked(move |_| diagnostics(&ui));
    }
    {
        let ui = ui.clone();
        lock.connect_clicked(move |_| runtime_control(&ui, "lock"));
    }
    {
        let ui = ui.clone();
        unlock.connect_clicked(move |_| runtime_control(&ui, "unlock"));
    }
    {
        let ui = ui.clone();
        toggle.connect_clicked(move |_| runtime_control(&ui, "toggle"));
    }
    {
        let ui = ui.clone();
        wd_start.connect_clicked(move |_| waydroid_action(&ui, "start"));
    }
    {
        let ui = ui.clone();
        wd_stop.connect_clicked(move |_| waydroid_action(&ui, "stop"));
    }

    {
        let armed = Rc::new(Cell::new(false));
        let armed_button = armed.clone();
        let ui_capture = ui.clone();
        capture_toggle.connect_clicked(move |_| {
            armed_button.set(true);
            ui_capture.mouse_toggle.grab_focus();
            ui_capture.status.set_text("Press the desired lock key…");
        });

        let controller = EventControllerKey::new();
        let armed_key = armed.clone();
        let entry = mouse_toggle.clone();
        let ui_key_capture = ui.clone();
        controller.connect_key_pressed(move |_, key, _, _| {
            if !armed_key.get() {
                return glib::Propagation::Proceed;
            }
            if let Some(name) = key.name() {
                let n = name.to_string();
                if !n.is_empty() {
                    entry.set_text(&key_alias(&n));
                    armed_key.set(false);
                    ui_key_capture.status.set_text("Lock key captured ✓");
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        mouse_toggle.add_controller(controller);
    }

    rebuild_profiles(&ui);
    sync_form(&ui);
    rebuild_bindings(&ui);
    selected_editor(&ui);
    update_runtime_status(&ui);

    {
        let ui = ui.clone();
        glib::timeout_add_local(Duration::from_millis(750), move || {
            update_runtime_status(&ui);
            glib::ControlFlow::Continue
        });
    }

    window.present();
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_name_is_safe() {
        assert_eq!(safe_profile_name("Free Fire").as_deref(), Some("Free_Fire"));
        assert_eq!(safe_profile_name("../escape").as_deref(), Some("escape"));
        assert!(safe_profile_name("___").is_none());
    }

    #[test]
    fn next_slot_prefers_first_free_slot() {
        let mut cfg = default_config();
        cfg.joystick.as_mut().unwrap().slot = 0;
        cfg.aim.as_mut().unwrap().slot = 1;
        assert_eq!(next_slot(&cfg), 6);
    }
}
