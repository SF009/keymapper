# Waydroid Keymapper

Low-latency Android/Waydroid keymapper for laptop keyboard and mouse.

The Android app owns the GUI, profiles, mappings, layout editor, overlay and touch backend. The Linux helper only captures laptop input with evdev and transports compact events through a persistent ADB port-forward.

    Laptop keyboard/mouse
            -> evdev host helper
            -> persistent ADB port-forward
            -> Android Input Gateway
            -> profile mapper
            -> Accessibility touch injector
            -> game

## Features

- Android GUI for profiles and mappings
- Drag-and-drop layout editor on Android
- Draggable joystick and aim controls
- Free Fire and Minimal starter profiles
- Keyboard TAP/HOLD bindings
- Mouse TAP/HOLD bindings
- Relative mouse aim with sensitivity and invert-Y
- Mouse lock toggle on F8
- Non-touchable in-game overlay
- Binary protocol with TCP_NODELAY
- Automatic ADB forward setup
- evdev input capture with optional exclusive grab
- Automatic reconnect after a dropped socket
- Android Accessibility gesture backend
- No third-party Android runtime libraries

## Android setup

1. Install the APK.
2. Grant Display over other apps.
3. Enable Waydroid Keymapper in Android Accessibility.
4. Start Input Gateway from the app.
5. Select or edit a game profile.
6. Open Layout Editor, drag controls, then save.
7. Show the overlay while the game is running.

The gateway listens on 127.0.0.1:27183 inside Android. The host helper exposes that socket only through ADB port-forwarding, so it is not opened to the LAN.

## Host setup

Build:

    cargo build --release --manifest-path host/Cargo.toml

List local input devices:

    ./host/target/release/waydroid-keymapper --list

Run with automatic device detection:

    ./host/target/release/waydroid-keymapper

Explicit devices:

    ./host/target/release/waydroid-keymapper       --keyboard /dev/input/event4       --mouse /dev/input/event8

Specific ADB target:

    ./host/target/release/waydroid-keymapper       --serial 127.0.0.1:5555

Disable evdev exclusive grab:

    ./host/target/release/waydroid-keymapper --no-grab

The host requires access to /dev/input/event* and an available adb binary.

## Build the Android app

The project uses Android Gradle Plugin 8.7.3 and Kotlin 2.0.21.

From the repository root, use a local Gradle installation:

    gradle :app:assembleDebug

The APK is produced under:

    app/build/outputs/apk/debug/app-debug.apk

A helper script is also provided:

    ./scripts/build.sh

## Latency

The host path is event-driven and sends mouse deltas immediately over a persistent ADB transport with TCP_NODELAY. No "adb shell input ..." process is spawned for each event.

The portable Android backend uses AccessibilityService.dispatchGesture. Android documents that dispatching a gesture cancels gestures already in progress, so this backend is intended as the portable fallback; true kernel-level multi-touch injection for rooted Waydroid can be added behind the same protocol later.

## Architecture

    GUI/Profile/Layout
             |
             +--> JSON profiles
             |
    Laptop --> evdev --> ADB forward --> InputGatewayService
                                      |
                                      v
                               TouchInjector
                                      |
                                      v
                          AccessibilityService
                                      |
                                      v
                                    Game

## License

GPL-3.0-or-later
