#!/usr/bin/env bash
# Build Rust staticlib (arm64) and Swift MusicDrums app.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=/dev/null
source "${HOME}/.cargo/env" 2>/dev/null || true

export CARGO_TARGET_DIR="${ROOT}/target"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.4}"
TARGET="aarch64-apple-darwin"

cd "$ROOT"
echo "==> rustup target add ${TARGET}"
rustup target add "${TARGET}" >/dev/null

echo "==> cargo build --release --target ${TARGET}"
cargo build --release --target "${TARGET}" -p music_drums_core

LIB_DIR="${ROOT}/target/${TARGET}/release"
LIB="${LIB_DIR}/libmusic_drums_core.a"
if [[ ! -f "$LIB" ]]; then
  echo "error: missing ${LIB}" >&2
  exit 1
fi
echo "Rust lib: ${LIB}"

HEADERS="${ROOT}/crates/music_drums_core/include"
SWIFT_DIR="${ROOT}/apps/MusicDrums"
OUT_DIR="${ROOT}/build"
APP="${OUT_DIR}/MusicDrums.app"
mkdir -p "${OUT_DIR}" "${APP}/Contents/MacOS" "${APP}/Contents/Resources"

echo "==> swiftc MusicDrums"
swiftc \
  -parse-as-library \
  -O \
  -target arm64-apple-macosx14.4 \
  -import-objc-header "${SWIFT_DIR}/BridgingHeader.h" \
  -I "${HEADERS}" \
  -L "${LIB_DIR}" \
  -lmusic_drums_core \
  -framework Cocoa \
  -framework AppKit \
  -framework CoreAudio \
  -framework AudioToolbox \
  -framework AVFoundation \
  -framework SwiftUI \
  -framework IOKit \
  -framework CoreFoundation \
  -framework Security \
  "${SWIFT_DIR}/Sources/MusicDrums/"*.swift \
  -o "${APP}/Contents/MacOS/MusicDrums"
cat > "${APP}/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>MusicDrums</string>
  <key>CFBundleIdentifier</key>
  <string>com.kevin.musicdrums</string>
  <key>CFBundleName</key>
  <string>MusicDrums</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.4</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSAudioCaptureUsageDescription</key>
  <string>Music Drums listens to system audio to pulse your MX Master 4 with the beat.</string>
</dict>
</plist>
PLIST

# Bundle logi-mode script
cp "${ROOT}/Scripts/logi-mode.sh" "${APP}/Contents/Resources/logi-mode.sh"
chmod +x "${APP}/Contents/Resources/logi-mode.sh" "${ROOT}/Scripts/logi-mode.sh"

echo "==> ad-hoc sign (required for Process Tap / TCC)"
codesign --force --deep --sign - \
  --entitlements "${SWIFT_DIR}/MusicDrums.entitlements" \
  "${APP}" 2>/dev/null || codesign --force --deep --sign - "${APP}"

echo "Built ${APP}"
echo "Run: open ${APP}"
echo "CLI: cargo run -p music_drums_core --release --bin music-drums-cli -- ping"
