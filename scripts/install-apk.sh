#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APK="$ROOT/app/build/outputs/apk/debug/app-debug.apk"

if [[ ! -f "$APK" ]]; then
  echo "APK not found. Run ./scripts/build.sh first." >&2
  exit 2
fi

adb install -r "$APK"
echo "Installed: $APK"
