use crate::{
    config::Config,
    input::{button_code, key_code, KeyAction, MouseAction},
};
use std::{
    error::Error,
    io,
    os::fd::{AsRawFd, FromRawFd},
    sync::Arc,
    time::{Duration, Instant},
};

const SYN: u16 = 0;
const KEY: u16 = 1;
const REL: u16 = 2;
const ABS: u16 = 3;
const BTN_TOUCH: u16 = 330;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;
const SLOT: u16 = 47;
const MAJOR: u16 = 48;
const MINOR: u16 = 49;
const X: u16 = 53;
const Y: u16 = 54;
const ID: u16 = 57;
const PRESS: u16 = 58;
const MAX_INPUT_CODE: usize = 1024;
const MAX_TOUCH_SLOTS: usize = 16;

#[repr(C)]
#[derive(Clone, Copy)]
struct LinuxInputEvent {
    tv_sec: i64,
    tv_usec: i64,
    type_: u16,
    code: u16,
    value: i32,
}

struct Pipe {
    path: String,
    file: Option<std::fs::File>,
    next_connect: Instant,
    reconnect: Duration,
    retries: u8,
    wait: Duration,
}

impl Pipe {
    fn new(path: String, reconnect_ms: u64, retries: u8, wait_ms: u64) -> Self {
        Self {
            path,
            file: None,
            next_connect: Instant::now(),
            reconnect: Duration::from_millis(reconnect_ms.max(5)),
            retries: retries.max(1),
            wait: Duration::from_millis(wait_ms),
        }
    }

    fn disconnect(&mut self) {
        self.file = None;
        self.next_connect = Instant::now() + self.reconnect;
    }

    fn connect(&mut self) -> io::Result<()> {
        if self.file.is_some() {
            return Ok(());
        }

        let now = Instant::now();
        if now < self.next_connect {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "FIFO reconnect backoff"));
        }

        let c = std::ffi::CString::new(self.path.as_str())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid FIFO path"))?;

        let fd = unsafe {
            libc::open(
                c.as_ptr(),
                libc::O_WRONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };

        if fd < 0 {
            self.next_connect = now + self.reconnect;
            return Err(io::Error::last_os_error());
        }

        self.file = Some(unsafe { std::fs::File::from_raw_fd(fd) });
        self.next_connect = now;
        Ok(())
    }

    fn send(&mut self, events: &[(u16, u16, i32)]) {
        self.send_with_policy(events, self.retries);
    }

    fn send_critical(&mut self, events: &[(u16, u16, i32)]) {
        self.send_with_policy(events, self.retries.max(8));
    }

    fn send_with_policy(&mut self, events: &[(u16, u16, i32)], retries: u8) {
        if events.is_empty() {
            return;
        }

        let mut bytes = Vec::with_capacity(events.len() * std::mem::size_of::<LinuxInputEvent>());
        for &(type_, code, value) in events {
            let event = LinuxInputEvent {
                tv_sec: 0,
                tv_usec: 0,
                type_,
                code,
                value,
            };
            let raw = unsafe {
                std::slice::from_raw_parts(
                    (&event as *const LinuxInputEvent).cast::<u8>(),
                    std::mem::size_of::<LinuxInputEvent>(),
                )
            };
            bytes.extend_from_slice(raw);
        }

        if self.file.is_none() && self.connect().is_err() {
            return;
        }

        for attempt in 0..retries.max(1) {
            let Some(file) = self.file.as_mut() else { return };
            let fd = file.as_raw_fd();

            match write_nonblocking_all(fd, &bytes, self.wait) {
                Ok(()) => return,
                Err(e) if matches!(e.raw_os_error(), Some(libc::ENOENT | libc::ENXIO | libc::EPIPE | libc::EBADF)) => {
                    self.disconnect();
                    if attempt + 1 < retries {
                        std::thread::sleep(self.wait);
                        let _ = self.connect();
                        continue;
                    }
                    return;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if attempt + 1 < retries {
                        continue;
                    }
                    return;
                }
                Err(_) => {
                    self.disconnect();
                    return;
                }
            }
        }
    }
}

