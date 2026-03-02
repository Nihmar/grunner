# Flatpak Packaging for grunner

This directory contains Flatpak packaging for grunner, a fast, keyboard-driven application launcher for GNOME and other Linux desktops.

## Overview

The Flatpak package provides:
- Sandboxed execution environment
- Automatic desktop integration (.desktop file and icon)
- AppData metadata for software centers
- Bundled dependencies via GNOME Platform runtime

## Prerequisites

### 1. Install Flatpak and Flatpak-builder

**Fedora/RHEL/CentOS:**
```bash
sudo dnf install flatpak flatpak-builder
```

**Ubuntu/Debian:**
```bash
sudo apt install flatpak flatpak-builder
```

**Arch Linux:**
```bash
sudo pacman -S flatpak
```

### 2. Add Flathub repository
```bash
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
```

### 3. Install required runtimes
```bash
flatpak install flathub org.gnome.Platform//48 org.gnome.Sdk//48 -y
```

## Building the Flatpak

### Using the build script (recommended)
```bash
cd flatpak
./build.sh
```

This will:
1. Check for required tools and runtimes
2. Build the application
3. Create a bundle file `grunner.flatpak`

### Manual build
```bash
flatpak-builder --force-clean --install-deps-from=flathub flatpak/build flatpak/org.nihmar.grunner.yml
```

To create a bundle:
```bash
flatpak-builder --repo=flatpak/repo --force-clean flatpak/build flatpak/org.nihmar.grunner.yml
flatpak build-bundle flatpak/repo grunner.flatpak org.nihmar.grunner
```

## Installation

### Install from local bundle
```bash
flatpak install --user grunner.flatpak
```

### Install from local repository
```bash
flatpak --user remote-add --no-gpg-verify grunner-repo flatpak/repo
flatpak --user install grunner-repo org.nihmar.grunner
```

## Running the Application

```bash
flatpak run org.nihmar.grunner
```

You can also launch it from your desktop application launcher (search for "grunner").

## Manifest Structure

The manifest file (`org.nihmar.grunner.yml`) defines:

### App ID and Runtime
- **App ID**: `org.nihmar.grunner` (reverse-DNS naming)
- **Runtime**: `org.gnome.Platform//48` (GNOME 48 platform)
- **SDK**: `org.gnome.Sdk//48` (development tools)

### Permissions (finish-args)
The application requests these permissions:
- `--socket=wayland` / `--socket=x11`: GUI display
- `--socket=session-bus`: D-Bus session bus
- `--filesystem=home`: Access to home directory for file search and config
- `--share=network`: Network access for D-Bus communication
- `--talk-name=org.freedesktop.DBus`: Launch other applications
- `--talk-name=org.freedesktop.portal.Desktop`: Open files via xdg-open
- `--talk-name=org.freedesktop.portal.OpenURI`: Open URIs
- `--system-talk-name=org.freedesktop.login1`: Power management (optional)

### Build Process
1. Uses `cargo` buildsystem with Rust stable channel
2. Builds release binary with optimizations
3. Installs binary, desktop file, icon, and appdata
4. Updates icon cache

## Desktop Integration

The package includes:

- **Desktop file**: `org.nihmar.grunner.desktop`
- **Icon**: `org.nihmar.grunner.svg` (scalable vector)
- **AppData**: `org.nihmar.grunner.appdata.xml` (metadata for software centers)

## Dependencies

The application depends on these runtime libraries (provided by GNOME Platform 48):
- GTK4 ≥ 0.10
- libadwaita ≥ 0.8 with `v1_6` feature
- GLib, Cairo, Pango, GDK, etc.
- Rust standard library

## Distribution

### Testing Locally
Test the flatpak thoroughly before distribution:
```bash
flatpak run --devel --command=bash org.nihmar.grunner
```

### Publishing to Flathub
To publish to Flathub:

1. Fork the [Flathub repository](https://github.com/flathub/flathub)
2. Add your manifest to `apps/org.nihmar.grunner/org.nihmar.grunner.yml`
3. Submit a pull request

### Self-hosted Repository
You can host your own flatpak repository:
```bash
flatpak build-export flatpak/repo flatpak/build
ostree summary --repo=flatpak/repo --update
```

## Troubleshooting

### Build fails with "rustc not found"
Ensure you have the GNOME SDK installed:
```bash
flatpak install flathub org.gnome.Sdk//48
```

### Application cannot access home directory
Check permissions:
```bash
flatpak info org.nihmar.grunner
flatpak override --user org.nihmar.grunner --filesystem=home
```

### Icon not showing in launcher
Update icon cache:
```bash
flatpak run --command=gtk-update-icon-cache org.nihmar.grunner -f /app/share/icons/hicolor
```

### Debugging sandbox issues
Run with strace:
```bash
flatpak run --devel --command=strace org.nihmar.grunner
```

## File Structure

```
flatpak/
├── org.nihmar.grunner.yml          # Flatpak manifest
├── org.nihmar.grunner.desktop      # Desktop entry
├── grunner.appdata.xml             # AppData metadata
├── build.sh                        # Build script
└── README.md                       # This file
```

## License

The Flatpak packaging is licensed under the same MIT license as grunner itself.