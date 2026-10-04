# Waydroid Keymapper — Rust

Low-latency keyboard/mouse to multitouch mapper for Waydroid.

## What it does

- evdev keyboard capture with optional exclusive grab
- evdev mouse capture
- WASD normalized analog joystick
- mouse-button held aim
- configurable mouse sensitivity, X/Y inversion, independent X/Y aim scaling and absolute-mode edge margin
- configurable joystick diagonal normalization
- touch and relative/unbounded FPS aim backends
- helper-inspired Aim ownership: aim can acquire/release mouse capture automatically
- runtime mouse lock/unlock with configurable toggle key (F8 by default)
- conflict detection for physical key/button reuse and reserved lock key
- tap and hold touch bindings
- up to 16 multitouch slots
- tracking IDs and pressure/major/minor fields
- normalized coordinates, independent of desktop resolution
- direct Waydroid touch FIFO output
- reconnect after Waydroid restarts and stable hotplug device paths when available
- CLI commands: run, check, devices, doctor
- user systemd unit and udev permissions

The hot path is event-driven: there is no 60 Hz polling loop. A kernel input event is consumed, mapped, batched and written immediately.

## Why the backend is Waydroid-specific

Modern Waydroid's EventHub integration defines three Wayland input FIFOs: wl_touch_events, wl_keyboard_events and wl_pointer_events. The touch FIFO is classified by Android as a multitouch device and supports ABS_MT_POSITION_X/Y, ABS_MT_TRACKING_ID, ABS_MT_PRESSURE and ABS_MT_SLOT.

This project therefore writes the Linux input_event ABI directly to wl_touch_events instead of creating a host uinput touchscreen that Android may never consume.

## Build on CachyOS / Arch

    sudo pacman -S --needed rustup gcc
    rustup default stable
    git clone https://github.com/SF009/keymapper
    cd keymapper
    cargo build --release
    install -Dm755 target/release/waydroid-keymapper ~/.local/bin/waydroid-keymapper

Install input permissions:

    sudo install -Dm644 udev/99-waydroid-keymapper.rules /etc/udev/rules.d/99-waydroid-keymapper.rules
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    sudo usermod -aG input "$USER"

Log out/in once after adding the input group.

## Configure

    mkdir -p ~/.config/waydroid-keymapper
    cp config.example.toml ~/.config/waydroid-keymapper/config.toml
    waydroid-keymapper devices
    nano ~/.config/waydroid-keymapper/config.toml
    waydroid-keymapper check

The default FIFO discovery checks:

    /dev/input/wl_touch_events
    /var/lib/waydroid/rootfs/dev/input/wl_touch_events
    /opt/waydroid/rootfs/dev/input/wl_touch_events

You can override it with WAYDROID_TOUCH_FIFO.

## FPS profile

The included example uses:

    W A S D       -> joystick slot 0
    Right mouse  -> aim slot 1 (auto-lock while held)
    Left mouse   -> fire hold slot
    Space        -> tap slot 2
    R            -> reload tap slot 3
    F            -> hold slot 4

For Fire, use a [[mouse_holds]] binding such as MOUSE_LEFT. The mapper also accepts MOUSE_MIDDLE, MOUSE_SIDE, MOUSE_EXTRA, MOUSE_FORWARD and MOUSE_BACK, plus common keyboard keys including 0-9, F1-F12, arrows, navigation keys and numpad digits.

## GTK4 GUI / Profile Editor

The GTK4 GUI is the primary management interface. It creates the first profile automatically and manages profiles, input devices, mappings, mouse lock/unlock, daemon installation/service state, Waydroid session state, validation, and input permissions. Normal daemon/profile management happens without a root shell; the optional input-permission repair uses a graphical polkit authentication prompt.

The project also ships a native GTK4 profile editor:

    cargo build --release --bin keymapper-gui
    install -Dm755 target/release/keymapper-gui ~/.local/bin/keymapper-gui
    install -Dm644 data/waydroid-keymapper.desktop ~/.local/share/applications/waydroid-keymapper.desktop
    update-desktop-database ~/.local/share/applications 2>/dev/null || true

On CachyOS / Arch, install GTK4 development/runtime packages before building:

    sudo pacman -S --needed gtk4 pkgconf

Launch it with:

    keymapper-gui

Profiles are stored in:

    ~/.config/waydroid-keymapper/profiles/

The GUI supports:
- automatic physical keyboard/mouse detection with stable `/dev/input/by-id` paths preferred and virtual-device filtering
- live daemon status, mouse LOCKED/UNLOCKED state, and diagnostics through a private per-user Unix control socket
- Install / Repair, Start, Stop, Restart, Enable-at-login and Disable-at-login for the user daemon service
- direct Lock, Unlock and Toggle commands without editing TOML or using the terminal
- Waydroid session Start/Stop controls and status display

