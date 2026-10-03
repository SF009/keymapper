# Waydroid Keymapper — Rust

Low-latency keyboard/mouse to multitouch mapper for Waydroid.

## What it does

- evdev keyboard capture with optional exclusive grab
- evdev mouse capture
- WASD normalized analog joystick
- mouse-button held aim
- configurable mouse sensitivity and Y inversion
- tap and hold touch bindings
- up to 16 multitouch slots
- tracking IDs and pressure/major/minor fields
- normalized coordinates, independent of desktop resolution
- direct Waydroid touch FIFO output
- reconnect after Waydroid restarts
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

For Fire, add a hold binding for MOUSE_LEFT by using a keyboard-compatible physical button mapping in a future profile backend; the current core keeps keyboard and mouse buttons separate by design.

## Run

Start Waydroid, then:

    waydroid-keymapper run

Optional user service:

    install -Dm644 systemd/waydroid-keymapper.service ~/.config/systemd/user/waydroid-keymapper.service
    systemctl --user daemon-reload
    systemctl --user enable --now waydroid-keymapper

Do not run the daemon as root when the udev input permissions are configured.

## Aim behavior

The generic Waydroid touch interface is absolute. The current mapper keeps an aim finger alive and moves it from the center. When it approaches the edge it recenters before the host mouse reaches a desktop boundary.

That solves the normal cursor-boundary problem, but it is not mathematically identical to a native relative Android mouse. Truly unbounded FPS aim requires an Android-side relative MotionEvent/socket backend or a Waydroid patch that preserves relative motion.

## Performance design

- blocking evdev reads instead of periodic polling
- one dedicated input thread per physical input source
- no GUI/network/ADB work in the input path
- compact touch-event batches
- normalized coordinates
- no screenshots or OCR
- optional EVIOCGRAB so the desktop does not also consume the captured game controls

The project is intentionally small enough to run comfortably on low-RAM systems.

## License

GPL-3.0-or-later