fn write_nonblocking_all(fd: i32, data: &[u8], wait: Duration) -> io::Result<()> {
    let mut offset = 0usize;

    while offset < data.len() {
        let n = unsafe {
            libc::write(
                fd,
                data[offset..].as_ptr().cast::<libc::c_void>(),
                data.len() - offset,
            )
        };

        if n > 0 {
            offset += n as usize;
            continue;
        }

        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "FIFO write returned zero"));
        }

        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EAGAIN) => {
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLOUT,
                    revents: 0,
                };
                let timeout = wait.as_millis().min(i32::MAX as u128) as i32;
                let rc = unsafe { libc::poll(&mut pfd, 1, timeout) };
                if rc == 0 {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "FIFO remains full"));
                }
                if rc < 0 {
                    let poll_err = io::Error::last_os_error();
                    if poll_err.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(poll_err);
                }
            }
            _ => return Err(err),
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct TouchSlot {
    down: bool,
}

#[derive(Clone, Copy)]
struct JoyRuntime {
    up: u16,
    down: u16,
    left: u16,
    right: u16,
    center_x: f32,
    center_y: f32,
    radius: f32,
    normalize_diagonal: bool,
    slot: u8,
}

#[derive(Clone, Copy)]
struct AimRuntime {
    center_x: f32,
    center_y: f32,
    sensitivity: f32,
    slot: u8,
    invert_x: bool,
    invert_y: bool,
    scale_x: f32,
    scale_y: f32,
    edge_margin: f32,
    relative: bool,
    continuous: bool,
    button: Option<u16>,
}

pub struct Mapper {
    cfg: Arc<Config>,
    touch: Pipe,
    pointer: Pipe,
    slots: [TouchSlot; MAX_TOUCH_SLOTS],
    next_tracking_id: i32,
    mx: f32,
    my: f32,
    aim_active: bool,
    mouse_locked: bool,
    rel_acc_x: f32,
    rel_acc_y: f32,
    keys: [bool; MAX_INPUT_CODE],
    key_actions: Box<[Option<KeyAction>]>,
    mouse_actions: Box<[Option<MouseAction>]>,
    joystick: Option<JoyRuntime>,
    aim_cfg: Option<AimRuntime>,
    touch_cfg: (i32, i32, i32),
    mouse_hold_slots: [bool; MAX_TOUCH_SLOTS],
}