- create, duplicate, delete and rename profiles
- import the existing active config as the first profile
- keyboard TAP/HOLD mappings
- mouse TAP/HOLD mappings
- optional WASD joystick
- optional mouse aim mapping with sensitivity, X/Y scaling, X/Y inversion and absolute-mode edge margin
- device selection from detected evdev devices
- 16:9 touch-map preview with click-to-select and drag-to-position
- normalized X/Y and touch-slot editing
- duplicate-slot validation before saving
- Save, Validate and Apply & Run
- Apply & Run copies the selected profile to ~/.config/waydroid-keymapper/config.toml and starts/restarts the user systemd service automatically

The GUI is not used by the daemon's input threads, so it adds no GUI work to the latency-sensitive input path.

## Run

Start Waydroid, then:

    waydroid-keymapper run

The daemon always starts with the configured ownership state only; the shipped defaults and GUI presets keep startup mouse lock **off**. A physical Aim button can auto-lock only after a real button press. F8 is the manual lock toggle, and Ctrl+Alt+F12 is the emergency unlock.

The GUI installs and manages the optional user service automatically. The manual service commands below are only a fallback for headless setups.

Optional user service (manual/headless):

    install -Dm644 systemd/waydroid-keymapper.service ~/.config/systemd/user/waydroid-keymapper.service
    systemctl --user daemon-reload
    systemctl --user enable --now waydroid-keymapper

Do not run the daemon as root when the udev input permissions are configured.

## Aim behavior

Two aim paths are available. `mode="touch"` keeps a virtual multitouch finger and is the compatibility path for touch-first shooters such as Free Fire; when it reaches the configured edge margin it performs an explicit UP -> recenter -> DOWN transaction instead of teleporting an existing finger. `mode="relative"` emits relative `REL_X/REL_Y` through Waydroid's pointer FIFO for unbounded FPS camera motion.

Waydroid's modern hardware composer also has an Android pointer-capture path that uses Wayland pointer constraints and a relative-pointer interface. The direct FIFO relative backend here is intentionally separate from that Android API, so support should be tested against the exact Waydroid image/vendor in use.

## Performance design

- blocking evdev reads instead of periodic polling
- one dedicated input thread per physical input source
- no GUI/network/ADB work in the input path
- compact touch-event batches
- normalized coordinates
- no screenshots or OCR
- EVIOCGRAB mouse/keyboard capture when enabled
- runtime mouse lock toggle: F8 by default; **mouse lock is off by default** so the shipped profiles do not capture the desktop cursor unexpectedly at startup
- Aim ownership is explicit: a manual F8 lock survives Aim release, while an Aim-owned lock is released when the Aim button is released
- unlocked mouse events are drained from the mapper's private evdev queue so the thread cannot busy-spin on permanent POLLIN while GNOME continues receiving its own event stream
- relative mouse motion is batched per evdev read before being written to Android
- configurable realtime scheduler priority (best-effort), FIFO retries/wait/reconnect backoff and Android touch pressure/major/minor tuning
- conflict validation prevents ambiguous physical-input ownership

The project is intentionally small enough to run comfortably on low-RAM systems.

## Diagnostics

Before gameplay, run:

    waydroid-keymapper doctor

This checks the configured evdev devices, Waydroid input FIFOs, configuration validity and the Waydroid command. It is the fastest way to distinguish a mapper problem from a Waydroid-image/vendor input problem.

## License

GPL-3.0-or-later


### Mouse lock safety

Locking the mouse intentionally grabs the selected physical evdev mouse so GNOME cannot consume the same movement stream. The recommended shooter path is `auto_lock_on_aim=true`: holding the configured Aim button acquires the grab, and releasing Aim gives ownership back. A manual F8 lock is still available; `Ctrl+Alt+F12` is a built-in emergency unlock and remains available even when the keyboard is grabbed.


## Advanced tuning

The GUI exposes low-latency tuning under **Performance** and **Touch input tuning**. `realtime_priority` is best-effort because an unprivileged user service may not have permission to enter `SCHED_FIFO`; the mapper continues normally when the request is denied. FIFO retries/wait/reconnect values are bounded during validation to prevent accidental long stalls in the input path. Touch pressure/major/minor values are advanced compatibility controls for Android input-device interpretation.

The absolute aim `edge_margin` controls how close a touch-mode aim point may approach the screen edge before the mapper returns it to the configured aim center. The default is `0.12`; use `0` to effectively disable the early margin while retaining the final normalized bounds.
