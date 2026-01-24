# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [0.1.3] - 2026-01-24

### Fixed
- Browser detection for Linux with standard paths (/usr/bin, /opt, /snap)
- Browser detection for Windows with Program Files and LOCALAPPDATA paths

### Added
- Comprehensive debug logging for browser discovery process including:
  - PATH checking for each candidate
  - Standard path checking per platform
  - LOCALAPPDATA environment variable verification
- Platform information in browser not found error message

## [0.1.2] - 2026-01-12

### Changed
- Updated dependencies

## [0.1.1] - 2026-01-12

### Added
- cargo-dist integration for automated releases
- Shell installer (install.sh) for Linux and macOS
- PowerShell installer (install.ps1) for Windows
- Prebuilt binaries for multiple platforms (macOS Intel/ARM64, Linux x86_64, Windows x86_64)
- `cargo install --git` installation method

### Changed
- Updated installation documentation with new installation methods
- Updated usage examples to use `web2pdf` command directly instead of `cargo run`

## [0.1.0] - 2025-12-30

### Added
- Initial release of web2pdf CLI tool
- Web page to PDF conversion using headless Chrome/Chromium
- Cross-platform support (Linux, macOS, Windows)
- Support for multiple architectures (x86_64, ARM64)
