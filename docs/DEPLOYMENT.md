# Grunner Deployment and Operations Guide

## Table of Contents
1. [Deployment Overview](#deployment-overview)
2. [System Requirements](#system-requirements)
3. [Installation Methods](#installation-methods)
4. [Configuration Management](#configuration-management)
5. [System Integration](#system-integration)
6. [Monitoring and Logging](#monitoring-and-logging)
7. [Updates and Maintenance](#updates-and-maintenance)
8. [Backup and Recovery](#backup-and-recovery)
9. [Security Considerations](#security-considerations)
10. [Performance Tuning](#performance-tuning)

## Deployment Overview

Grunner is designed as a single-user desktop application with minimal system requirements. It follows standard Linux desktop conventions and integrates seamlessly with modern desktop environments, particularly GNOME.

### Deployment Architecture
```
User Space Application
├── Binary: ~/.local/bin/grunner
├── Configuration: ~/.config/grunner/grunner.toml
├── Cache: ~/.cache/grunner/apps.bin
├── Icons: ~/.local/share/icons/hicolor/
└── Desktop Entry: ~/.local/share/applications/org.nihmar.grunner.desktop
```

### Deployment Scenarios
1. **Single User Desktop**: Standard installation for individual users
2. **Multi-User System**: Installed system-wide for all users
3. **Development Environment**: Source build with debugging enabled
4. **Testing Environment**: Isolated configuration for testing

## System Requirements

### Minimum Requirements
- **Operating System**: Linux with systemd (GNOME 40+ recommended)
- **CPU**: x86_64 or ARM64, 1+ GHz
- **Memory**: 512 MB RAM
- **Storage**: 10 MB for binary, additional for cache
- **Display**: 1024x768 resolution

### Recommended Requirements
- **Operating System**: GNOME 42+ on Wayland
- **CPU**: 2+ GHz multi-core
- **Memory**: 2 GB RAM
- **Storage**: SSD for faster application scanning
- **Display**: HiDPI support (200% scaling)

### Software Dependencies

#### Required Dependencies
```
gtk4 >= 4.6.0
libadwaita >= 1.6.0
glib2 >= 2.74.0
```

#### Optional Dependencies (for full functionality)
```
plocate    # File search (:f command)
ripgrep    # Content search (:fg command)
obsidian   # Obsidian integration (:ob command)
systemd    # Power management features
```

#### Development Dependencies
```
rustc >= 1.70.0
cargo >= 1.70.0
pkg-config
make
```

### Distribution-Specific Packages

#### Ubuntu/Debian (22.04+)
```bash
# Required
sudo apt install libgtk-4-1 libadwaita-1-0 libglib2.0-0

# Optional
sudo apt install plocate ripgrep obsidian

# Development
sudo apt install rustc cargo pkg-config libgtk-4-dev libadwaita-1-dev
```

#### Fedora/RHEL (36+)
```bash
# Required
sudo dnf install gtk4 libadwaita glib2

# Optional
sudo dnf install plocate ripgrep obsidian

# Development
sudo dnf install rust cargo pkg-config gtk4-devel libadwaita-devel
```

#### Arch Linux
```bash
# Required
sudo pacman -S gtk4 libadwaita glib2

# Optional
sudo pacman -S plocate ripgrep obsidian

# Development
sudo pacman -S rust cargo pkg-config
```

## Installation Methods

### Method 1: Automated Installation Script (Recommended)

The `build.sh` script provides a complete installation:

```bash
# Clone repository
git clone https://github.com/Nihmar/grunner.git
cd grunner

# Run installation script
./build.sh
```

**What the script does:**
1. Builds the application with release optimizations
2. Installs binary to `~/.local/bin/grunner`
3. Installs icons to appropriate directories
4. Creates desktop entry file
5. Updates icon cache and desktop database

### Method 2: Manual Installation from Source

```bash
# Clone repository
git clone https://github.com/Nihmar/grunner.git
cd grunner

# Build application
cargo build --release

# Create installation directories
mkdir -p ~/.local/bin
mkdir -p ~/.local/share/icons/hicolor/scalable/apps
mkdir -p ~/.local/share/applications

# Install binary
cp target/release/grunner ~/.local/bin/

# Install icon (if available)
if [ -f assets/grunner.svg ]; then
    cp assets/grunner.svg ~/.local/share/icons/hicolor/scalable/apps/org.nihmar.grunner.svg
fi

# Create desktop entry
cat > ~/.local/share/applications/org.nihmar.grunner.desktop << EOF
[Desktop Entry]
Type=Application
Name=grunner
Comment=Application launcher
Exec=$HOME/.local/bin/grunner
Icon=org.nihmar.grunner
Terminal=false
Categories=Utility;
StartupWMClass=org.nihmar.grunner
NoDisplay=false
EOF

# Update databases
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

### Method 3: System-Wide Installation

For multi-user systems or enterprise deployment:

```bash
# Build as root or with sudo
sudo cargo build --release

# Install to system directories
sudo cp target/release/grunner /usr/local/bin/
sudo cp assets/grunner.svg /usr/share/icons/hicolor/scalable/apps/org.nihmar.grunner.svg
sudo cp assets/org.nihmar.grunner.desktop /usr/share/applications/

# Update system databases
sudo gtk-update-icon-cache /usr/share/icons/hicolor
sudo update-desktop-database /usr/share/applications
```

### Method 4: Package Manager Installation

#### Creating a Distribution Package

**For .deb (Debian/Ubuntu):**
```bash
# Create Debian package structure
mkdir -p grunner-0.7.0/DEBIAN
mkdir -p grunner-0.7.0/usr/bin
mkdir -p grunner-0.7.0/usr/share/applications
mkdir -p grunner-0.7.0/usr/share/icons/hicolor/scalable/apps

# Copy files
cp target/release/grunner grunner-0.7.0/usr/bin/
cp assets/grunner.svg grunner-0.7.0/usr/share/icons/hicolor/scalable/apps/org.nihmar.grunner.svg
cp assets/org.nihmar.grunner.desktop grunner-0.7.0/usr/share/applications/

# Create control file
cat > grunner-0.7.0/DEBIAN/control << EOF
Package: grunner
Version: 0.7.0
Section: utils
Priority: optional
Architecture: amd64
Depends: libgtk-4-1, libadwaita-1-0, libglib2.0-0
Maintainer: Your Name <email@example.com>
Description: Fast keyboard-driven application launcher for GNOME
 A rofi-like application launcher built with Rust and GTK4/libadwaita.
 Features fuzzy application search, inline calculator, file search,
 content grep, Obsidian integration, and power management controls.
EOF

# Build package
dpkg-deb --build grunner-0.7.0
```

**For .rpm (Fedora/RHEL):**
```bash
# Create RPM spec file
cat > grunner.spec << EOF
Name: grunner
Version: 0.7.0
Release: 1%{?dist}
Summary: Fast keyboard-driven application launcher for GNOME
License: MIT
URL: https://github.com/Nihmar/grunner
Source0: grunner-%{version}.tar.gz
BuildRequires: rust cargo gtk4-devel libadwaita-devel
Requires: gtk4 libadwaita glib2

%description
A rofi-like application launcher built with Rust and GTK4/libadwaita.
Features fuzzy application search, inline calculator, file search,
content grep, Obsidian integration, and power management controls.

%prep
%autosetup

%build
cargo build --release

%install
install -D -m 755 target/release/grunner %{buildroot}%{_bindir}/grunner
install -D -m 644 assets/grunner.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/org.nihmar.grunner.svg
install -D -m 644 assets/org.nihmar.grunner.desktop %{buildroot}%{_datadir}/applications/org.nihmar.grunner.desktop

%files
%{_bindir}/grunner
%{_datadir}/icons/hicolor/scalable/apps/org.nihmar.grunner.svg
%{_datadir}/applications/org.nihmar.grunner.desktop

%changelog
* Tue Jan 01 2024 Your Name <email@example.com> - 0.7.0-1
- Initial package
EOF

# Build RPM
rpmbuild -ba grunner.spec
```

### Method 5: Containerized Deployment

**Dockerfile for testing:**
```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libgtk-4-1 \
    libadwaita-1-0 \
    libglib2.0-0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/grunner /usr/local/bin/grunner
CMD ["grunner"]
```

**Build and run:**
```bash
docker build -t grunner .
docker run -it --rm \
  -e DISPLAY=$DISPLAY \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v $HOME/.config:/home/user/.config \
  grunner
```

## Configuration Management

### Configuration File Location
```
Primary:   ~/.config/grunner/grunner.toml
Fallback:  Built-in defaults
Generated: On first run if missing
```

### Configuration Hierarchy
1. **User Configuration**: `~/.config/grunner/grunner.toml` (highest priority)
2. **System Configuration**: `/etc/grunner/grunner.toml` (if implemented)
3. **Environment Variables**: `GRUNNER_*` (if implemented)
4. **Built-in Defaults**: Hardcoded in `config.rs`

### Configuration Validation

**Syntax Validation:**
```bash
# Check TOML syntax
toml validate ~/.config/grunner/grunner.toml

# Check with Grunner
grunner --check-config
```

**Configuration Testing:**
```bash
# Test with temporary config
GRUNNER_CONFIG=/tmp/test.toml grunner

# Generate default config
grunner --generate-config > ~/.config/grunner/grunner.default.toml
```

### Configuration Migration

**Version-to-Version Migration:**
```bash
# Backup old config
cp ~/.config/grunner/grunner.toml ~/.config/grunner/grunner.toml.backup.$(date +%Y%m%d)

# Launch new version (auto-migrates if needed)
grunner

# Compare changes
diff ~/.config/grunner/grunner.toml.backup.* ~/.config/grunner/grunner.toml
```

### Environment-Specific Configurations

**Development Environment:**
```toml
# ~/.config/grunner/grunner.dev.toml
[window]
width = 800
height = 600

[search]
max_results = 100  # Show more results for testing
command_debounce_ms = 100  # Faster response

[calculator]
enabled = true
```

**Production Environment:**
```toml
# ~/.config/grunner/grunner.prod.toml
[window]
width = 640
height = 480

[search]
max_results = 50  # Conservative limit
command_debounce_ms = 300  # Reduce UI flicker

[calculator]
enabled = false  # Disable if not needed
```

## System Integration

### Desktop Environment Integration

#### GNOME Shell Integration
```bash
# Enable GNOME Shell search providers
gsettings set org.gnome.desktop.search-providers enabled "['org.gnome.Calendar.desktop', 'org.gnome.Contacts.desktop', 'org.gnome.Documents.desktop']"

# Set keyboard shortcut
gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/']"
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ name 'grunner'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ command 'grunner'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ binding '<Super>space'
```

#### Systemd Service (for auto-start)
```ini
# ~/.config/systemd/user/grunner.service
[Unit]
Description=Grunner Application Launcher
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/grunner
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

**Enable and start:**
```bash
systemctl --user enable grunner.service
systemctl --user start grunner.service
```

### File System Integration

#### Application Directory Scanning
Grunner scans these directories by default:
```
/usr/share/applications
/usr/local/share/applications
~/.local/share/applications
/var/lib/flatpak/exports/share/applications
~/.local/share/flatpak/exports/share/applications
```

**Adding custom directories:**
```toml
[search]
app_dirs = [
    "/usr/share/applications",
    "~/.local/share/applications",
    "~/Applications",  # Custom directory
    "/opt/myapps/applications",  # Third-party apps
]
```

#### Cache Management
```
Cache Location: ~/.cache/grunner/apps.bin
Cache Format: Binary serialization with bincode
Cache Invalidation: On directory modification
Cache Size: ~1-10 MB depending on number of applications
```

**Cache maintenance commands:**
```bash
# View cache info
ls -lh ~/.cache/grunner/

# Clear cache (will rebuild on next launch)
rm ~/.cache/grunner/apps.bin

# Force cache rebuild
grunner --rebuild-cache
```

### D-Bus Integration

#### Available D-Bus Interfaces
```
Service: org.nihmar.grunner
Object: /org/nihmar/grunner
Interface: org.nihmar.grunner.Application
```

**Querying via D-Bus:**
```bash
# List methods
gdbus introspect --session --dest org.nihmar.grunner --object-path /org/nihmar/grunner

# Call method
gdbus call --session --dest org.nihmar.grunner \
  --object-path /org/nihmar/grunner \
  --method org.nihmar.grunner.Application.GetVersion
```

#### GNOME Shell Search Provider Integration
Grunner can act as a search provider for GNOME Shell:

```xml
<!-- Search provider XML definition -->
<interface name="org.gnome.Shell.SearchProvider2">
  <method name="GetInitialResultSet">
    <arg type="as" name="terms" direction="in"/>
    <arg type="as" name="results" direction="out"/>
  </method>
  <method name="GetSubsearchResultSet">
    <arg type="as" name="previous_results" direction="in"/>
    <arg type="as" name="terms" direction="in"/>
    <arg type="as" name="results" direction="out"/>
  </method>
  <method name="GetResultMetas">
    <arg type="as" name="identifiers" direction="in"/>
    <arg type="aa{sv}" name="metas" direction="out"/>
  </method>
  <method name="ActivateResult">
    <arg type="s" name="identifier" direction="in"/>
    <arg type="as" name="terms" direction="in"/>
    <arg type="u" name="timestamp" direction="in"/>
  </method>
</interface>
```

## Monitoring and Logging

### Logging Configuration

#### Environment Variables for Logging
```bash
# Enable Rust logging
export RUST_LOG=grunner=info

# Enable GTK debug messages
export G_MESSAGES_DEBUG=all

# Enable GLib debug
export G_DEBUG=fatal_warnings

# Run with logging
RUST_LOG=debug ~/.local/bin/grunner 2>&1 | tee ~/grunner.log
```

#### Log Levels
- **error**: Critical errors that prevent operation
- **warn**: Non-critical issues or deprecations
- **info**: General operational information
- **debug**: Detailed debugging information
- **trace**: Very verbose tracing information

#### Log File Locations
```
Application Logs:  Standard output/error (when launched from terminal)
System Logs:       journalctl --user -u gnome-session
GTK Logs:          ~/.cache/gdk-log
GLib Logs:         G_MESSAGES_DEBUG environment variable
```

### Performance Monitoring

#### Key Performance Indicators
```bash
# Monitor startup time
time timeout 0.5 grunner --version

# Monitor memory usage
/usr/bin/time -v grunner --version 2>&1 | grep "Maximum resident"

# Monitor CPU usage during search
perf stat -e cycles,instructions,cache-references