#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MANIFEST="$SCRIPT_DIR/org.nihmar.grunner.yml"
BUILD_DIR="$SCRIPT_DIR/build"
REPO_DIR="$SCRIPT_DIR/repo"
BUNDLE="$SCRIPT_DIR/grunner.flatpak"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== grunner Flatpak Builder ===${NC}"

# Check if flatpak-builder is installed
if ! command -v flatpak-builder &> /dev/null; then
    echo -e "${RED}flatpak-builder is not installed.${NC}"
    echo "Please install flatpak-builder from your distribution's package manager."
    echo "For example:"
    echo "  Fedora: sudo dnf install flatpak-builder"
    echo "  Ubuntu/Debian: sudo apt install flatpak-builder"
    echo "  Arch: sudo pacman -S flatpak"
    exit 1
fi

# Check if required runtimes are installed
if ! flatpak info org.gnome.Platform//49 &> /dev/null; then
    echo -e "${YELLOW}GNOME Platform 49 runtime not found. Installing...${NC}"
    flatpak install flathub org.gnome.Platform//49 org.gnome.Sdk//49 -y
fi

# Clean previous build
if [ -d "$BUILD_DIR" ]; then
    echo -e "${YELLOW}Cleaning previous build...${NC}"
    rm -rf "$BUILD_DIR"
fi

# Build flatpak
echo -e "${GREEN}Building flatpak...${NC}"
flatpak-builder \
    --force-clean \
    --repo="$REPO_DIR" \
    --install-deps-from=flathub \
    --disable-updates \
    "$BUILD_DIR" \
    "$MANIFEST"

# Create bundle
echo -e "${GREEN}Creating bundle...${NC}"
flatpak build-bundle "$REPO_DIR" "$BUNDLE" org.nihmar.grunner

echo -e "${GREEN}✓ Flatpak bundle created: $BUNDLE${NC}"
echo ""
echo "To install locally:"
echo "  flatpak install --user $BUNDLE"
echo ""
echo "To run:"
echo "  flatpak run org.nihmar.grunner"
echo ""
echo "To build without installing dependencies every time, use:"
echo "  flatpak-builder --install-deps-from=flathub --force-clean $BUILD_DIR $MANIFEST"
