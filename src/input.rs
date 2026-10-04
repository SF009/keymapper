use crate::{control, touch::Mapper};
use evdev::{Device, EventSummary, KeyCode, RelativeAxisCode};
use std::{
    error::Error,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

#[derive(Clone, Copy)]
pub enum InputKind {
    Keyboard,
    Mouse,
}

pub struct RuntimeControl {
    pub mouse_locked: AtomicBool,
    mouse_event: OwnedFd,
    keyboard_event: OwnedFd,
}

impl RuntimeControl {
    pub fn new(locked: bool) -> io::Result<Arc<Self>> {
        let mfd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if mfd < 0 {
            return Err(io::Error::last_os_error());
        }
        let kfd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if kfd < 0 {
            unsafe { libc::close(mfd) };
            return Err(io::Error::last_os_error());
        }
        Ok(Arc::new(Self {
            mouse_locked: AtomicBool::new(locked),
            mouse_event: unsafe { OwnedFd::from_raw_fd(mfd) },
            keyboard_event: unsafe { OwnedFd::from_raw_fd(kfd) },
        }))
    }

    pub fn set_locked(&self, locked: bool, allow_lock: bool) -> bool {
        if locked && !allow_lock {
            return false;
        }
        self.mouse_locked.store(locked, Ordering::Release);
        self.notify_all();
        true
    }

    pub fn force_unlock(&self) {
        self.mouse_locked.store(false, Ordering::Release);
        self.notify_all();
    }

    pub fn toggle(&self, allow_lock: bool) -> Option<bool> {
        let mut current = self.mouse_locked.load(Ordering::Acquire);
        loop {
            let next = !current;
            if next && !allow_lock {
                return None;
            }
            match self
                .mouse_locked
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    self.notify_all();
                    return Some(next);
                }
                Err(actual) => current = actual,
            }
        }
    }

    pub fn notify_all(&self) {
        let value: libc::c_ulonglong = 1;
        unsafe {
            let _ = libc::write(
                self.mouse_event.as_raw_fd(),
                (&value as *const libc::c_ulonglong).cast::<libc::c_void>(),
                std::mem::size_of::<libc::c_ulonglong>(),
            );
            let _ = libc::write(
                self.keyboard_event.as_raw_fd(),
                (&value as *const libc::c_ulonglong).cast::<libc::c_void>(),
                std::mem::size_of::<libc::c_ulonglong>(),
            );
        }
    }

    pub fn drain_mouse_notifications(&self) {
        let mut value: libc::c_ulonglong = 0;
        unsafe {
            let _ = libc::read(
                self.mouse_event.as_raw_fd(),
                (&mut value as *mut libc::c_ulonglong).cast::<libc::c_void>(),
                std::mem::size_of::<libc::c_ulonglong>(),
            );
        }
    }

    pub fn drain_keyboard_notifications(&self) {
        let mut value: libc::c_ulonglong = 0;
        unsafe {
            let _ = libc::read(
                self.keyboard_event.as_raw_fd(),
                (&mut value as *mut libc::c_ulonglong).cast::<libc::c_void>(),
                std::mem::size_of::<libc::c_ulonglong>(),
            );
        }
    }

    pub fn mouse_event_fd(&self) -> i32 {
        self.mouse_event.as_raw_fd()
    }

    pub fn keyboard_event_fd(&self) -> i32 {
        self.keyboard_event.as_raw_fd()
    }
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

    let (grab, realtime, toggle) = match mapper.lock() {
        Ok(m) => (
            m.config().performance.grab,
            m.config().performance.realtime,
            key_code(&m.config().performance.mouse_toggle_key).ok(),
        ),
        Err(_) => return,
    };
    let priority = match mapper.lock() {
        Ok(m) => m.config().performance.realtime_priority,
        Err(_) => 10,
    };
    best_effort_realtime(realtime, priority, "wd-keyboard");

    let mut locked = control.mouse_locked.load(Ordering::Acquire);
    if grab && locked {
        if let Err(e) = d.grab() {
            eprintln!("waydroid-keymapper: initial keyboard grab failed for {path}: {e}");
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
            if control::shutdown_requested() {
                break;
            }
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_keyboard_state();
            }
            return;
        }

        // Handle lock/unlock state notifications
        if fds[0].revents & (libc::POLLIN | libc::POLLERR) != 0 {
            control.drain_keyboard_notifications();
            let desired = control.mouse_locked.load(Ordering::Acquire);
            if desired != locked {
                if grab {
                    if desired {
                        let _ = d.grab();
                    } else {
                        let _ = d.ungrab();
                    }
                }
                locked = desired;
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

        if fds[1].revents & libc::POLLIN != 0 {
            let fetched = d.fetch_events().map(|events| events.collect::<Vec<_>>());
            match fetched {
                Ok(events) => {
                    let mut m = match mapper.lock() {
                        Ok(x) => x,
                        Err(_) => return,
                    };
                    for e in events {
                        if let EventSummary::Key(_, c, v) = e.destructure() {
                            let code = c.0;
                            if code == KeyCode::KEY_LEFTCTRL.0 || code == KeyCode::KEY_RIGHTCTRL.0 {
                                ctrl_down = v != 0;
                            }
                            if code == KeyCode::KEY_LEFTALT.0 || code == KeyCode::KEY_RIGHTALT.0 {
                                alt_down = v != 0;
                            }

                            // Emergency unlock: Ctrl+Alt+F12
                            if code == KeyCode::KEY_F12.0 && v == 1 && ctrl_down && alt_down {
                                control.force_unlock();
                                if grab {
                                    let _ = d.ungrab();
                                }
                                locked = false;
                                m.set_mouse_lock(false);
                                m.reset_keyboard_state();
                                continue;
                            }

                            // Toggle key (e.g. F8)
                            if Some(code) == toggle && v == 1 {
                                let can_grab = m.config().performance.grab;
                                if let Some(next) = control.toggle(can_grab) {
                                    if grab {
                                        if next {
                                            let _ = d.grab();
                                        } else {
                                            let _ = d.ungrab();
                                        }
                                    }
                                    locked = next;
                                    if !locked {
                                        m.reset_keyboard_state();
                                    }
                                }
                                continue;
                            }

                            // When in game (locked), map keys to Waydroid
                            if locked {
                                m.key(code, v);
                            }
                        }
                    }
                }
                Err(e) => {
                    if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) {
                        continue;
                    }
                    let _ = d.ungrab();
                    if let Ok(mut m) = mapper.lock() {
                        m.reset_keyboard_state();
                    }
                    return;
                }
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
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                if e.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
            }
            Err(_) => break,
        }
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

    if d.set_nonblocking(true).is_err() {
        eprintln!("waydroid-keymapper: failed to set nonblocking mouse input for {path}");
        return;
    }

    let (grab, realtime) = match mapper.lock() {
        Ok(m) => (m.config().performance.grab, m.config().performance.realtime),
        Err(_) => return,
    };
    let priority = match mapper.lock() {
        Ok(m) => m.config().performance.realtime_priority,
        Err(_) => 10,
    };
    best_effort_realtime(realtime, priority, "wd-mouse");

    let mut locked = control.mouse_locked.load(Ordering::Acquire);
    if grab && locked {
        if let Err(e) = d.grab() {
            eprintln!("waydroid-keymapper: initial mouse grab failed for {path}: {e}");
            locked = false;
            control.mouse_locked.store(false, Ordering::Release);
        } else {
            flush_pending_mouse_events(&mut d);
        }
    }
    if let Ok(mut m) = mapper.lock() {
        m.set_mouse_lock(locked);
    }

    loop {
        if control::shutdown_requested() {
            let _ = d.ungrab();
            if let Ok(mut m) = mapper.lock() {
                m.reset_mouse_state();
            }
            break;
        }

        // When locked, poll both control and mouse device fd.
        // When unlocked, ONLY poll control fd to eliminate wakeups and 0% CPU consumption!
        let mut fds = [
            libc::pollfd {
                fd: control.mouse_event_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: d.as_raw_fd(),
                events: if locked { libc::POLLIN } else { 0 },
                revents: 0,
            },
        ];
        let nfds = if locked { 2 } else { 1 };

        let rc = unsafe { libc::poll(fds.as_mut_ptr(), nfds, -1) };
        if rc < 0 {
            if control::shutdown_requested() {
                break;
            }
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if let Ok(mut m) = mapper.lock() {
                m.reset_mouse_state();
            }
            return;
        }

        // State notification from control server or toggle key
        if fds[0].revents & (libc::POLLIN | libc::POLLERR) != 0 {
            control.drain_mouse_notifications();
            let desired = control.mouse_locked.load(Ordering::Acquire);
            if desired != locked {
                let ok = if grab {
                    if desired {
                        d.grab().is_ok()
                    } else {
                        d.ungrab().is_ok()
                    }
                } else {
                    true
                };

                if ok {
                    if desired {
                        flush_pending_mouse_events(&mut d);
                    }
                    locked = desired;
                    if let Ok(mut m) = mapper.lock() {
                        m.set_mouse_lock(locked);
                    }
                } else if desired {
                    control.mouse_locked.store(false, Ordering::Release);
                    locked = false;
                    if let Ok(mut m) = mapper.lock() {
                        m.set_mouse_lock(false);
                    }
                }
            }
        }

        if locked {
            if fds[1].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                let _ = d.ungrab();
                if let Ok(mut m) = mapper.lock() {
                    m.reset_mouse_state();
                }
                return;
            }

            if fds[1].revents & libc::POLLIN != 0 {
                let fetched = d.fetch_events().map(|events| events.collect::<Vec<_>>());
                match fetched {
                    Ok(events) => {
                        let mut m = match mapper.lock() {
                            Ok(x) => x,
                            Err(_) => return,
                        };
                        let mut dx = 0i32;
                        let mut dy = 0i32;
                        for e in events {
                            match e.destructure() {
                                EventSummary::RelativeAxis(_, c, v) => {
                                    if c == RelativeAxisCode::REL_X {
                                        dx = dx.saturating_add(v);
                                    } else if c == RelativeAxisCode::REL_Y {
                                        dy = dy.saturating_add(v);
                                    }
                                }
                                EventSummary::Key(_, c, v) => {
                                    if dx != 0 || dy != 0 {
                                        m.mouse(dx, dy);
                                        dx = 0;
                                        dy = 0;
                                    }
                                    m.button(c.0, v);
                                }
                                _ => {}
                            }
                        }
                        if dx != 0 || dy != 0 {
                            m.mouse(dx, dy);
                        }
                    }
                    Err(e) => {
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) {
                            continue;
                        }
                        let _ = d.ungrab();
                        if let Ok(mut m) = mapper.lock() {
                            m.reset_mouse_state();
                        }
                        return;
                    }
                }
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
            .into(),
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
                thread::sleep(std::time::Duration::from_millis(250));
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
    let is_mouse = keys
        .as_ref()
        .map(|k| {
            k.contains(KeyCode::BTN_LEFT)
                || k.contains(KeyCode::BTN_RIGHT)
                || k.contains(KeyCode::BTN_MIDDLE)
        })
        .unwrap_or(false)
        && rel
            .as_ref()
            .map(|a| {
                a.contains(RelativeAxisCode::REL_X) || a.contains(RelativeAxisCode::REL_Y)
            })
            .unwrap_or(false);
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
    let mut out = Vec::<InputDeviceInfo>::new();
    let mut seen = std::collections::HashSet::<std::path::PathBuf>::new();

    if let Ok(dir) = std::fs::read_dir("/dev/input/by-id") {
        let mut stable: Vec<std::path::PathBuf> = dir.flatten().map(|e| e.path()).collect();
        stable.sort();
        for p in stable {
            if !p.is_symlink() {
                continue;
            }
            if let Some(info) = inspect_device(&p) {
                let canonical = p.canonicalize().unwrap_or_else(|_| p.clone());
                if seen.insert(canonical) {
                    out.push(info);
                }
            }
        }
    }

    for (path, _) in evdev::enumerate() {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
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
        "MOUSE_MIDDLE" | "MIDDLE" | "BTN_MIDDLE" | "MOUSE3" | "MCLICK" => {
            Ok(KeyCode::BTN_MIDDLE.0)
        }
        "MOUSE_SIDE" | "SIDE" | "MOUSE_4" | "MOUSE4" | "BTN_SIDE" => Ok(KeyCode::BTN_SIDE.0),
        "MOUSE_EXTRA" | "EXTRA" | "MOUSE_5" | "MOUSE5" | "BTN_EXTRA" => Ok(KeyCode::BTN_EXTRA.0),
        "MOUSE_FORWARD" | "FORWARD" | "BTN_FORWARD" => Ok(KeyCode::BTN_FORWARD.0),
        "MOUSE_BACK" | "BACK" | "BTN_BACK" => Ok(KeyCode::BTN_BACK.0),
        "MOUSE_TASK" | "TASK" | "BTN_TASK" => Ok(KeyCode::BTN_TASK.0),
        _ => Err(format!("unknown button {s}").into()),
    }
}

#[cfg(test)]
mod runtime_control_tests {
    use super::*;

    #[test]
    fn toggle_respects_grab_permission() {
        let control = RuntimeControl::new(false).unwrap();
        assert_eq!(control.toggle(false), None);
        assert!(!control.mouse_locked.load(Ordering::Acquire));

        assert_eq!(control.toggle(true), Some(true));
        assert!(control.mouse_locked.load(Ordering::Acquire));
    }

    #[test]
    fn emergency_unlock_always_clears_state() {
        let control = RuntimeControl::new(true).unwrap();
        assert!(control.mouse_locked.load(Ordering::Acquire));
        control.force_unlock();
        assert!(!control.mouse_locked.load(Ordering::Acquire));
    }
}
