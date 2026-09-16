use anyhow::Result;
use chromiumoxide::cdp::browser_protocol::network::{EnableParams as NetworkEnableParams, Headers, SetExtraHttpHeadersParams};
use chromiumoxide::cdp::js_protocol::runtime::{
    ConsoleApiCalledType, EnableParams, EventConsoleApiCalled,
};
use chromiumoxide::{Page, browser::BrowserConfig};
use futures::StreamExt;
use std::path::Path;

use crate::auth::profile_dir;

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Neutral headers that real browsers send identically with every request
/// (documents, subresources, XHR alike). Navigation-specific headers
/// (Accept, Sec-Fetch-*, Upgrade-Insecure-Requests) Chrome adds itself per
/// request; overriding them globally would corrupt subresource requests.
const EXTRA_HEADERS: &[(&str, &str)] = &[("Accept-Language", "en-US,en;q=0.9")];

pub fn build_browser_config(browser_path: &str) -> Result<BrowserConfig, String> {
    let config_builder = if let Ok(profile) = profile_dir() {
        BrowserConfig::builder().user_data_dir(&profile)
    } else {
        BrowserConfig::builder()
    };

    config_builder.chrome_executable(browser_path)
        .arg("--disable-web-security")
        .arg("--disable-features=VizDisplayCompositor")
        .arg("--disable-font-subpixel-positioning")
        .arg("--export-tagged-pdf")
        .arg("--force-renderer-accessibility")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg(format!("--user-agent={USER_AGENT}"))
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
        // .with_head()
        // wait until the page is fully loaded before printing only with head
        // .arg("--run-all-compositor-stages-before-draw")
        // .arg("--virtual-time-budget=10000")
        // .arg("--disable-gpu")
        // .arg("--headless=new")
        // .arg("about:blank")
        .build()
}

/// Try to find browser binary.
/// 1. Checks PATH (chromium, google-chrome, chrome).
/// 2. Checks standard paths for macOS and Windows and Linux.
///
/// Returns path to binary or error.
pub fn find_browser() -> Result<String> {
    tracing::debug!("Searching for browser binary...");

    for candidate in ["chromium", "google-chrome", "chrome"] {
        tracing::debug!("Checking PATH for: {}", candidate);
        match which::which(candidate) {
            Ok(path) => {
                tracing::debug!("Found browser in PATH: {}", path.display());
                return Ok(path.to_string_lossy().to_string());
            }
            Err(e) => {
                tracing::debug!("Not found in PATH: {} - {}", candidate, e);
            }
        }
    }

    let mac_paths = [
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ];

    let windows_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\Chromium\Application\chrome.exe",
        r"C:\Program Files (x86)\Chromium\Application\chrome.exe",
    ];

    let linux_paths = [
        "/usr/bin/chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium-browser",
        "/opt/google/chrome/google-chrome",
        "/snap/bin/chromium",
        "/snap/bin/google-chrome",
    ];

    if cfg!(target_os = "macos") {
        tracing::debug!("Checking macOS standard paths...");
        for candidate in mac_paths {
            tracing::debug!("Checking: {}", candidate);
            if Path::new(candidate).exists() {
                tracing::debug!("Found browser: {}", candidate);
                return Ok(candidate.to_string());
            }
        }
    }

    if cfg!(target_os = "linux") {
        tracing::debug!("Checking Linux standard paths...");
        for candidate in linux_paths {
            tracing::debug!("Checking: {}", candidate);
            if Path::new(candidate).exists() {
                tracing::debug!("Found browser: {}", candidate);
                return Ok(candidate.to_string());
            }
        }
    }

    if cfg!(target_os = "windows") {
        tracing::debug!("Checking Windows standard paths...");
        for candidate in windows_paths {
            tracing::debug!("Checking: {}", candidate);
            if Path::new(candidate).exists() {
                tracing::debug!("Found browser: {}", candidate);
                return Ok(candidate.to_string());
            }
        }

        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            tracing::debug!("LOCALAPPDATA found: {}", app_data);
            let chrome_path = format!(r"{}\Google\Chrome\Application\chrome.exe", app_data);
            tracing::debug!("Checking: {}", chrome_path);
            if Path::new(&chrome_path).exists() {
                tracing::debug!("Found browser: {}", chrome_path);
                return Ok(chrome_path);
            }

            let chromium_path = format!(r"{}\Chromium\Application\chrome.exe", app_data);
            tracing::debug!("Checking: {}", chromium_path);
            if Path::new(&chromium_path).exists() {
                tracing::debug!("Found browser: {}", chromium_path);
                return Ok(chromium_path);
            }
        } else {
            tracing::debug!("LOCALAPPDATA environment variable not found");
        }
    }

    anyhow::bail!(
        "Chromium or Chrome not found! \
        Platform: {} \
        Please install Google Chrome or Chromium and ensure it's accessible from PATH or standard installation paths",
        std::env::consts::OS
    )
}

/// Forward page console output (console.log/warn/error/...) to tracing.
/// Requires Runtime.enable; events are logged with target `web2pdf::console`.
pub async fn attach_console_logger(page: &Page) -> Result<()> {
    page.execute(EnableParams::default()).await?;

    let mut events = page.event_listener::<EventConsoleApiCalled>().await?;
    tokio::spawn(async move {
        while let Some(event) = events.next().await {
            let text = event
                .args
                .iter()
                .map(|arg| match arg.value.as_ref() {
                    Some(v) => v
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| v.to_string()),
                    None => arg
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("<{:?}>", arg.r#type)),
                })
                .collect::<Vec<_>>()
                .join(" ");

            match event.r#type {
                ConsoleApiCalledType::Error => {
                    tracing::error!(target: "web2pdf::console", "{text}")
                }
                ConsoleApiCalledType::Warning => {
                    tracing::warn!(target: "web2pdf::console", "{text}")
                }
                ConsoleApiCalledType::Debug => {
                    tracing::debug!(target: "web2pdf::console", "{text}")
                }
                _ => tracing::info!(target: "web2pdf::console", "{text}"),
            }
        }
    });

    Ok(())
}

/// Set extra HTTP headers that real browsers send.
/// This helps bypass CloudFront WAF and other bot detection.
pub async fn set_extra_headers(page: &Page) -> Result<()> {
    // Enable network domain
    page.execute(NetworkEnableParams::default()).await?;

    // Build headers as JSON object
    let headers_map: serde_json::Map<String, serde_json::Value> = EXTRA_HEADERS
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
        .collect();
    let headers = Headers::new(serde_json::Value::Object(headers_map));

    let params = SetExtraHttpHeadersParams { headers };
    page.execute(params).await?;

    tracing::debug!("Extra HTTP headers set for page");
    Ok(())
}
