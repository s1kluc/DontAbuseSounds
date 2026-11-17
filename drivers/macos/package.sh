#!/bin/bash
set -euo pipefail
app=${1:?Usage: package.sh /path/to/DontAbuseSounds.app /path/to/output.dmg}
out=${2:?Pass a DMG output path}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
root="$work/root"
mkdir -p "$root/Applications" "$root/Library/Audio/Plug-Ins/HAL" "$work/image"
ditto "$app" "$root/Applications/DontAbuseSounds.app"
bash "$(dirname "$0")/build.sh" "$root/Library/Audio/Plug-Ins/HAL/DontAbuseSoundsMic.driver"
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  codesign --force --sign - --entitlements "$(dirname "$0")/../../desktop/Entitlements.plist"     "$root/Applications/DontAbuseSounds.app"
fi
codesign --verify --deep --strict "$root/Applications/DontAbuseSounds.app"
version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$root/Applications/DontAbuseSounds.app/Contents/Info.plist")
sign=()
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
  sign=(--sign "$APPLE_INSTALLER_IDENTITY")
fi
pkgbuild --root "$root" --ownership recommended --install-location / \
  --identifier com.s1kluc.dontabusesounds.installer --version "$version" \
  ${sign[@]+"${sign[@]}"} "$work/image/DontAbuseSounds.pkg"
cat > "$work/image/Install.txt" <<'NOTE'
Open DontAbuseSounds.pkg to install the app and virtual microphone.
Restart your Mac after installing so CoreAudio loads the microphone.
Open DontAbuseSounds, choose your hardware microphone and the
DontAbuseSounds Mic output, and click Start audio.
In Discord choose DontAbuseSounds Mic as Input Device.
NOTE
hdiutil create -quiet -ov -format UDZO -volname DontAbuseSounds -srcfolder "$work/image" "$out"
hdiutil verify "$out"
