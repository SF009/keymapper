# Waydroid Keymapper

Low-latency Android/Waydroid keymapper for laptop keyboard and mouse.

The Android app contains the GUI, profiles, mapping rules and layout editor. The Linux helper only captures laptop input with evdev and sends compact events through a persistent ADB port-forward.

    Laptop keyboard/mouse -> evdev -> host helper -> adb forward -> Android gateway -> profile mapper -> touch injector -> game

The host does not spawn adb shell input for every event.

## Features
- Android profile editor and drag layout editor
- Free Fire and Minimal starter profiles
- Keyboard TAP/HOLD
- Mouse button TAP/HOLD
- Relative mouse aim with sensitivity and invert-Y
- WASD joystick visualization
- Non-touchable in-game overlay
- Binary input protocol with TCP_NODELAY
- Linux evdev capture with optional exclusive grab
- F8 mouse-lock toggle
- Android Accessibility gesture backend
- No third-party Android runtime libraries

## Waydroid setup
1. Install the APK.
2. Grant Display over other apps.
3. Enable Waydroid Keymapper in Accessibility.
4. Start Input Gateway.
5. Select/edit a profile and show the overlay.
6. Build and run the Linux host helper.

## Host
Build:
    cargo build --release --manifest-path host/Cargo.toml

List devices:
    ./host/target/release/waydroid-keymapper --list

Run:
    ./host/target/release/waydroid-keymapper

Explicit devices:
    ./host/target/release/waydroid-keymapper --keyboard /dev/input/event4 --mouse /dev/input/event8

Specific adb target:
    ./host/target/release/waydroid-keymapper --serial 127.0.0.1:5555

The host requires access to /dev/input/event*.

## Latency
Input capture is event-driven. Mouse deltas are transmitted immediately; only the Android aim gesture dispatcher uses a short 4 ms coalescing interval to prevent gesture backlog.

## Limitation
The portable backend uses AccessibilityService.dispatchGesture, not privileged kernel injection. A rooted native uinput backend can later use exactly the same binary gateway protocol.
