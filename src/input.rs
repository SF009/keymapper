use crate::{control, touch::Mapper};
use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode};
use std::{
    error::Error,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub enum InputKind {
    Keyboard,
    Mouse,
}

const LOCK_NONE: u8 = 0;
const LOCK_MANUAL: u8 = 1;
const LOCK_AIM: u8 = 2;

pub struct RuntimeControl {
    pub mouse_locked: AtomicBool,
    lock_owner: AtomicU8,
    mouse_event: OwnedFd,
    keyboard_event: OwnedFd,
}

impl RuntimeControl {
    pub fn new(locked: bool) -> io::Result<Arc<Self>> {
        let mouse_fd = create_event_fd()?;
        let keyboard_fd = create_event_fd()?;

        Ok(Arc::new(Self {
            mouse_locked: AtomicBool::new(locked),
            lock_owner: AtomicU8::new(if locked { LOCK_MANUAL } else { LOCK_NONE }),
            mouse_event: mouse_fd,
            keyboard_event: keyboard_fd,
        }))
    }

    pub fn set_locked(&self, locked: bool, allow_lock: bool) -> bool {
        if locked && !allow_lock {
            return false;
        }

        self.lock_owner.store(
            if locked { LOCK_MANUAL } else { LOCK_NONE },
            Ordering::Release,
        );
        self.mouse_locked.store(locked, Ordering::Release);
        self.notify_all();
        true
    }

    pub fn force_unlock(&self) {
        self.lock_owner.store(LOCK_NONE, Ordering::Release);
        self.mouse_locked.store(false, Ordering::Release);
        self.notify_all();
    }

    pub fn toggle(&self, allow_lock: bool) -> Option<bool> {
        if self.mouse_locked.load(Ordering::Acquire) {
            self.set_locked(false, allow_lock);
            Some(false)
        } else if allow_lock {
            self.set_locked(true, true);
            Some(true)
        } else {
            None
        }
    }

    pub fn request_aim_lock(&self, allow_lock: bool) -> bool {
        if !allow_lock {
            return false;
        }

        if self
            .lock_owner
            .compare_exchange(
                LOCK_NONE,
                LOCK_AIM,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.mouse_locked.store(true, Ordering::Release);
            self.notify_all();
            true
        } else {
            false
        }
    }

    pub fn release_aim_lock(&self) -> bool {
        if self
            .lock_owner
            .compare_exchange(
                LOCK_AIM,
                LOCK_NONE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.mouse_locked.store(false, Ordering::Release);
            self.notify_all();
            true
        } else {
            false
        }
    }

    pub fn owner_name(&self) -> &'static str {
        match self.lock_owner.load(Ordering::Acquire) {
            LOCK_MANUAL => "manual",
            LOCK_AIM => "aim",
            _ => "none",
        }
    }

    pub fn notify_mouse(&self) {
        write_eventfd(&self.mouse_event);
    }

    pub fn notify_keyboard(&self) {
        write_eventfd(&self.keyboard_event);
    }

    pub fn notify_all(&self) {
        self.notify_mouse();
        self.notify_keyboard();
    }

    pub fn drain_mouse_notifications(&self) {
        drain_eventfd(&self.mouse_event);
    }

    pub fn drain_keyboard_notifications(&self) {
        drain_eventfd(&self.keyboard_event);
    }

    pub fn mouse_event_fd(&self) -> i32 {
        self.mouse_event.as_raw_fd()
    }

    pub fn keyboard_event_fd(&self) -> i32 {
        self.keyboard_event.as_raw_fd()
    }
}

fn create_event_fd() -> io::Result<OwnedFd> {
    let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn write_eventfd(fd: &OwnedFd) {
    let value: libc::eventfd_t = 1;
    let rc = unsafe {
        libc::write(
            fd.as_raw_fd(),
            (&value as *const libc::eventfd_t).cast::<libc::c_void>(),
            std::mem::size_of::<libc::eventfd_t>(),
        )
    };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::WouldBlock {
            eprintln!("waydroid-keymapper: event notification failed: {err}");
        }
    }
}

fn drain_eventfd(fd: &OwnedFd) {
    let mut value: libc::eventfd_t = 0;
    let _ = unsafe {
        libc::read(
            fd.as_raw_fd(),
            (&mut value as *mut libc::eventfd_t).cast::<libc::c_void>(),
            std::mem::size_of::<libc::eventfd_t>(),
        )
    };
}

