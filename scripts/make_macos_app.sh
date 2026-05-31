#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_NAME="discord_msg_sender"
APP_NAME="DiscordVoiceTUI"
BUNDLE_ID="dev.anna.discordvoicetui"
APP="$ROOT/dist/$APP_NAME.app"

cd "$ROOT"
echo "==> cargo build --release"
cargo build --release

echo "==> assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/release/$BIN_NAME" "$APP/Contents/MacOS/$BIN_NAME"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>$BIN_NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSMicrophoneUsageDescription</key><string>Discord voice (E2EE) needs the microphone to transmit your voice.</string>
</dict>
</plist>
PLIST

echo "==> ad-hoc codesign (stable TCC identity)"
codesign --force --sign - --identifier "$BUNDLE_ID" \
  --options runtime "$APP/Contents/MacOS/$BIN_NAME" 2>/dev/null || \
  codesign --force --sign - --identifier "$BUNDLE_ID" "$APP/Contents/MacOS/$BIN_NAME"
codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$APP" 2>/dev/null || true

echo "==> done: $APP"
echo "Run it FROM kitty (keeps the TUI, mic permission tracked on the bundle):"
echo "  \"$APP/Contents/MacOS/$BIN_NAME\" --tui"
echo "First launch: macOS will prompt for Microphone — click Allow."
