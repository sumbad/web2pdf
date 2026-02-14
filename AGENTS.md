# Agents Guide

## Build/Test Commands
- **Build**: `cargo build` (add `--release` for optimized builds)
- **Test single test**: `cargo test -- <test_name>` 
- **Run test binary**: `cargo run --bin test_merge`
- **Test all**: `cargo test`
- **Run**: `cargo run -- [--debug] <URL> <output.pdf>`
- **Lint**: `cargo clippy -- -D warnings` (strict clippy with warnings as errors)
- **Format**: `cargo fmt`
- **Check**: `cargo check`

## Code Style Guidelines
- **Rust edition**: 2024
- **Error handling**: Use `anyhow::{Context, Result}` for application errors, `lopdf::Result` for PDF operations
- **Imports**: Group standard library imports first, then external crates, then local modules
- **Async**: Use `#[tokio::main]` for async main functions
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants
- **Comments**: Russian comments are acceptable in this codebase (mixed language project)
- **Logging**: Use `tracing` crate with `tracing_subscriber` for structured logging
- **Module prefixes**: Private modules use `_` prefix (e.g., `_pdf_utils`, `_adapters`)

## Project Architecture

### Adapter Pattern
The project uses an adapter registry system to handle different content types:
- **AdapterRegistry**: Registry for detecting and using appropriate content adapters
- **ResourceAdapter trait**: Base trait for all adapters with `before_page()` and `after_page()` hooks
- **MdBookAdapter**: Special adapter for mdbook documentation format

### Key Modules
- `main.rs`: Entry point with URL argument parsing, browser launch, PDF generation pipeline
- `browser_utils.rs`: Browser configuration and finding browser executable
- `toc.rs`: Table of Contents generation from web pages
- `_pdf_utils/`: PDF manipulation utilities (merge, sanitize, helpers)
- `_adapters/`: Content adapters for different documentation formats
- `_adapter_registry/`: Registry system for adapter detection and instantiation

### Dependencies
- `reqwest`: HTTP client for fetching web content
- `chromiumoxide`: Browser automation (CDP) for rendering pages
- `lopdf`: PDF manipulation and merging
- `tokio`: Async runtime
- `scraper`: HTML parsing
- `tempfile`: Temporary file management
- `quick-xml`: XML parsing
- `tracing`/`tracing-subscriber`: Structured logging
- `anyhow`: Error handling

## Debug Mode
Use `--debug` flag to enable verbose tracing output:
- Shows detailed browser configuration and CDP events
- Limits pages in debug builds (first 3 pages)
- Logs at `debug` level for web2pdf and chromiumoxide crates