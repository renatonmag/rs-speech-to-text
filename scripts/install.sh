#!/usr/bin/env bash
# Build a release binary and register it with the desktop app launcher.
# Usage: scripts/install.sh [--uninstall]
set -euo pipefail

APP=rs-speech-to-text
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"

BIN="$HOME/.local/bin/$APP"
ICON="$DATA/icons/hicolor/scalable/apps/$APP.svg"
DESKTOP="$DATA/applications/$APP.desktop"

refresh() {
    update-desktop-database "$DATA/applications" 2>/dev/null || true
    gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$BIN" "$ICON" "$DESKTOP"
    refresh
    echo "Uninstalled $APP."
    exit 0
fi

cd "$ROOT"
cargo build --release

install -Dm755 "target/release/$APP" "$BIN"
install -Dm644 "packaging/$APP.svg" "$ICON"
mkdir -p "$(dirname "$DESKTOP")"
sed "s|@BIN@|$BIN|" "packaging/$APP.desktop" > "$DESKTOP"
chmod 644 "$DESKTOP"

if command -v desktop-file-validate >/dev/null; then
    desktop-file-validate "$DESKTOP"
fi
refresh

echo "Installed $APP:"
echo "  binary:  $BIN"
echo "  icon:    $ICON"
echo "  launcher: $DESKTOP"
