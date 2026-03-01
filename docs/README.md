# Grunner Documentation

Welcome to the Grunner documentation! This directory contains comprehensive technical documentation for the Grunner application launcher.

## Documentation Structure

### Core Documentation
- **[OVERVIEW.md](OVERVIEW.md)** - High-level project overview, architecture, and technology stack
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Detailed system architecture, module dependencies, and data flow
- **[API.md](API.md)** - Complete API reference for all modules and functions
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development environment setup, testing, and contribution guidelines
- **[USER_GUIDE.md](USER_GUIDE.md)** - User-focused guide for installation, configuration, and usage
- **[DEPLOYMENT.md](DEPLOYMENT.md)** - Deployment, operations, and system integration guide

### Quick Links
- [Project README](../README.md) - Main project README with installation and basic usage
- [Source Code](../src/) - Source code directory
- [Configuration](../src/config.rs) - Configuration system implementation

## About Grunner

Grunner is a fast, keyboard-driven application launcher for GNOME and other Linux desktops, written in Rust. Built on GTK4 and libadwaita, it follows your system's light/dark theme and accent color automatically.

### Key Features
- **Fuzzy application search** with instant results
- **Inline calculator** for quick calculations
- **Colon command system** for file search, content grep, and more
- **Obsidian integration** for note management
- **GNOME Shell search provider** support
- **Power management** controls
- **Fully configurable** via TOML file

## Documentation Purpose

This documentation serves multiple audiences:

### For Users
- Installation instructions and basic usage
- Configuration options and examples
- Troubleshooting common issues
- Feature explanations and tutorials

### For Developers
- Architecture overview and design decisions
- API reference for all modules
- Development environment setup
- Testing and debugging guidelines
- Contribution workflow

### For System Administrators
- Deployment options and system requirements
- System integration and automation
- Performance tuning and monitoring
- Security considerations

## Getting Started

### Quick Start for Users
1. Read the [USER_GUIDE.md](USER_GUIDE.md) for installation and basic usage
2. Check the [configuration examples](../README.md#configuration) for customization
3. Refer to [troubleshooting](USER_GUIDE.md#troubleshooting) for common issues

### Quick Start for Developers
1. Review the [ARCHITECTURE.md](ARCHITECTURE.md) to understand the system design
2. Set up your development environment using [DEVELOPMENT.md](DEVELOPMENT.md)
3. Explore the [API.md](API.md) for module interfaces and functions

### Quick Start for Contributors
1. Read the [contribution guidelines](DEVELOPMENT.md#contributing)
2. Follow the code style and commit message conventions
3. Test your changes thoroughly before submitting

## Documentation Conventions

### Code Examples
Code examples are provided in Rust syntax unless otherwise specified. Configuration examples use TOML format.

### Cross-References
- Internal links use relative paths (e.g., `[ARCHITECTURE.md](ARCHITECTURE.md)`)
- Source code references use file paths (e.g., `../src/config.rs`)
- External links are clearly marked

### Version Information
This documentation corresponds to Grunner version 0.7.0. Check the [CHANGELOG](../CHANGELOG.md) for version-specific information.

## Contributing to Documentation

We welcome contributions to improve this documentation:

1. **Reporting Issues**: Open an issue for documentation errors, omissions, or clarifications needed
2. **Suggesting Improvements**: Propose new sections or better organization
3. **Submitting Changes**: Follow the same contribution process as code changes

### Documentation Standards
- Use clear, concise language
- Include practical examples where helpful
- Maintain consistent formatting and structure
- Update documentation when features change

## Additional Resources

- **GitHub Repository**: https://github.com/Nihmar/grunner
- **Issue Tracker**: https://github.com/Nihmar/grunner/issues
- **Discussions**: https://github.com/Nihmar/grunner/discussions

## License

The documentation is licensed under the same [MIT License](../LICENSE) as the Grunner project.

---

*Last Updated: Documentation for Grunner v0.7.0*