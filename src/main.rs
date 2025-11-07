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

    let sitemap_links = get_sitemap_url(url).await?;
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
    //     sitemap_links[0..3.min(sitemap_links.len())]
    //         .iter()
    //         .collect()
    // };
    println!("Sitemap links {:#?}", sitemap_links);

    // 🧭 1. Start browser
    let config = BrowserConfig::builder()
        .chrome_executable(browser_path)
        // .with_head()
        // .arg("--disable-web-security")
        // .arg("--disable-features=VizDisplayCompositor")
        // .arg("--disable-dev-shm-usage")
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    let (mut browser, mut handler) = Browser::launch(config).await?;

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
        println!("  ✅ Page created successfully");

        println!("  🧹 Removing unwanted elements...");
        let js_remove = r#"
            () => {
                document.querySelectorAll('.ads, .cookie, .footer').forEach(e => e.remove());
                return true;
            }
        "#;
        let _removed: bool = page.evaluate(js_remove).await?.into_value()?;
        println!("  ✅ Elements removed");

        println!("  📝 Extracting page title...");
        // Extract page title
        let title_js = r#"
            () => {
                return document.title || window.location.href;
            }
        "#;
        let title: String = page.evaluate(title_js).await?.into_value()?;
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
        // let pdf_opts = PrintToPdfParams::default();
        let pdf_opts = PrintToPdfParams {
            generate_tagged_pdf: Some(true),
            scale: Some(1.0),
            print_background: Some(false),
            ..Default::default()
        };
        let pdf_path = dir.path().join(format!("page_{:04}.pdf", i));
        // pdf_opts.generate_document_outline = Some(true);

        println!("  💾 Saving PDF to {}...", pdf_path.display());
        let save_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            page.save_pdf(pdf_opts, &pdf_path),
        )
        .await;

        match save_result {
            Ok(Ok(_)) => println!("  ✅ PDF saved successfully"),
            Ok(Err(e)) => {
                println!("  ❌ Failed to save PDF: {}", e);
                continue;
            }
            Err(_) => {
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

    let xml = reqwest::get(sitemap_url).await?.text().await?;

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
