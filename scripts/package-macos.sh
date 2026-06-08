#!/bin/bash
# macOS packaging script: builds release binary and bundles into .app

set -e

OUTDIR="${1:-build/macos}"
APPNAME="RustixEngine"
BUNDLE="$OUTDIR/$APPNAME.app"

 echo "Building Rustix Engine (macOS Release)..."
cargo build --release --workspace

mkdir -p "$BUNDLE/Contents/MacOS"
mkdir -p "$BUNDLE/Contents/Resources"

cp "target/release/rustix-runtime" "$BUNDLE/Contents/MacOS/$APPNAME"
cp -r "assets" "$BUNDLE/Contents/Resources/" 2>/dev/null || true
cp -r "shaders" "$BUNDLE/Contents/Resources/" 2>/dev/null || true

cat > "$BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APPNAME</string>
    <key>CFBundleIdentifier</key>
    <string>com.rustix.engine</string>
    <key>CFBundleName</key>
    <string>Rustix Engine</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
</dict>
</plist>
EOF

echo "macOS bundle created: $BUNDLE"