fn best_effort_realtime(enabled: bool, priority: i32, thread_name: &str) {
    if !enabled {
        return;
    }

    let mut param = libc::sched_param {
        sched_priority: priority,
    };
    let rc = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &mut param) };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EPERM) {
            eprintln!(
                "waydroid-keymapper: realtime scheduling for {thread_name} unavailable: {err}"
            );
        }
    }
}

fn keyboard_loop(path: &str, mapper: &Arc<Mutex<Mapper>>, control: &Arc<RuntimeControl>) {
    let mut d = match Device::open(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("waydroid-keymapper: open keyboard {path}: {e}");
            return;
        }
    };

    let _ = d.set_nonblocking(true);

    let (grab, realtime, toggle, priority) = match mapper.lock() {
        Ok(m) => (
            m.config().performance.grab,
            m.config().performance.realtime,
            key_code(&m.config().performance.mouse_toggle_key).ok(),
            m.config().performance.realtime_priority,
        ),
        Err(_) => return,
    };
    best_effort_realtime(realtime, priority, "wd-keyboard");

    let mut locked = control.mouse_locked.load(Ordering::Acquire);
    if grab && locked {
        if let Err(e) = d.grab() {
            eprintln!("waydroid-keymapper: initial keyboard grab failed for {path}: {e}");
            control.force_unlock();
            locked = false;
        }
    }

    let mut ctrl_down = false;
    let mut alt_down = false;

    loop {
        if control::shutdown_requested() {
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_keyboard_state();
            }
            break;
        }

        let mut fds = [
            libc::pollfd {
                fd: control.keyboard_event_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: d.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let rc = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            eprintln!("waydroid-keymapper: keyboard poll failed: {err}");
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_keyboard_state();
            }
            return;
        }

        if fds[0].revents & (libc::POLLIN | libc::POLLERR) != 0 {
            control.drain_keyboard_notifications();
            let desired = control.mouse_locked.load(Ordering::Acquire);

            if desired != locked {
                if grab {
                    let result = if desired { d.grab() } else { d.ungrab() };
                    if let Err(e) = result {
                        eprintln!("waydroid-keymapper: keyboard grab transition failed: {e}");
                        control.force_unlock();
                        locked = false;
                    } else {
                        locked = desired;
                    }
                } else {
                    locked = desired;
                }

                if !locked {
                    if let Ok(mut m) = mapper.lock() {
                        m.reset_keyboard_state();
                    }
                }
            }
        }

        if fds[1].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_keyboard_state();
            }
            return;
        }

        if fds[1].revents & libc::POLLIN == 0 {
            continue;
        }

        let events = match d.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(e) => {
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    continue;
                }
                eprintln!("waydroid-keymapper: keyboard read failed: {e}");
                let _ = d.ungrab();
                if let Ok(mut m) = mapper.lock() {
                    m.reset_keyboard_state();
                }
                return;
            }
        };

        let mut m = match mapper.lock() {
            Ok(x) => x,
            Err(_) => return,
        };

        for e in events {
            let EventSummary::Key(_, c, v) = e.destructure() else {
                continue;
            };

            let code = c.0;
            if code == KeyCode::KEY_LEFTCTRL.0 || code == KeyCode::KEY_RIGHTCTRL.0 {
                ctrl_down = v != 0;
            }
            if code == KeyCode::KEY_LEFTALT.0 || code == KeyCode::KEY_RIGHTALT.0 {
                alt_down = v != 0;
            }

            if code == KeyCode::KEY_F12.0 && v == 1 && ctrl_down && alt_down {
                control.force_unlock();
                locked = false;
                m.set_mouse_lock(false);
                m.reset_keyboard_state();
                let _ = d.ungrab();
                continue;
            }

            if Some(code) == toggle && v == 1 {
                let can_grab = m.config().performance.grab;
                if let Some(next) = control.toggle(can_grab) {
                    locked = next;
                    if !locked {
                        m.reset_keyboard_state();
                    }
                }
                continue;
            }

            if locked {
                m.key(code, v);
            }
        }
    }
}

