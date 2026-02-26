#!/bin/bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

APP_ID="org.nihmar.grunner"
BIN_DIR="$HOME/.local/bin"
ICON_DIR_SVG="$HOME/.local/share/icons/hicolor/scalable/apps"
ICON_DIR_PNG="$HOME/.local/share/icons/hicolor/256x256/apps"
DESKTOP_DIR="$HOME/.local/share/applications"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

echo "Building grunner in release mode..."
cargo build --release
echo "Build successful."

# ---------------------------------------------------------------------------
# Install binary
# ---------------------------------------------------------------------------

mkdir -p "$BIN_DIR"
cp "./target/release/grunner" "$BIN_DIR/grunner"
echo "Binary installed to $BIN_DIR/grunner"

# ---------------------------------------------------------------------------
# Install icon
#
# Place your icon at one of these paths in the repo before running this script:
#   assets/grunner.svg   (preferred — scales to any size)
#   assets/grunner.png   (fallback — should be at least 256x256)
# ---------------------------------------------------------------------------

ICON_INSTALLED=0

if [ -f "./assets/grunner.svg" ]; then
    mkdir -p "$ICON_DIR_SVG"
    cp "./assets/grunner.svg" "$ICON_DIR_SVG/${APP_ID}.svg"
    echo "SVG icon installed to $ICON_DIR_SVG/${APP_ID}.svg"
    ICON_INSTALLED=1
elif [ -f "./assets/grunner.png" ]; then
    mkdir -p "$ICON_DIR_PNG"
    cp "./assets/grunner.png" "$ICON_DIR_PNG/${APP_ID}.png"
    echo "PNG icon installed to $ICON_DIR_PNG/${APP_ID}.png"
    ICON_INSTALLED=1
else
    echo "Warning: no icon found at assets/grunner.svg or assets/grunner.png — skipping icon install."
    echo "         The app will use a generic icon in GNOME Shell."
fi

if [ "$ICON_INSTALLED" -eq 1 ]; then
    # -f forces the update even if the cache appears up to date.
    # -t skips the check for a writable index.theme (required for user-local dirs).
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
# Install .desktop file
#
# The file must be named <APP_ID>.desktop so GNOME Shell can match the running
# window (whose Wayland app-id is the GApplication ID) to this entry.
# ---------------------------------------------------------------------------

mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_DIR/${APP_ID}.desktop" << EOF
[Desktop Entry]
Type=Application
Name=grunner
Comment=Application launcher
Exec=$BIN_DIR/grunner/grunner
Icon=${APP_ID}
Terminal=false
Categories=Utility;
StartupWMClass=${APP_ID}
NoDisplay=false
EOF

update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
echo ".desktop file installed to $DESKTOP_DIR/${APP_ID}.desktop"

# ---------------------------------------------------------------------------

echo ""
echo "Installation complete."
echo "If this is the first install, log out and back in (or run: killall -3 gnome-shell)"
echo "to make GNOME Shell pick up the new icon and .desktop entry."
