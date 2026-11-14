#!/usr/bin/env bash
# Installa Pulse nativo per l'utente corrente (~/.local) con voce nel menu applicazioni.
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-release.sh
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -Dm755 src-tauri/target/release/pulse "$BIN_DIR/pulse"
install -Dm644 src-tauri/icons/icon.png "$ICON_DIR/pulse.png"
cat > "$APP_DIR/pulse.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Pulse
GenericName=Launcher globale
Comment=Launcher globale istantaneo e clipboard manager locale
Exec=${BIN_DIR}/pulse
Icon=pulse
Terminal=false
Categories=Utility;System;
Keywords=launcher;search;clipboard;appunti;
StartupNotify=false
DESKTOP
update-desktop-database "$APP_DIR" 2>/dev/null || true
gtk-update-icon-cache "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true
echo "✔ Pulse installato in ${BIN_DIR}/pulse — cercalo come 'Pulse' nel menu o premi Alt+Space"