fn flush_pending_mouse_events(d: &mut Device) {
    loop {
        match d.fetch_events() {
            Ok(events) => {
                let _ = events.count();
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn apply_mouse_lock(
    d: &mut Device,
    desired: bool,
    grab: bool,
    discard_queued: bool,
    mapper: &Arc<Mutex<Mapper>>,
    control: &Arc<RuntimeControl>,
) -> bool {
    if desired {
        if !grab {
            control.force_unlock();
            if let Ok(mut m) = mapper.lock() {
                m.set_mouse_lock(false);
            }
            return false;
        }

        match d.grab() {
            Ok(()) => {
                if discard_queued {
                    flush_pending_mouse_events(d);
                }
                if let Ok(mut m) = mapper.lock() {
                    m.set_mouse_lock(true);
                }
                true
            }
            Err(e) => {
                eprintln!("waydroid-keymapper: mouse grab failed: {e}");
                control.force_unlock();
                if let Ok(mut m) = mapper.lock() {
                    m.set_mouse_lock(false);
                }
                false
            }
        }
    } else {
        let _ = d.ungrab();
        if let Ok(mut m) = mapper.lock() {
            m.set_mouse_lock(false);
        }
        true
    }
}

fn mouse_loop(path: &str, mapper: &Arc<Mutex<Mapper>>, control: &Arc<RuntimeControl>) {
    let mut d = match Device::open(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("waydroid-keymapper: open mouse {path}: {e}");
            return;
        }
    };

    if let Err(e) = d.set_nonblocking(true) {
        eprintln!("waydroid-keymapper: nonblocking mouse input failed for {path}: {e}");
        return;
    }

    let (grab, realtime, auto_lock_on_aim, aim_button, priority) = match mapper.lock() {
        Ok(m) => (
            m.config().performance.grab,
            m.config().performance.realtime,
            m.config().performance.auto_lock_on_aim,
            m.config()
                .aim
                .as_ref()
                .filter(|a| !a.is_continuous())
                .and_then(|a| button_code(&a.button).ok()),
            m.config().performance.realtime_priority,
        ),
        Err(_) => return,
    };
    best_effort_realtime(realtime, priority, "wd-mouse");

    let mut locked = false;
    if control.mouse_locked.load(Ordering::Acquire) {
        locked = apply_mouse_lock(&mut d, true, grab, true, mapper, control);
    } else if let Ok(mut m) = mapper.lock() {
        m.set_mouse_lock(false);
    }

    loop {
        if control::shutdown_requested() {
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_mouse_state();
            }
            control.force_unlock();
            break;
        }

        let mut fds = [
            libc::pollfd {
                fd: control.mouse_event_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: d.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        // We keep the physical mouse fd in the poll set even while unlocked:
        // there is no polling timer or busy-spin, and it is required to detect
        // a configured Aim button for optional Helper-style auto-lock.
        let rc = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            eprintln!("waydroid-keymapper: mouse poll failed: {err}");
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_mouse_state();
            }
            control.force_unlock();
            return;
        }

        if fds[0].revents & (libc::POLLIN | libc::POLLERR) != 0 {
            control.drain_mouse_notifications();
            let desired = control.mouse_locked.load(Ordering::Acquire);
            if desired != locked {
                locked = apply_mouse_lock(&mut d, desired, grab, true, mapper, control);
            }
        }

        if fds[1].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_mouse_state();
            }
            control.force_unlock();
            return;
        }

        if fds[1].revents & libc::POLLIN == 0 {
            continue;
        }

        let events = match d.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(e) => {
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) {
                    continue;
                }
                eprintln!("waydroid-keymapper: mouse read failed: {e}");
                let _ = d.ungrab();
                if let Ok(mut m) = mapper.lock() {
                    m.reset_mouse_state();
                }
                control.force_unlock();
                return;
            }
        };

        let mut dx = 0i32;
        let mut dy = 0i32;

        for e in events {
            match e.destructure() {
                EventSummary::RelativeAxis(_, c, v) => {
                    if locked {
                        if c == RelativeAxisCode::REL_X {
                            dx = dx.saturating_add(v);
                        } else if c == RelativeAxisCode::REL_Y {
                            dy = dy.saturating_add(v);
                        }
                    }
                }
                EventSummary::Key(_, c, v) => {
                    if locked && (dx != 0 || dy != 0) {
                        if let Ok(mut m) = mapper.lock() {
                            m.mouse(dx, dy);
                        }
                        dx = 0;
                        dy = 0;
                    }

                    let code = c.0;

                    // Detect the configured physical Aim button before applying
                    // ordinary button mappings. Auto-lock is only entered from
                    // an actual button press; "ALWAYS" never locks at startup.
                    if Some(code) == aim_button && v == 1 && auto_lock_on_aim && !locked {
                        if control.request_aim_lock(grab) {
                            locked = apply_mouse_lock(&mut d, true, grab, true, mapper, control);
                        }
                    }

                    if locked {
                        if let Ok(mut m) = mapper.lock() {
                            m.button(code, v);
                        }
                    } else if Some(code) == aim_button {
                        // The press/release is not stolen before lock, but the
                        // mapper still sees the event so an Aim contact can be
                        // initialized when auto-lock is unavailable.
                        if let Ok(mut m) = mapper.lock() {
                            m.button(code, v);
                        }
                    }

                    if Some(code) == aim_button && v == 0 && auto_lock_on_aim {
                        if control.release_aim_lock() {
                            locked = apply_mouse_lock(&mut d, false, grab, false, mapper, control);
                        } else {
                            locked = control.mouse_locked.load(Ordering::Acquire);
                        }
                    }
                }
                _ => {}
            }
        }

        if locked && (dx != 0 || dy != 0) {
            if let Ok(mut m) = mapper.lock() {
                m.mouse(dx, dy);
            }
        }
    }
}

