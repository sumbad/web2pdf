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
    let sitemap_links: Vec<String> = sitemap_links
        .into_iter()
        .filter(|url| !sitemap_blacklist.iter().any(|bad| url.contains(bad)))
        .collect();
    let sitemap_links = &sitemap_links[0..2];
    println!("Sitemap links {:#?}", sitemap_links);

    // 🧭 1. Start browser
    let config = BrowserConfig::builder()
        .chrome_executable(browser_path)
        // .with_head()
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    let (mut browser, mut handler) = Browser::launch(config).await?;

    tokio::spawn(async move {
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
    let mut pdf_files: Vec<PathBuf> = Vec::new();

    // 🌀 3. Process each page
    for (i, link) in sitemap_links.iter().enumerate() {
        println!("→ [{}/{}] Processing {}", i + 1, sitemap_links.len(), link);
        let page = browser.new_page(link).await?;
        let js_remove = r#"
            () => {
                document.querySelectorAll('.ads, .cookie, .footer').forEach(e => e.remove());
                return true;
            }
        "#;
        let _removed: bool = page.evaluate(js_remove).await?.into_value()?;

        let pdf_opts = PrintToPdfParams::default();
        let pdf_path = dir.path().join(format!("page_{:04}.pdf", i));
        // pdf_opts.generate_document_outline = Some(true);
        page.save_pdf(pdf_opts, &pdf_path).await?;
        println!(
            "Save PDF to {} - {} bytes",
            pdf_path.display(),
            std::fs::metadata(&pdf_path)?.len()
        );
        pdf_files.push(pdf_path);
    }

    browser.close().await?;

    // 🧩 4. Merge PDFs
    let output_path = PathBuf::from(output);
    println!("📚 Merging {} PDFs into {}", pdf_files.len(), output);
    merge_pdfs(pdf_files.iter(), &output_path)?;

    Ok(())
}

async fn get_sitemap_url(base_url: &String) -> Result<Vec<String>> {
    let sitemap_url = format!("{base_url}/sitemap.xml");

    let xml = reqwest::get(sitemap_url).await?.text().await?;

    let mut reader = quick_xml::Reader::from_str(&xml);
    // reader.trim_text(true);

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
