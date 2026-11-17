#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
out=${1:?Usage: build.sh /absolute/path/to/DontAbuseSoundsMic.driver}
mkdir -p "$out/Contents/MacOS" "$out/Contents/Resources"
xcrun clang -arch arm64 -arch x86_64 -mmacosx-version-min=12.0 -std=gnu11 -O2 -bundle \
  -framework CoreFoundation -framework CoreAudio Mic.c -o "$out/Contents/MacOS/DontAbuseSoundsMic"
cp Info.plist "$out/Contents/"
cp LICENSE.txt "$out/Contents/Resources/"
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$out"
else
  codesign --force --sign - "$out"
fi
codesign --verify --strict "$out"