pub fn spawn_input(
    path: String,
    kind: InputKind,
    mapper: Arc<Mutex<Mapper>>,
    control: Arc<RuntimeControl>,
) -> Result<(), Box<dyn Error>> {
    thread::Builder::new()
        .name(
            match kind {
                InputKind::Keyboard => "wd-keyboard",
                InputKind::Mouse => "wd-mouse",
            }
            .to_string(),
        )
        .spawn(move || {
            loop {
                if control::shutdown_requested() {
                    break;
                }

                match kind {
                    InputKind::Keyboard => keyboard_loop(&path, &mapper, &control),
                    InputKind::Mouse => mouse_loop(&path, &mapper, &control),
                }

                if !control::shutdown_requested() {
                    thread::sleep(Duration::from_millis(250));
                }
            }
        })?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub enum KeyAction {
    Joystick,
    Tap { slot: u8, x: f32, y: f32 },
    Hold { slot: u8, x: f32, y: f32 },
}

#[derive(Clone, Copy, Debug)]
pub enum MouseAction {
    Aim,
    Tap { slot: u8, x: f32, y: f32 },
    Hold { slot: u8, x: f32, y: f32 },
}

#[derive(Clone, Debug)]
pub struct InputDeviceInfo {
    pub path: String,
    pub name: String,
    pub is_keyboard: bool,
    pub is_mouse: bool,
    pub score: i32,
}

fn inspect_device(path: &std::path::Path) -> Option<InputDeviceInfo> {
    let d = Device::open(path).ok()?;
    let keys = d.supported_keys();
    let rel = d.supported_relative_axes();

    let has_mouse_buttons = keys
        .as_ref()
        .map(|k| {
            k.contains(KeyCode::BTN_LEFT)
                || k.contains(KeyCode::BTN_RIGHT)
                || k.contains(KeyCode::BTN_MIDDLE)
        })
        .unwrap_or(false);

    let has_relative = rel
        .as_ref()
        .map(|a| {
            a.contains(RelativeAxisCode::REL_X) || a.contains(RelativeAxisCode::REL_Y)
        })
        .unwrap_or(false);

    let is_mouse = has_mouse_buttons && has_relative;

    let is_keyboard = keys
        .as_ref()
        .map(|k| {
            k.contains(KeyCode::KEY_A)
                || k.contains(KeyCode::KEY_W)
                || k.contains(KeyCode::KEY_ENTER)
                || k.contains(KeyCode::KEY_ESC)
        })
        .unwrap_or(false);

    if !is_mouse && !is_keyboard {
        return None;
    }

    let name = d.name().unwrap_or("input").to_string();
    let lower = name.to_ascii_lowercase();
    let mut score = 0;

    if !lower.contains("virtual") {
        score += 40;
    }
    if !lower.contains("ydotool") {
        score += 40;
    }
    if !lower.contains("keyd") {
        score += 40;
    }

    if is_mouse {
        if lower.contains("usb") {
            score += 25;
        }
        if lower.contains("optical") || lower.contains("gaming") {
            score += 20;
        }
        if lower.contains("touchpad") {
            score -= 35;
        }
    }

    if is_keyboard {
        if lower.contains("at translated") {
            score += 25;
        }
        if lower.contains("keyboard") {
            score += 20;
        }
    }

    Some(InputDeviceInfo {
        path: path.to_string_lossy().to_string(),
        name,
        is_keyboard,
        is_mouse,
        score,
    })
}

pub fn list_input_devices() -> Vec<InputDeviceInfo> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::<std::path::PathBuf>::new();

    if let Ok(dir) = std::fs::read_dir("/dev/input/by-id") {
        let mut stable = dir.flatten().map(|e| e.path()).collect::<Vec<_>>();
        stable.sort();

        for p in stable {
            if !p.is_symlink() {
                continue;
            }
            if let Some(info) = inspect_device(&p) {
                let canonical = p.canonicalize().unwrap_or(p.clone());
                if seen.insert(canonical) {
                    out.push(info);
                }
            }
        }
    }

    for (path, _) in evdev::enumerate() {
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if seen.contains(&canonical) {
            continue;
        }
        if let Some(info) = inspect_device(&path) {
            seen.insert(canonical);
            out.push(info);
        }
    }

    out.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    out
}