impl Mapper {
    pub fn new(cfg: Config) -> Result<Self, Box<dyn Error>> {
        cfg.validate()?;

        let mut key_actions = vec![None; MAX_INPUT_CODE];
        let mut mouse_actions = vec![None; MAX_INPUT_CODE];

        let joystick = if let Some(j) = &cfg.joystick {
            let up = key_code(&j.up)?;
            let down = key_code(&j.down)?;
            let left = key_code(&j.left)?;
            let right = key_code(&j.right)?;

            key_actions[up as usize] = Some(KeyAction::Joystick);
            key_actions[down as usize] = Some(KeyAction::Joystick);
            key_actions[left as usize] = Some(KeyAction::Joystick);
            key_actions[right as usize] = Some(KeyAction::Joystick);

            Some(JoyRuntime {
                up,
                down,
                left,
                right,
                center_x: j.center_x,
                center_y: j.center_y,
                radius: j.radius,
                normalize_diagonal: j.normalize_diagonal,
                slot: j.slot,
            })
        } else {
            None
        };

        let aim_cfg = if let Some(a) = &cfg.aim {
            let continuous = a.is_continuous();
            let button = if continuous {
                None
            } else {
                Some(button_code(&a.button)?)
            };

            if let Some(code) = button {
                mouse_actions[code as usize] = Some(MouseAction::Aim);
            }

            Some(AimRuntime {
                center_x: a.center_x,
                center_y: a.center_y,
                sensitivity: a.sensitivity,
                slot: a.slot,
                invert_x: a.invert_x,
                invert_y: a.invert_y,
                scale_x: a.scale_x,
                scale_y: a.scale_y,
                edge_margin: a.edge_margin,
                relative: a.mode.eq_ignore_ascii_case("relative"),
                continuous,
                button,
            })
        } else {
            None
        };

        for x in &cfg.taps {
            key_actions[key_code(&x.key)? as usize] =
                Some(KeyAction::Tap { slot: x.slot, x: x.x, y: x.y });
        }

        for x in &cfg.holds {
            key_actions[key_code(&x.key)? as usize] =
                Some(KeyAction::Hold { slot: x.slot, x: x.x, y: x.y });
        }

        for x in &cfg.mouse_taps {
            mouse_actions[button_code(&x.button)? as usize] =
                Some(MouseAction::Tap { slot: x.slot, x: x.x, y: x.y });
        }

        for x in &cfg.mouse_holds {
            mouse_actions[button_code(&x.button)? as usize] =
                Some(MouseAction::Hold { slot: x.slot, x: x.x, y: x.y });
        }

        let mut mouse_hold_slots = [false; MAX_TOUCH_SLOTS];
        for x in &cfg.mouse_holds {
            mouse_hold_slots[x.slot as usize] = true;
        }

        let perf = &cfg.performance;
        let touch_cfg = (cfg.touch.pressure, cfg.touch.major, cfg.touch.minor);

        Ok(Self {
            touch: Pipe::new(
                cfg.touch_fifo(),
                perf.fifo_reconnect_ms,
                perf.fifo_write_retries,
                perf.fifo_write_wait_ms,
            ),
            pointer: Pipe::new(
                cfg.pointer_fifo(),
                perf.fifo_reconnect_ms,
                perf.fifo_write_retries,
                perf.fifo_write_wait_ms,
            ),
            cfg: Arc::new(cfg),
            slots: [TouchSlot { down: false }; MAX_TOUCH_SLOTS],
            next_tracking_id: 1,
            mx: 0.5,
            my: 0.5,
            aim_active: false,
            mouse_locked: false,
            rel_acc_x: 0.0,
            rel_acc_y: 0.0,
            keys: [false; MAX_INPUT_CODE],
            key_actions: key_actions.into_boxed_slice(),
            mouse_actions: mouse_actions.into_boxed_slice(),
            joystick,
            aim_cfg,
            touch_cfg,
            mouse_hold_slots,
        })
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    pub fn is_mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    pub fn set_mouse_lock(&mut self, locked: bool) {
        if self.mouse_locked == locked {
            return;
        }

        self.mouse_locked = locked;

        if locked {
            self.rel_acc_x = 0.0;
            self.rel_acc_y = 0.0;

            if let Some(a) = self.aim_cfg {
                self.mx = a.center_x;
                self.my = a.center_y;
                if a.continuous {
                    self.aim_active = true;
                    if !a.relative {
                        self.down(a.slot, self.mx, self.my);
                    }
                }
            }
        } else {
            self.release_mouse_inputs();
        }
    }

    fn out_touch(&mut self, events: &[(u16, u16, i32)]) {
        self.touch.send(events);
    }

    fn out_touch_critical(&mut self, events: &[(u16, u16, i32)]) {
        self.touch.send_critical(events);
    }

    fn out_pointer(&mut self, events: &[(u16, u16, i32)]) {
        self.pointer.send(events);
    }

    fn xy(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x.clamp(0.0, 1.0) * (self.cfg.display.width - 1) as f32).round() as i32,
            (y.clamp(0.0, 1.0) * (self.cfg.display.height - 1) as f32).round() as i32,
        )
    }

    fn down(&mut self, slot: u8, x: f32, y: f32) {
        let i = slot as usize;
        if i >= MAX_TOUCH_SLOTS || self.slots[i].down {
            return;
        }

        let first = !self.any_down();
        let (x, y) = self.xy(x, y);
        let id = self.next_tracking_id;
        self.next_tracking_id = if id >= i32::MAX - 1 { 1 } else { id + 1 };
        self.slots[i].down = true;

        let mut events = vec![
            (ABS, SLOT, slot as i32),
            (ABS, ID, id),
            (ABS, X, x),
            (ABS, Y, y),
            (ABS, MAJOR, self.touch_cfg.1),
            (ABS, MINOR, self.touch_cfg.2),
            (ABS, PRESS, self.touch_cfg.0),
        ];
        if first {
            events.push((KEY, BTN_TOUCH, 1));
        }
        events.push((SYN, 0, 0));
        self.out_touch_critical(&events);
    }

    fn mv(&mut self, slot: u8, x: f32, y: f32) {
        let i = slot as usize;
        if i >= MAX_TOUCH_SLOTS || !self.slots[i].down {
            return;
        }

        let (x, y) = self.xy(x, y);
        self.out_touch(&[
            (ABS, SLOT, slot as i32),
            (ABS, X, x),
            (ABS, Y, y),
            (ABS, PRESS, self.touch_cfg.0),
            (SYN, 0, 0),
        ]);
    }

    fn any_down(&self) -> bool {
        self.slots.iter().any(|slot| slot.down)
    }

    fn up(&mut self, slot: u8) {
        let i = slot as usize;
        if i >= MAX_TOUCH_SLOTS || !self.slots[i].down {
            return;
        }

        self.slots[i].down = false;
        let last = !self.any_down();

        let mut events = vec![
            (ABS, SLOT, slot as i32),
            (ABS, ID, -1),
            (ABS, PRESS, 0),
        ];
        if last {
            events.push((KEY, BTN_TOUCH, 0));
        }
        events.push((SYN, 0, 0));
        self.out_touch_critical(&events);
    }

    pub fn key(&mut self, code: u16, value: i32) {
        let index = code as usize;
        if index >= MAX_INPUT_CODE {
            return;
        }

        self.keys[index] = value != 0;
        let Some(action) = self.key_actions[index] else {
            return;
        };

        match action {
            KeyAction::Joystick => self.joy(),
            KeyAction::Tap { slot, x, y } => {
                if value == 1 {
                    self.down(slot, x, y);
                    self.up(slot);
                }
            }
            KeyAction::Hold { slot, x, y } => {
                if value == 1 {
                    self.down(slot, x, y);
                } else if value == 0 {
                    self.up(slot);
                }
            }
        }
    }

    fn pressed(&self, code: u16) -> bool {
        self.keys.get(code as usize).copied().unwrap_or(false)
    }

    fn joy(&mut self) {
        let Some(j) = self.joystick else { return };

        let mut dx: f32 = 0.0;
        let mut dy: f32 = 0.0;

        if self.pressed(j.left) {
            dx -= 1.0;
        }
        if self.pressed(j.right) {
            dx += 1.0;
        }
        if self.pressed(j.up) {
            dy -= 1.0;
        }
        if self.pressed(j.down) {
            dy += 1.0;
        }

        let length = (dx * dx + dy * dy).sqrt();
        if j.normalize_diagonal && length > 1.0 {
            dx /= length;
            dy /= length;
        }

        let x = j.center_x + dx * j.radius;
        let y = j.center_y + dy * j.radius;

        if length == 0.0 {
            self.up(j.slot);
        } else if self.slots[j.slot as usize].down {
            self.mv(j.slot, x, y);
        } else {
            self.down(j.slot, x, y);
        }
    }

    pub fn button(&mut self, code: u16, value: i32) {
        let Some(action) = self.mouse_actions.get(code as usize).copied().flatten() else {
            return;
        };

        match action {
            MouseAction::Aim => {
                let Some(a) = self.aim_cfg else { return };
                if a.continuous {
                    return;
                }

                if value == 1 && !self.aim_active {
                    self.aim_active = true;
                    self.mx = a.center_x;
                    self.my = a.center_y;
                    if !a.relative {
                        self.down(a.slot, self.mx, self.my);
                    }
                } else if value == 0 && self.aim_active {
                    self.aim_active = false;
                    if !a.relative {
                        self.up(a.slot);
                    }
                }
            }
            MouseAction::Tap { slot, x, y } => {
                if value == 1 {
                    self.down(slot, x, y);
                    self.up(slot);
                }
            }
            MouseAction::Hold { slot, x, y } => {
                if value == 1 {
                    self.down(slot, x, y);
                } else if value == 0 {
                    self.up(slot);
                }
            }
        }
    }

    pub fn mouse(&mut self, dx: i32, dy: i32) {
        if !self.mouse_locked {
            return;
        }

        let Some(a) = self.aim_cfg else { return };
        if !a.relative && !a.continuous && !self.aim_active {
            return;
        }

        if a.relative {
            if dx == 0 && dy == 0 {
                return;
            }

            self.rel_acc_x += dx as f32
                * a.sensitivity
                * a.scale_x
                * if a.invert_x { -1.0 } else { 1.0 };
            self.rel_acc_y += dy as f32
                * a.sensitivity
                * a.scale_y
                * if a.invert_y { -1.0 } else { 1.0 };

            let sx = self.rel_acc_x.trunc() as i32;
            let sy = self.rel_acc_y.trunc() as i32;
            self.rel_acc_x -= sx as f32;
            self.rel_acc_y -= sy as f32;

            if sx == 0 && sy == 0 {
                return;
            }

            let mut events = Vec::with_capacity(3);
            if sx != 0 {
                events.push((REL, REL_X, sx));
            }
            if sy != 0 {
                events.push((REL, REL_Y, sy));
            }
            events.push((SYN, 0, 0));
            self.out_pointer(&events);
            return;
        }

        self.mx += dx as f32
            * a.sensitivity
            * a.scale_x
            * if a.invert_x { -1.0 } else { 1.0 }
            / self.cfg.display.width.max(1) as f32;

        self.my += dy as f32
            * a.sensitivity
            * a.scale_y
            * if a.invert_y { -1.0 } else { 1.0 }
            / self.cfg.display.height.max(1) as f32;

        let margin = a.edge_margin;
        let hit_edge = self.mx <= margin
            || self.mx >= 1.0 - margin
            || self.my <= margin
            || self.my >= 1.0 - margin;

        if hit_edge {
            self.up(a.slot);
            self.mx = a.center_x;
            self.my = a.center_y;
            self.down(a.slot, self.mx, self.my);
        } else {
            self.mv(a.slot, self.mx, self.my);
        }
    }

    fn release_mouse_inputs(&mut self) {
        self.aim_active = false;

        if let Some(a) = self.aim_cfg {
            if !a.relative {
                self.up(a.slot);
            }
        }

        for slot in 0..MAX_TOUCH_SLOTS {
            if self.mouse_hold_slots[slot] {
                self.up(slot as u8);
            }
        }

        self.rel_acc_x = 0.0;
        self.rel_acc_y = 0.0;
        self.mx = self.aim_cfg.map(|a| a.center_x).unwrap_or(0.5);
        self.my = self.aim_cfg.map(|a| a.center_y).unwrap_or(0.5);
    }

    pub fn reset_keyboard_state(&mut self) {
        self.keys.fill(false);

        if let Some(j) = self.joystick {
            self.up(j.slot);
        }

        let slots = self.cfg.holds.iter().map(|x| x.slot).collect::<Vec<_>>();
        for slot in slots {
            self.up(slot);
        }
    }

    pub fn reset_mouse_state(&mut self) {
        self.release_mouse_inputs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Aim, Config, Devices, Display, Performance, TouchSettings};

    fn config() -> Config {
        Config {
            display: Display { width: 1920, height: 1080 },
            devices: Devices { keyboard: None, mouse: None },
            joystick: None,
            aim: Some(Aim {
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
            }),
            taps: vec![],
            holds: vec![],
            mouse_taps: vec![],
            mouse_holds: vec![],
            performance: Performance::default(),
            touch: TouchSettings::default(),
        }
    }

    #[test]
    fn mapper_accepts_relative_mouse_aim() {
        let mapper = Mapper::new(config()).unwrap();
        assert!(!mapper.is_mouse_locked());
    }

    #[test]
    fn continuous_aim_is_safe_at_start() {
        let mut cfg = config();
        cfg.aim.as_mut().unwrap().button = "ALWAYS".into();
        let mapper = Mapper::new(cfg).unwrap();
        assert!(!mapper.is_mouse_locked());
    }
}
