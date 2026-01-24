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

pub async fn get_sitemap_url(base_url: &String) -> Result<Vec<String>> {
    let sitemap_url = format!("{base_url}/sitemap.xml");
    tracing::debug!("Fetching sitemap from: {}", sitemap_url);

    let response = reqwest::get(&sitemap_url).await?;
    tracing::debug!("Sitemap response status: {}", response.status());

    let xml = response.text().await?;
    tracing::debug!("Sitemap XML length: {} bytes", xml.len());

    let mut reader = quick_xml::Reader::from_str(&xml);

    let mut buf = Vec::new();
    let mut links = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            quick_xml::events::Event::Start(e) if e.name().as_ref() == b"loc" => {
                if let Ok(quick_xml::events::Event::Text(t)) = reader.read_event_into(&mut buf) {
                    let url = t.decode()?;
                    links.push(url.into_owned());
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(links)
}

pub fn extract_chapter_number(url: &str) -> u32 {
    use std::str::FromStr;

    // Extract number from the last path segment
    if let Some(segment) = url.split('/').next_back() {
        // Find digits at the end of the segment
        if let Some(digit_start) = segment.find(|c: char| c.is_ascii_digit()) {
            let digit_end = if let Some(digit_end) = segment.rfind(|c: char| c.is_ascii_digit()) {
                digit_end
            } else {
                segment.len()
            };

            let digits = &segment[digit_start..=digit_end];
            let number = u32::from_str(digits).unwrap_or(0);
            return number;
        }
    }
    0
}

/// Try to find browser binary.
/// 1. Checks PATH (chromium, google-chrome, chrome).
/// 2. Checks standard paths for macOS and Windows and Linux.
///
/// Returns path to binary or error.
pub fn find_browser() -> Result<String> {
    for candidate in ["chromium", "google-chrome", "chrome"] {
        if let Ok(path) = which::which(candidate) {
            return Ok(path.to_string_lossy().to_string());
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
        for candidate in mac_paths {
            if Path::new(candidate).exists() {
                return Ok(candidate.to_string());
            }
        }
    }

    if cfg!(target_os = "linux") {
        for candidate in linux_paths {
            if Path::new(candidate).exists() {
                return Ok(candidate.to_string());
            }
        }
    }

    if cfg!(target_os = "windows") {
        for candidate in windows_paths {
            if Path::new(candidate).exists() {
                return Ok(candidate.to_string());
            }
        }

        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let chrome_path = format!(
                r"{}\Google\Chrome\Application\chrome.exe",
                app_data
            );
            if Path::new(&chrome_path).exists() {
                return Ok(chrome_path);
            }

            let chromium_path = format!(
                r"{}\Chromium\Application\chrome.exe",
                app_data
            );
            if Path::new(&chromium_path).exists() {
                return Ok(chromium_path);
            }
        }
    }

    anyhow::bail!("Chromium or Chrome not found! Please, install Chromium or Google Chrome")
}