pub fn key_code(s: &str) -> Result<u16, Box<dyn Error>> {
    let v = match s.trim().to_ascii_uppercase().as_str() {
        "A" => KeyCode::KEY_A.0,
        "B" => KeyCode::KEY_B.0,
        "C" => KeyCode::KEY_C.0,
        "D" => KeyCode::KEY_D.0,
        "E" => KeyCode::KEY_E.0,
        "F" => KeyCode::KEY_F.0,
        "G" => KeyCode::KEY_G.0,
        "H" => KeyCode::KEY_H.0,
        "I" => KeyCode::KEY_I.0,
        "J" => KeyCode::KEY_J.0,
        "K" => KeyCode::KEY_K.0,
        "L" => KeyCode::KEY_L.0,
        "M" => KeyCode::KEY_M.0,
        "N" => KeyCode::KEY_N.0,
        "O" => KeyCode::KEY_O.0,
        "P" => KeyCode::KEY_P.0,
        "Q" => KeyCode::KEY_Q.0,
        "R" => KeyCode::KEY_R.0,
        "S" => KeyCode::KEY_S.0,
        "T" => KeyCode::KEY_T.0,
        "U" => KeyCode::KEY_U.0,
        "V" => KeyCode::KEY_V.0,
        "W" => KeyCode::KEY_W.0,
        "X" => KeyCode::KEY_X.0,
        "Y" => KeyCode::KEY_Y.0,
        "Z" => KeyCode::KEY_Z.0,
        "0" => KeyCode::KEY_0.0,
        "1" => KeyCode::KEY_1.0,
        "2" => KeyCode::KEY_2.0,
        "3" => KeyCode::KEY_3.0,
        "4" => KeyCode::KEY_4.0,
        "5" => KeyCode::KEY_5.0,
        "6" => KeyCode::KEY_6.0,
        "7" => KeyCode::KEY_7.0,
        "8" => KeyCode::KEY_8.0,
        "9" => KeyCode::KEY_9.0,
        "SPACE" | "SPACEBAR" => KeyCode::KEY_SPACE.0,
        "ENTER" | "RETURN" => KeyCode::KEY_ENTER.0,
        "ESC" | "ESCAPE" => KeyCode::KEY_ESC.0,
        "SHIFT" | "LEFTSHIFT" | "LSHIFT" | "SHIFT_L" => KeyCode::KEY_LEFTSHIFT.0,
        "RIGHTSHIFT" | "RSHIFT" | "SHIFT_R" => KeyCode::KEY_RIGHTSHIFT.0,
        "CTRL" | "CONTROL" | "LEFTCTRL" | "LCTRL" | "CONTROL_L" => KeyCode::KEY_LEFTCTRL.0,
        "RIGHTCTRL" | "RCTRL" | "CONTROL_R" => KeyCode::KEY_RIGHTCTRL.0,
        "ALT" | "LEFTALT" | "LALT" | "ALT_L" => KeyCode::KEY_LEFTALT.0,
        "RIGHTALT" | "RALT" | "ALT_R" | "ALTGR" => KeyCode::KEY_RIGHTALT.0,
        "TAB" => KeyCode::KEY_TAB.0,
        "BACKSPACE" => KeyCode::KEY_BACKSPACE.0,
        "CAPSLOCK" | "CAPS" => KeyCode::KEY_CAPSLOCK.0,
        "F1" => KeyCode::KEY_F1.0,
        "F2" => KeyCode::KEY_F2.0,
        "F3" => KeyCode::KEY_F3.0,
        "F4" => KeyCode::KEY_F4.0,
        "F5" => KeyCode::KEY_F5.0,
        "F6" => KeyCode::KEY_F6.0,
        "F7" => KeyCode::KEY_F7.0,
        "F8" => KeyCode::KEY_F8.0,
        "F9" => KeyCode::KEY_F9.0,
        "F10" => KeyCode::KEY_F10.0,
        "F11" => KeyCode::KEY_F11.0,
        "F12" => KeyCode::KEY_F12.0,
        "HOME" => KeyCode::KEY_HOME.0,
        "END" => KeyCode::KEY_END.0,
        "UP" => KeyCode::KEY_UP.0,
        "DOWN" => KeyCode::KEY_DOWN.0,
        "LEFT" => KeyCode::KEY_LEFT.0,
        "RIGHT" => KeyCode::KEY_RIGHT.0,
        "PAGEUP" | "PAGE_UP" | "PGUP" => KeyCode::KEY_PAGEUP.0,
        "PAGEDOWN" | "PAGE_DOWN" | "PGDN" => KeyCode::KEY_PAGEDOWN.0,
        "INSERT" | "INS" => KeyCode::KEY_INSERT.0,
        "DELETE" | "DEL" => KeyCode::KEY_DELETE.0,
        "NUMLOCK" => KeyCode::KEY_NUMLOCK.0,
        "SCROLLLOCK" => KeyCode::KEY_SCROLLLOCK.0,
        "KP0" => KeyCode::KEY_KP0.0,
        "KP1" => KeyCode::KEY_KP1.0,
        "KP2" => KeyCode::KEY_KP2.0,
        "KP3" => KeyCode::KEY_KP3.0,
        "KP4" => KeyCode::KEY_KP4.0,
        "KP5" => KeyCode::KEY_KP5.0,
        "KP6" => KeyCode::KEY_KP6.0,
        "KP7" => KeyCode::KEY_KP7.0,
        "KP8" => KeyCode::KEY_KP8.0,
        "KP9" => KeyCode::KEY_KP9.0,
        "LEFTMETA" | "META_L" | "SUPER_L" | "WIN_L" | "WINDOWS" => KeyCode::KEY_LEFTMETA.0,
        "RIGHTMETA" | "META_R" | "SUPER_R" | "WIN_R" => KeyCode::KEY_RIGHTMETA.0,
        "META" | "SUPER" | "WIN" => KeyCode::KEY_LEFTMETA.0,
        "MENU" | "APPLICATION" => KeyCode::KEY_MENU.0,
        "PRINT" | "PRINTSCREEN" | "SYSRQ" => KeyCode::KEY_SYSRQ.0,
        "PAUSE" => KeyCode::KEY_PAUSE.0,
        "KPENTER" => KeyCode::KEY_KPENTER.0,
        "KPSLASH" => KeyCode::KEY_KPSLASH.0,
        "KPASTERISK" | "KPSTAR" => KeyCode::KEY_KPASTERISK.0,
        "KPMINUS" => KeyCode::KEY_KPMINUS.0,
        "KPPLUS" => KeyCode::KEY_KPPLUS.0,
        "KPDOT" => KeyCode::KEY_KPDOT.0,
        "KPEQUAL" => KeyCode::KEY_KPEQUAL.0,
        "GRAVE" | "BACKTICK" | "TILDE" => KeyCode::KEY_GRAVE.0,
        "MINUS" | "DASH" => KeyCode::KEY_MINUS.0,
        "EQUAL" | "EQUALS" => KeyCode::KEY_EQUAL.0,
        "LEFTBRACE" | "LBRACKET" | "BRACKETLEFT" => KeyCode::KEY_LEFTBRACE.0,
        "RIGHTBRACE" | "RBRACKET" | "BRACKETRIGHT" => KeyCode::KEY_RIGHTBRACE.0,
        "BACKSLASH" => KeyCode::KEY_BACKSLASH.0,
        "SEMICOLON" => KeyCode::KEY_SEMICOLON.0,
        "APOSTROPHE" | "QUOTE" => KeyCode::KEY_APOSTROPHE.0,
        "COMMA" => KeyCode::KEY_COMMA.0,
        "DOT" | "PERIOD" => KeyCode::KEY_DOT.0,
        "SLASH" => KeyCode::KEY_SLASH.0,
        _ => return Err(format!("unknown key {s}").into()),
    };
    Ok(v)
}

