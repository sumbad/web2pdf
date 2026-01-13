use anyhow::Result;
use chromiumoxide::browser::BrowserConfig;
use std::path::Path;

pub fn build_browser_config(browser_path: &str) -> Result<BrowserConfig, String> {
    BrowserConfig::builder()
        .chrome_executable(browser_path)
        .arg("--disable-web-security")
        .arg("--disable-features=VizDisplayCompositor")
        .arg("--disable-font-subpixel-positioning")
        .arg("--export-tagged-pdf")
        .arg("--force-renderer-accessibility")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-features=ChromeWhatsNewUI,TabHoverCardImages,TabHoverCards,OmniboxOnDeviceHeadSuggestions")
        .arg("--disable-background-networking")
        .arg("--disable-renderer-backgrounding")
        .arg("--disable-client-side-phishing-detection")
        .arg("--disable-component-update")
        .arg("--disable-domain-reliability")
        .arg("--disable-default-apps")
        .arg("--disable-sync")
        .arg("--disable-ntp-most-likely-favicons-from-server")
        .arg("--disable-features=NewTabPage")
        .arg("--homepage=about:blank")
        .arg("--new-window")
        .arg("about:blank")
        // .with_head()
        // wait until the page is fully loaded before printing only with head
        // .arg("--run-all-compositor-stages-before-draw")
        // .arg("--virtual-time-budget=10000")
        // .arg("--disable-gpu")
        // .arg("--headless=new")
        //
        .build()
}

/// Try to find browser binary.
/// 1. Checks PATH (chromium, google-chrome).
/// 2. Checks standard paths for macOS.
///
/// Returns path to binary or error.
pub fn find_browser() -> Result<String> {
    for candidate in ["chromium", "google-chrome"] {
        if let Ok(path) = which::which(candidate) {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    let mac_paths = [
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ];

    for candidate in mac_paths {
        if Path::new(candidate).exists() {
            return Ok(candidate.to_string());
        }
    }

    anyhow::bail!("Chromium or Chrome not found! Please, install Chromium")
}
