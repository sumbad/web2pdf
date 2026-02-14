# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [0.4.0] - 2026-02-13

### Added
- **CLI improvements**: Migrated from manual argument parsing to `clap` library for better user experience
- **Debug flag**: Added `--debug` / `-d` flag for verbose tracing output
- **Optional output**: Made output file argument optional with default value `output.pdf`
- **Auto-suggestions**: Added intelligent argument suggestions for typos (e.g., `--debhg` → `--debug`)
- **Colored output**: Added colored terminal output with custom styling (green headers, cyan literals, yellow usage, red errors)

### Changed
- **CLI syntax**: Updated from `web2pdf <URL> <output.pdf>` to `web2pdf [--debug] <URL> [OUTPUT]`
- **Logging consistency**: Replaced remaining `eprintln!` statements with `tracing::error!` for structured logging

---

## [0.3.4] - 2026-02-10

### Fixed
- **mdBook lists**: Added safety improvements for list rendering in Chrome → PDF conversion
- **Text wrapping**: Fixed text wrapping around links containing code tags

---

## [0.3.3] - 2026-02-07

### Fixed
- **Acrobat compatibility**: Fixed critical issue where root Pages node incorrectly retained a Parent reference, causing display problems in Acrobat Reader
- **Page loading timeout**: Now actively stops page loading after timeout instead of continuing, equivalent to pressing browser stop button
- **PDF save timeout**: Increased PDF save timeout from 10s to 60s to handle tagged PDF generation

### Changed
- **Article preparation**: Simplified preprocessing script by removing comment loading and expansion logic
- **Navigation timeout**: Reduced from 10s to 5s for faster failure detection


---

## [0.3.2] - 2026-02-05

### Fixed
- **Navigation timeout**: Fixed indefinite hanging on pages with long-loading external resources (ads, trackers) by continuing after timeout instead of failing

### Changed
- **Reduced timeout**: Lowered navigation timeout from 30s to 10s for faster failure detection
- **Logging system**: Migrated to `EnvFilter` for fine-grained control over log levels per crate
- **Debug logging**: Added detailed debug traces for HTML fetching, page creation, navigation, and PDF operations
- **TOC filtering**: Changed from `filter().collect()` to `retain()` for better performance and no cloning
- **Error handling**: Added check for empty TOC after filtering to prevent merge errors


## [0.3.1] - 2026-02-03

### Fixed
- **mdBook cleanup**: Reduced aggressiveness of page cleanup for mdBook to preserve a content structure


## [0.3.0] - 2026-02-02

### Added
- **PDF structure sanitization**: Clean up PDF/UA structure by dissolving NonStruct elements and removing OBJR references
- **PDF structure merging**: Merge PDF structure trees along with document content for multi-page PDFs
- **Enhanced mdBook adapter**: Added page cleanup JavaScript for better mdBook PDF output
- **PDF/UA compliance tools**: Helper functions for working with tagged PDF structure elements
- **JavaScript sanitation utilities**: Code style cleanup scripts for web pages

### Changed
- **PDF merging architecture**: Refactored PDF merging to preserve structural elements and accessibility tags
- **File organization**: Reorganized PDF utility modules into separate files for better maintainability
- **Adapter system**: Enhanced mdBook adapter with additional page preparation steps

### Fixed
- **Structure preservation**: Maintain PDF/UA compliance when merging multiple PDF documents
- **Parent-child relationships**: Properly wire structural element hierarchies in merged PDFs
- **Page references**: Correctly shift StructParents indices when combining documents


## [0.2.0] - 2026-01-24

### Added
- Adapter registry system for pluggable resource-specific processing
- ResourceAdapter trait with `before_page()` and `after_page()` hooks for page customization
- ResourceDetector trait for fast HTML-based and slow browser-based detection
- MdBookAdapter with automatic light theme forcing for mdBook documentation sites
- DefaultAdapter with page cleanup for screen readers
- Table of Contents (TOC) generation system supporting multiple sources:
  - XML sitemap parsing
  - mdBook navbar navigation parsing
- Hierarchical bookmark support with parent-child relationships in PDF bookmarks
- TocNode structure to track file path, title, URL, and hierarchy level
- Chapter number extraction from URLs for automatic title formatting

### Changed
- Improved bookmark hierarchy tracking with level-based parent relationships


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
