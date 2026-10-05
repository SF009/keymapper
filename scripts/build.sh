#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "[1/2] Android APK"
if [[ -x "$ROOT/gradlew" ]]; then
  "$ROOT/gradlew" :app:assembleDebug
elif command -v gradle >/dev/null 2>&1; then
  gradle :app:assembleDebug
else
  echo "error: Gradle is not installed and ./gradlew is missing." >&2
  exit 2
fi

echo "[2/2] Linux host"
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required for the Linux helper." >&2
  exit 2
fi
cargo build --release --manifest-path "$ROOT/host/Cargo.toml"

echo
echo "APK:  $ROOT/app/build/outputs/apk/debug/app-debug.apk"
echo "Host: $ROOT/host/target/release/waydroid-keymapper"