pub fn button_code(s: &str) -> Result<u16, Box<dyn Error>> {
    match s.trim().to_ascii_uppercase().as_str() {
        "MOUSE_LEFT" | "LEFT" | "BTN_LEFT" | "MOUSE1" | "LCLICK" => Ok(KeyCode::BTN_LEFT.0),
        "MOUSE_RIGHT" | "RIGHT" | "BTN_RIGHT" | "MOUSE2" | "RCLICK" => Ok(KeyCode::BTN_RIGHT.0),
        "MOUSE_MIDDLE" | "MIDDLE" | "BTN_MIDDLE" | "MOUSE3" | "MCLICK" => Ok(KeyCode::BTN_MIDDLE.0),
        "MOUSE_SIDE" | "SIDE" | "MOUSE_4" | "MOUSE4" | "BTN_SIDE" => Ok(KeyCode::BTN_SIDE.0),
        "MOUSE_EXTRA" | "EXTRA" | "MOUSE_5" | "MOUSE5" | "BTN_EXTRA" => Ok(KeyCode::BTN_EXTRA.0),
        "MOUSE_FORWARD" | "FORWARD" | "BTN_FORWARD" => Ok(KeyCode::BTN_FORWARD.0),
        "MOUSE_BACK" | "BACK" | "BTN_BACK" => Ok(KeyCode::BTN_BACK.0),
        "MOUSE_TASK" | "TASK" | "BTN_TASK" => Ok(KeyCode::BTN_TASK.0),
        _ => Err(format!("unknown mouse button {s}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_starts_unlocked_by_default() {
        let control = RuntimeControl::new(false).unwrap();
        assert!(!control.mouse_locked.load(Ordering::Acquire));
        assert_eq!(control.owner_name(), "none");
    }

    #[test]
    fn manual_toggle_obeys_permission() {
        let control = RuntimeControl::new(false).unwrap();
        assert_eq!(control.toggle(false), None);
        assert_eq!(control.toggle(true), Some(true));
        assert!(control.mouse_locked.load(Ordering::Acquire));
        assert_eq!(control.owner_name(), "manual");
        assert_eq!(control.toggle(true), Some(false));
        assert!(!control.mouse_locked.load(Ordering::Acquire));
    }

    #[test]
    fn aim_lock_is_exclusive() {
        let control = RuntimeControl::new(false).unwrap();
        assert!(control.request_aim_lock(true));
        assert_eq!(control.owner_name(), "aim");
        assert!(!control.request_aim_lock(true));
        assert!(control.release_aim_lock());
        assert_eq!(control.owner_name(), "none");
    }

    #[test]
    fn manual_owner_blocks_aim_owner() {
        let control = RuntimeControl::new(false).unwrap();
        assert!(control.set_locked(true, true));
        assert_eq!(control.owner_name(), "manual");
        assert!(!control.request_aim_lock(true));
        assert_eq!(control.owner_name(), "manual");
    }

    #[test]
    fn force_unlock_clears_owner() {
        let control = RuntimeControl::new(true).unwrap();
        control.force_unlock();
        assert!(!control.mouse_locked.load(Ordering::Acquire));
        assert_eq!(control.owner_name(), "none");
    }

    #[test]
    fn always_button_is_not_a_physical_button() {
        assert!(crate::config::Aim {
            button: "ALWAYS".into(),
            center_x: 0.5,
            center_y: 0.5,
            sensitivity: 1.0,
            slot: 1,
            invert_x: false,
            invert_y: false,
            scale_x: 1.0,
            scale_y: 1.0,
            edge_margin: 0.12,
            mode: "relative".into(),
        }
        .is_continuous());
    }
}
