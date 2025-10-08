# Agents Guide

## Build/Test Commands
- **Build**: `cargo build` (add `--release` for optimized builds)
- **Test single test**: `cargo test -- <test_name>` 
- **Run test binary**: `cargo run --bin test_merge`
- **Test all**: `cargo test`
- **Run**: `cargo run -- <URL> <output.pdf>`
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
- **Dependencies**: Use `reqwest` for HTTP, `chromiumoxide` for browser automation, `lopdf` for PDF manipulation