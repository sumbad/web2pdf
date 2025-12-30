# Web2PDF

English | [Русский](./README_RU.md)

A command-line utility for converting websites to a PDF document. It fetches a site via sitemap, converts each page to PDF using headless Chrome, and merges all pages into one file.

## Features

- 🌐 Automatic page discovery via sitemap.xml
- 🖨️ HTML to PDF conversion using Chromium/Chrome
- 📚 Merge multiple PDFs into one document with bookmarks
- 🧹 Remove unwanted elements (ads, cookie notices, footers)
- 🔧 Cross-platform support (macOS, Linux, Windows)

## Installation

### Requirements

- Rust 2024+
- Chromium or Google Chrome

### Build from source

```bash
git clone <repository-url>
cd web2pdf
cargo build --release
```

## Usage

### Basic syntax

```bash
cargo run -- <URL> <output.pdf>
```

### Examples

```bash
# Convert website to PDF
cargo run -- https://example.com site.pdf

# Using optimized build
./target/release/web2pdf https://example.com site.pdf
```

### How it works

1. **Browser detection** - Finds Chromium/Chrome in PATH or standard paths
2. **Sitemap fetching** - Loads sitemap.xml from the specified URL
3. **Page filtering** - Excludes unwanted pages (subscribe, errata, colophon)
4. **PDF conversion** - Creates PDF for each page via headless browser
5. **Merging** - Combines all PDF files into one document with bookmarks

## Development

### Project structure

```
src/
├── main.rs       # Main application logic
├── browser_utils.rs # Browser utilities
└── pdf_utils.rs  # PDF manipulation utilities
js/
├── flatten-shadow-dom.js # Shadow DOM handling
├── iconify-icon.js      # Iconify icon handling
├── lang-set.js          # Language setting
├── page-cleanup.js      # Page cleanup
├── page-wait.js         # Page waiting
├── prepare-habr.js      # Habr page preparation
└── title-extract.js     # Title extraction
```

### Build and testing

```bash
# Build
cargo build

# Optimized build
cargo build --release

# Run tests
cargo test

# Check code
cargo check

# Format code
cargo fmt

# Linting
cargo clippy -- -D warnings
```

### Key dependencies

- `chromiumoxide` - Headless Chrome control
- `reqwest` - HTTP client for sitemap fetching
- `lopdf` - PDF document manipulation
- `quick-xml` - XML sitemap parsing
- `tokio` - Async runtime

## Limitations

- Some JavaScript-heavy sites may not render correctly

## License

MIT License
