#!/bin/bash
# macOS 앱 빌드 -> dist/QRWedge.app  (맥에서 실행: bash build_mac.sh)
# 처음 한 번: xcode-select --install  그리고  curl https://sh.rustup.rs -sSf | sh
set -e
cd "$(dirname "$0")"
# rustup 설치 직후 PATH에 cargo가 아직 없을 수 있다
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "cargo 없음: curl https://sh.rustup.rs -sSf | sh 로 Rust 설치"; exit 1; }
cargo build --release --bin QRWedge

APP=dist/QRWedge.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/QRWedge "$APP/Contents/MacOS/"

# 아이콘: assets/icon.png -> icon.icns
ICONSET="$(mktemp -d)/icon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>QR Wedge</string>
  <key>CFBundleDisplayName</key><string>QR Wedge</string>
  <key>CFBundleExecutable</key><string>QRWedge</string>
  <key>CFBundleIdentifier</key><string>net.oktoya.qrwedge</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1.0.0</string>
  <key>CFBundleShortVersionString</key><string>1.0.0</string>
  <key>NSHumanReadableCopyright</key><string>© OKTO</string>
  <key>NSCameraUsageDescription</key><string>QR/바코드 스캔에 카메라를 사용합니다.</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF

codesign --force --deep -s - "$APP"
echo "완료: $APP"
