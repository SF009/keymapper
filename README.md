# Waydroid Keymapper — Rust

Low-latency keyboard/mouse to multitouch mapper for Waydroid.

## What it does

- evdev keyboard capture with optional exclusive grab
- evdev mouse capture
- WASD normalized analog joystick
- mouse-button held aim
- configurable mouse sensitivity and Y inversion
- touch and relative/unbounded FPS aim backends
- runtime mouse lock/unlock with configurable toggle key (F8 by default)
- conflict detection for physical key/button reuse and reserved lock key
- tap and hold touch bindings
- up to 16 multitouch slots
- tracking IDs and pressure/major/minor fields
- normalized coordinates, independent of desktop resolution
- direct Waydroid touch FIFO output
- reconnect after Waydroid restarts and stable hotplug device paths when available
- CLI commands: run, check, devices
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
    Right mouse  -> aim slot 1
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
- optional mouse aim mapping with sensitivity and Y inversion
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

The GUI installs and manages the optional user service automatically. The manual service commands below are only a fallback for headless setups.

Optional user service (manual/headless):

    install -Dm644 systemd/waydroid-keymapper.service ~/.config/systemd/user/waydroid-keymapper.service
    systemctl --user daemon-reload
    systemctl --user enable --now waydroid-keymapper

Do not run the daemon as root when the udev input permissions are configured.

## Aim behavior

Two aim paths are available. `mode="touch"` keeps a virtual multitouch finger and is the compatibility path for touch-first shooters such as Free Fire. `mode="relative"` emits relative `REL_X/REL_Y` motion through Waydroid's pointer input FIFO, avoiding the old edge-recenter jump and keeping the host pointer captured while aiming.

Waydroid's modern hardware composer also has an Android pointer-capture path that uses Wayland pointer constraints and a relative-pointer interface. The direct FIFO relative backend here is intentionally separate from that Android API, so support should be tested against the exact Waydroid image/vendor in use.

## Performance design

- blocking evdev reads instead of periodic polling
- one dedicated input thread per physical input source
- no GUI/network/ADB work in the input path
- compact touch-event batches
- normalized coordinates
- no screenshots or OCR
- EVIOCGRAB mouse/keyboard capture when enabled
- runtime mouse lock toggle: F8 by default; **mouse lock is off by default** so starting the daemon never captures the desktop cursor unexpectedly; locking releases active aim/fire touch slots on unlock
- relative mouse motion is batched per evdev read before being written to Android
- conflict validation prevents ambiguous physical-input ownership

The project is intentionally small enough to run comfortably on low-RAM systems.

## License

GPL-3.0-or-later


### Mouse lock safety

Locking the mouse intentionally grabs the selected physical evdev mouse so GNOME cannot consume the same movement stream. The GUI therefore cannot receive mouse clicks while the lock is active. Use the configured toggle key (default `F8`) to unlock; `Ctrl+Alt+F12` is a built-in emergency unlock and remains available even when the keyboard is grabbed.
