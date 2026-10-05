# Linux host helper

The host helper captures the laptop keyboard and mouse with Linux evdev and sends a compact binary event stream through an ADB TCP forward.

## Build

    cargo build --release --manifest-path host/Cargo.toml

## Find input devices

    ./host/target/release/waydroid-keymapper --list

## Run

    ./host/target/release/waydroid-keymapper

The helper automatically:

1. starts the Android InputGatewayService through ADB,
2. creates tcp:27183 -> tcp:27183 with adb forward,
3. opens the selected keyboard and mouse evdev devices,
4. optionally grabs them exclusively,
5. keeps one TCP connection per input source,
6. reconnects when Android restarts.

Use --keyboard and --mouse to override automatic device detection.

The F8 key toggles the mouse grab/aim capture state. Use --no-grab when you do not want exclusive input capture.
