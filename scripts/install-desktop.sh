#!/bin/sh
set -eu
cd "$(dirname "$0")"
application_dir="${XDG_DATA_HOME:-$HOME/.local/share}/virtual-mixer"
menu_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icon_dir="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/512x512/apps"
mkdir -p "$application_dir" "$menu_dir" "$icon_dir"
cp VirtualMixer "$application_dir/VirtualMixer"
chmod +x "$application_dir/VirtualMixer"
cp VirtualMixer.png "$icon_dir/VirtualMixer.png"
# Desktop entry quoting needs escaped backslashes, quotes, backticks and dollars.
exec_path=$(printf '%s' "$application_dir/VirtualMixer" | sed 's/[\\"`$]/\\&/g; s/%/%%/g')
printf '[Desktop Entry]\nType=Application\nName=Virtual Mixer\nExec="%s"\nIcon=VirtualMixer\nTerminal=false\nCategories=AudioVideo;Audio;\n' "$exec_path" > "$menu_dir/VirtualMixer.desktop"
printf '%s\n' 'Virtual Mixer installed in your application menu.'
