use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use futures::StreamExt;

use std::env;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

mod pdf_utils;
use pdf_utils::merge_pdfs;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <URL> <output.pdf>", args[0]);
        std::process::exit(1);
    }
    let url = &args[1];
    let output = &args[2];

    let browser_path = find_browser().context("Browser not found!")?;
    println!("Use browser: {}", browser_path);

    tracing::debug!("Fetching sitemap for URL: {}", url);
    let sitemap_links = get_sitemap_url(url).await?;
    tracing::info!("Found {} sitemap links", sitemap_links.len());
    let sitemap_blacklist = ["subscribe", "errata", "colophon"];
    let mut sitemap_links: Vec<String> = sitemap_links
        .into_iter()
        .filter(|url| !sitemap_blacklist.iter().any(|bad| url.contains(bad)))
        .collect();
    sitemap_links.sort_by(|a, b| {
        let num_a = extract_chapter_number(a);
        let num_b = extract_chapter_number(b);
        num_a.cmp(&num_b)
    });
    let sitemap_links: Vec<&String> = if sitemap_links.is_empty() {
        println!("  ❌ No sitemap links found, using direct URL");
        vec![&url]
    } else {
        sitemap_links.iter().collect()
    };
    // DEBUG: add cft for tests
    // {
    //     // -----------------------------
    //     // ENABLE DETAILED LOGGING
    //     // -----------------------------
    //     unsafe {
    //         std::env::set_var("RUST_LOG", "debug");
    //         tracing_subscriber::fmt()
    //             .with_max_level(tracing::Level::DEBUG)
    //             .with_target(true)
    //             .with_thread_ids(true)
    //             .with_file(true)
    //             .with_line_number(true)
    //             .init();
    //     }
    //
    //     tracing::debug!("Logging initialized successfully");
    //     sitemap_links[2..3.min(sitemap_links.len())]
    //         .iter()
    //         .collect()
    // };
    println!("Sitemap links {:#?}", sitemap_links);

    // 🧭 1. Start browser
    tracing::debug!("Configuring browser with path: {}", browser_path);
    let config = BrowserConfig::builder()
        .chrome_executable(browser_path)
        // .with_head()
        .arg("--disable-web-security")
        .arg("--disable-features=VizDisplayCompositor")
        .arg("--disable-font-subpixel-positioning")
        .arg("--export-tagged-pdf")
        .arg("--force-renderer-accessibility")
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-gpu")
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
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    tracing::debug!("Browser configuration created, launching...");
    tracing::debug!("Launching browser...");
    let (mut browser, mut handler) = Browser::launch(config).await?;
    tracing::debug!("Browser launched successfully");

    let handle = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            match event {
                Ok(_) => {}
                Err(e) => {
                    // @see: https://github.com/mattsse/chromiumoxide/issues/266
                    let msg = format!("{:?}", e);
                    if msg.contains("data did not match any variant of untagged enum Message") {
                        // eprintln!("⚠️ Parsing error: {}", e);
                        continue;
                    } else {
                        eprintln!("Handler error: {:?}", e);
                        break;
                    }
                }
            }
        }
    });

    // 📂 2. Temporary folder for individual PDFs
    let dir = tempdir()?;
    let mut pdf_files: Vec<(PathBuf, String)> = Vec::new();

    // 🌀 3. Process each page
    for (i, link) in sitemap_links.iter().enumerate() {
        println!("→ [{}/{}] Processing {}", i + 1, sitemap_links.len(), link);

        println!("  🌐 Creating new page...");

        let page = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            browser.new_page((*link).clone()),
        )
        .await;

        let page = match page {
            Ok(p) => p,
            Err(_) => {
                println!("  ❌ Timeout creating page after 30 seconds");
                continue;
            }
        };

        let page = match page {
            Ok(p) => p,
            Err(e) => {
                println!("  ❌ Failed to create page: {}", e);
                continue;
            }
        };

        // Wait for document to be ready
        let wait_js = r#"
            () => {
                return new Promise((resolve) => {
                    if (document.readyState === 'complete') {
                        resolve(true);
                    } else {
                        window.addEventListener('load', () => resolve(true));
                        setTimeout(() => resolve(false), 5000); // fallback after 5s
                    }
                });
            }
        "#;

        let wait_result: bool = page.evaluate(wait_js).await?.into_value()?;
        if wait_result {
            tracing::debug!("Page document is ready");
        } else {
            tracing::warn!("Page wait timed out, proceeding anyway");
        }

        println!("  ✅ Page created successfully");

        println!("  🧹 Clean page for screen readers...");
        let js_remove = /*js*/ r#"
            () => {
                document.querySelectorAll('.ads, .cookie, .footer, footer').forEach(e => e.remove());

                function cleanNodeText(node) {
                    if (node.nodeType === Node.TEXT_NODE) {
                        node.textContent = node.textContent
                            .replace(/[\u200B-\u200D\uFEFF]/g, '')   // zero-width
                            .replace(/\u00A0/g, ' ')                 // non-breaking space
                            .replace(/\s+/g, ' ');                   // normalize spaces
                    } else if (node.nodeType === Node.ELEMENT_NODE) {
                        node.childNodes.forEach(cleanNodeText);
                    }
                }

                document.querySelectorAll('p, div, span, li, h1, h2, h3, h4, h5, h6').forEach(el => {
                    cleanNodeText(el);
                });

                // Remove empty paragraphs
                document.querySelectorAll('p').forEach(p => {
                    if (!p.textContent.trim()) {
                        p.remove();
                    }
                });
                
                // Add CSS to prevent text breaking
                const style = document.createElement('style');
                style.innerHTML = `
                    body {
                        font-variation-settings: "wght" 400;
                        font-feature-settings: "kern" 0, "liga" 0, "calt" 0;
                    }
                    * {
                        font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif !important;
                    }
                    p, li, td, th {
                        page-break-inside: avoid;
                        break-inside: avoid;
                        orphans: 3;
                        widows: 3;
                    }
                `;
                document.head.appendChild(style);
                return true;
            }
        "#;
        tracing::debug!("Executing page cleanup script");
        let _removed: bool = page.evaluate(js_remove).await?.into_value()?;
        tracing::debug!("Page cleanup completed successfully");
        println!("  ✅ Page cleaned");

        println!("  📝 Extracting page title...");
        // Extract page title
        let title_js = r#"
            () => {
                return document.title || window.location.href;
            }
        "#;
        tracing::debug!("Executing title extraction script");
        let title: String = page.evaluate(title_js).await?.into_value()?;
        tracing::debug!("Extracted title: {}", title);
        let chapter_num = extract_chapter_number(link);
        let title = if chapter_num > 0 {
            format!("Chapter {} - {}", chapter_num, title)
        } else {
            title
        };
        println!("  ✅ Title extracted: {}", title);

        let js_add_lang = r#"
        () => {
            document.documentElement.lang = document.documentElement.lang || 'en';
            return document.documentElement.lang;
        }
        "#;

        page.evaluate(js_add_lang).await?;

        println!("  🖨️ Generating PDF...");
        tracing::debug!("Configuring PDF generation options");
        // let pdf_opts = PrintToPdfParams::default();
        let pdf_opts = PrintToPdfParams {
            generate_tagged_pdf: Some(true),
            scale: Some(1.0),
            print_background: Some(false),
            prefer_css_page_size: Some(true),
            ..Default::default()
        };
        tracing::debug!(
            "PDF options: tagged={}, scale={}, background={}, css_size={}",
            pdf_opts.generate_tagged_pdf.unwrap_or(false),
            pdf_opts.scale.unwrap_or(0.0),
            pdf_opts.print_background.unwrap_or(false),
            pdf_opts.prefer_css_page_size.unwrap_or(false)
        );
        let pdf_path = dir.path().join(format!("page_{:04}.pdf", i));
        // pdf_opts.generate_document_outline = Some(true);

        println!("  💾 Saving PDF to {}...", pdf_path.display());
        let save_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            page.save_pdf(pdf_opts, &pdf_path),
        )
        .await;

        match save_result {
            Ok(Ok(_)) => {
                tracing::debug!("PDF saved successfully to: {}", pdf_path.display());
                println!("  ✅ PDF saved successfully");
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to save PDF: {}", e);
                println!("  ❌ Failed to save PDF: {}", e);
                continue;
            }
            Err(_) => {
                tracing::error!("Timeout saving PDF after 10 seconds");
                println!("  ❌ Timeout saving PDF after 60 seconds");
                continue;
            }
        }

        match std::fs::metadata(&pdf_path) {
            Ok(metadata) => {
                println!("  📊 PDF size: {} bytes", metadata.len());
            }
            Err(e) => {
                println!("  ❌ Failed to get PDF metadata: {}", e);
                continue;
            }
        }

        pdf_files.push((pdf_path, title));
        println!("  ✅ Page processing complete\n");
    }

    browser.close().await?;
    handle.await?;

    // 🧩 4. Merge PDFs
    let output_path = PathBuf::from(output);
    println!("📚 Merging {} PDFs into {}", pdf_files.len(), output);
    merge_pdfs(pdf_files, output_path)?;

    Ok(())
}

async fn get_sitemap_url(base_url: &String) -> Result<Vec<String>> {
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

fn extract_chapter_number(url: &str) -> u32 {
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
/// 1. Checks PATH (chromium, google-chrome).
/// 2. Checks standard paths for macOS.
///
/// Returns path to binary or error.
fn find_browser() -> Result<String> {
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
