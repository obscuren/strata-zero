#!/bin/sh
# Installs Strata Zero for you: the game as ~/.local/bin/strata-zero, its icon, and its
# entry in your desktop's app menu. `./install.sh --uninstall` takes them out again.
# Your cities stay in ~/.local/share/strata-zero either way.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin/strata-zero"
icon="$data/icons/hicolor/256x256/apps/strata-zero.png"
desktop="$data/applications/strata-zero.desktop"

# Lets the app menu see the change, where the desktop has the tools for it.
refresh() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$data/applications" || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t "$data/icons/hicolor" || true
    fi
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$bin" "$icon" "$desktop"
    refresh
    echo "Strata Zero is uninstalled. Your cities are still in $data/strata-zero."
    exit 0
fi

mkdir -p "$(dirname "$bin")" "$(dirname "$icon")" "$(dirname "$desktop")"
install -m 755 "$here/strata-zero" "$bin"
install -m 644 "$here/strata-zero.png" "$icon"
sed "s|^Exec=.*|Exec=\"$bin\"|" "$here/strata-zero.desktop" > "$desktop"
refresh
echo "Strata Zero is installed: it is in your app menu, and runs as $bin."
