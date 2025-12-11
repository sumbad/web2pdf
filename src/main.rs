use anyhow::{Context, Result};
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use futures::StreamExt;

use std::env;
use std::path::PathBuf;
use tempfile::tempdir;

mod pdf_utils;
use pdf_utils::merge_pdfs;

use crate::browser_utils::{
    build_browser_config, extract_chapter_number, find_browser, get_sitemap_url,
};

mod browser_utils;

// JavaScript scripts
const PAGE_WAIT_JS: &str = include_str!("../js/page-wait.js");
const PAGE_CLEANUP_JS: &str = include_str!("../js/page-cleanup.js");
const TITLE_EXTRACT_JS: &str = include_str!("../js/title-extract.js");
const LANG_SET_JS: &str = include_str!("../js/lang-set.js");
const ICONIFY_ICON: &str = include_str!("../js/iconify-icon.js");

const LOAD_PAGE_TIMEOUT_SEC: u64 = 30;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} [--debug] <URL> <output.pdf>", args[0]);
        std::process::exit(1);
    }

    let mut arg_idx = 1;
    let debug_mode = args.get(arg_idx) == Some(&"--debug".to_string());
    if debug_mode {
        arg_idx += 1;
    }

    let url = &args[arg_idx];
    let output = &args[arg_idx + 1];

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
    // Initialize logging based on debug flag
    if debug_mode {
        unsafe {
            std::env::set_var("RUST_LOG", "debug");
        }
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .init();
        println!("🐛 Debug mode enabled");
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    let sitemap_links: Vec<&String> = if sitemap_links.is_empty() {
        println!("🚨 No sitemap links found, using direct URL");
        vec![&url]
    } else {
        // Limit to first 3 links in debug mode
        if debug_mode {
            println!("🐛 Debug mode: limiting to first 3 pages");
            sitemap_links[0..3.min(sitemap_links.len())]
                .iter()
                .collect()
        } else {
            sitemap_links.iter().collect()
        }
    };

    tracing::debug!("Logging initialized successfully");
    println!("Convert URLs {:#?}", sitemap_links);

    // 🧭 1. Start browser
    tracing::debug!("Configuring browser with path: {}", browser_path);
    let config = build_browser_config(&browser_path).map_err(|e| anyhow::anyhow!(e))?;
    tracing::debug!("Browser configuration created");

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
            std::time::Duration::from_secs(LOAD_PAGE_TIMEOUT_SEC),
            browser.new_page((*link).clone()),
        )
        .await;

        let page = match page {
            Ok(p) => p,
            Err(_) => {
                println!("  ❌ Timeout creating page after {LOAD_PAGE_TIMEOUT_SEC} seconds");
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
        let wait_js = PAGE_WAIT_JS;

        // Wait for page to be ready
        let mut wait_attempts = 0;
        let max_attempts = 50; // 5 seconds total

        loop {
            let wait_result = page.evaluate_function(wait_js).await?;
            let is_ready: bool = wait_result.into_value()?;

            if is_ready {
                tracing::debug!("Page document is ready");
                break;
            } else if wait_attempts >= max_attempts {
                tracing::warn!("Page wait timed out after 5 seconds, proceeding anyway");
                break;
            } else {
                wait_attempts += 1;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        println!("  ✅ Page created successfully");

        println!("  🧹 Clean page for screen readers...");
        let js_remove_result = page.evaluate_function(PAGE_CLEANUP_JS).await?;
        tracing::debug!("Executing page cleanup result {js_remove_result:?}");
        match js_remove_result.into_value::<bool>() {
            Ok(d) => {
                tracing::debug!("Page cleanup completed successfully, {d}");
                println!("  ✅ Page cleaned");
            }
            Err(e) => {
                tracing::warn!("Failed to parse cleanup result: {:?}, but continuing", e);
                println!("  🚨 Page cleaned (with warnings)");
            }
        }

        println!("  📝 Extracting page title...");
        // Extract page title
        let title_js = TITLE_EXTRACT_JS;
        tracing::debug!("Executing title extraction script");
        let title = match page
            .evaluate_function(title_js)
            .await?
            .into_value::<String>()
        {
            Ok(title) => title,
            Err(_) => {
                tracing::warn!("Failed to extract title, using URL fallback");
                link.to_string()
            }
        };
        tracing::debug!("Extracted title: {}", title);
        let chapter_num = extract_chapter_number(link);
        let title = if chapter_num > 0 {
            format!("Chapter {} - {}", chapter_num, title)
        } else {
            title
        };
        println!("  ✅ Title extracted: {}", title);

        let js_add_lang = LANG_SET_JS;

        page.evaluate(js_add_lang).await?;

        page.evaluate(ICONIFY_ICON).await?;

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

        // tokio::time::sleep(Duration::from_millis(10000)).await;
    }

    browser.close().await?;
    handle.await?;

    // 🧩 4. Merge PDFs
    let output_path = PathBuf::from(output);
    println!("📚 Merging {} PDFs into {}", pdf_files.len(), output);
    merge_pdfs(pdf_files, output_path)?;

    Ok(())
}
