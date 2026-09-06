# Web2PDF

English | [Русский](./README_RU.md)

A command-line utility for converting websites to a PDF document. It fetches a site via sitemap, converts each page to PDF using headless Chrome, and merges all pages into one file.

## Features

- 🌐 Automatic page discovery via sitemap.xml
- 🖨️ HTML to PDF conversion using Chromium/Chrome
- 📚 Merge multiple PDFs into one document with bookmarks
- 🧹 Remove unwanted elements (ads, cookie notices, footers)
- 🧩 Platform-specific adapters (e.g., for mdBook documentation)
- 🎨 Forced light theme for documentation sites
- 📋 Page `console.*` output forwarded to tool logs
- 🔧 Cross-platform support (macOS, Linux, Windows)

## Installation

### Requirements

- Chromium or Google Chrome

### Using installers

```bash
# Linux / macOS (via install.sh)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/sumbad/web2pdf/releases/latest/download/web2pdf-installer.sh | sh
```

```bash
# Windows (via install.ps1)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/sumbad/web2pdf/releases/latest/download/web2pdf-installer.ps1 | iex"
```

### Download prebuilt binaries

Prebuilt binaries are available from the [GitHub Releases](https://github.com/sumbad/web2pdf/releases) page for:
- macOS (Intel and Apple Silicon)
- Linux (x86_64)
- Windows (x86_64)

### Using cargo install (Rust users)

```bash
cargo install --git https://github.com/sumbad/web2pdf.git web2pdf
```


## Usage

### Basic syntax

```bash
web2pdf [--debug] <URL> [OUTPUT]
```

### Options

- `--login`, `-l` - Run authorization (session data is saved to a local browser profile)
- `--debug`, `-d` - Enable debug mode with verbose logging (limits pages to 3 in debug builds, dumps source page HTML)
- `--help`, `-h` - Display help information
- `--version`, `-V` - Display version information

### Authorization

For sites behind authentication, log in first:

```bash
web2pdf --login https://example.com
```

A browser window opens — complete the login and press Enter in the terminal. The session is saved to a local profile (`~/web2pdf/chrome-profile`) and automatically reused for subsequent conversions.

### Examples

```bash
# Convert website to PDF (output defaults to output.pdf)
web2pdf https://example.com

# Specify custom output file
web2pdf https://example.com site.pdf

# Authenticated sites: perform a one-time login
web2pdf --login https://example.com

# Then convert as usual — the profile is reused
web2pdf https://example.com

# Enable debug mode
web2pdf --debug https://example.com
```

### How it works

1. **Browser detection** - Finds Chromium/Chrome in PATH or standard paths
2. **Auth profile** - If a session was configured via `--login`, pages open with saved cookies
3. **Sitemap fetching** - Loads sitemap.xml from the specified URL
4. **Page filtering** - Excludes unwanted pages (subscribe, errata, colophon)
5. **PDF conversion** - Creates PDF for each page via headless browser, waiting for content to fully render
6. **Merging** - Combines all PDF files into one document with bookmarks

## Development

### Project structure

```
src/
├── main.rs             # Main application logic with CLI parsing
├── auth.rs             # Authorization and browser profile
├── browser_utils.rs    # Browser configuration and detection
├── toc.rs              # Table of Contents generation
├── _pdf_utils/         # PDF manipulation utilities
│   ├── merge_pdfs.rs   # PDF merging implementation
│   ├── sanitize_pdf.rs # PDF/UA structure sanitization
│   └── helpers.rs      # Structure helper functions
├── _adapters/          # Content adapters for different formats
│   └── _mdbook/        # MdBook documentation format adapter
└── _adapter_registry/  # Registry system for adapter detection
js/
├── flatten-shadow-dom.js # Shadow DOM handling
├── iconify-icon.js       # Iconify icon handling
├── lang-set.js           # Language setting
├── mdbook-sanitation.js  # mdBook page preparation
├── page-cleanup.js       # Page cleanup
├── page-wait.js          # Page waiting
├── prepare-habr.js       # Habr page preparation
└── title-extract.js      # Title extraction
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

- `chromiumoxide` - Headless Chrome control via CDP
- `reqwest` - HTTP client for sitemap fetching
- `lopdf` - PDF document manipulation
- `quick-xml` - XML sitemap parsing
- `tokio` - Async runtime
- `clap` - Command-line argument parsing with suggestions and colored output
- `scraper` - HTML parsing
- `tracing`/`tracing-subscriber` - Structured logging
- `anyhow` - Error handling

## Limitations

- Some JavaScript-heavy sites may not render correctly
- Authenticated sites require a one-time login via `--login`
- Heavy SPA pages take longer to convert: the tool waits for content to render before printing

## License

MIT License
