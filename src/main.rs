use anyhow::{Context, Result};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use chromiumoxide::{browser::Browser, page::MediaTypeParams};
use futures::StreamExt;

use std::env;
use std::path::PathBuf;
use tempfile::{TempDir, tempdir};

mod _pdf_utils;
use _pdf_utils::merge_pdfs::merge_pdfs;

mod browser_utils;
use crate::_adapter_registry::traits::ResourceAdapter;
use crate::browser_utils::{build_browser_config, find_browser};
use crate::toc::TocNode;

mod toc;

mod _adapter_registry;
use _adapter_registry::registry::AdapterRegistry;

mod _adapters;
use _adapters::_mdbook::adapter::MdBookAdapter;

// JavaScript scripts
const PAGE_WAIT_JS: &str = include_str!("../js/page-wait.js");
const TITLE_EXTRACT_JS: &str = include_str!("../js/title-extract.js");
const PREPARE_HABR: &str = include_str!("../js/prepare-habr.js");

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

    let mut toc = toc::generate_toc(url).await?;

    // Limit in debug mode
    toc = if debug_mode {
        println!("🐛 Debug mode: limiting to first 3 pages");
        toc[0..3.min(toc.len())].iter().cloned().collect()
    } else {
        toc
    };

    println!("TOC {:#?}", toc);

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

    let html = reqwest::get(url).await?.text().await?;

    tracing::debug!("Register adapters");
    let mut registry = AdapterRegistry::new();
    registry.register::<MdBookAdapter>();
    let adapter = registry.detect(&html, &browser, url).await;
    tracing::debug!("Detected adapter {:?}", adapter);

    // 📂 2. Temporary folder for individual PDFs
    let dir = tempdir()?;

    let toc_len = toc.len();
    // 🌀 3. Process each page
    for (i, node) in toc.iter_mut().enumerate() {
        println!(
            "→ [{}/{}] Processing {}",
            i + 1,
            toc_len,
            node.href
        );

        process_page(i, node, &browser, &dir, adapter).await?;
    }

    browser.close().await?;
    handle.await?;

    // 🧩 4. Merge PDFs
    let output_path = PathBuf::from(output);
    println!("📚 Merging {} PDFs into {}", toc.len(), output);
    merge_pdfs(toc, output_path)?;

    Ok(())
}

///
/// Processing a web page
///
async fn process_page(
    index: usize,
    node: &mut TocNode,
    browser: &Browser,
    dir: &TempDir,
    adapter: &dyn ResourceAdapter,
) -> Result<()> {
    println!("  🌐 Creating new page...");

    let page = browser.new_page("about:blank").await?;

    adapter.before_page(&page).await?;

    let link = &node.href;

    let page = tokio::time::timeout(
        std::time::Duration::from_secs(LOAD_PAGE_TIMEOUT_SEC),
        page.goto(link),
    )
    .await;

    let page = match page {
        Ok(p) => p,
        Err(_) => {
            println!("  ❌ Timeout creating page after {LOAD_PAGE_TIMEOUT_SEC} seconds");
            return Ok(());
        }
    };

    let page = match page {
        Ok(p) => p,
        Err(e) => {
            println!("  ❌ Failed to create page: {}", e);
            return Ok(());
        }
    };

    page.emulate_media_type(MediaTypeParams::Print).await?;

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

    adapter.after_page(page).await?;

    // TODO: collect title inside TocNode
    let title = if let Some(ref t) = node.title {
        t.clone()
    } else {
        println!("  📝 Extracting page title...");
        // Extract page title
        let title_js = TITLE_EXTRACT_JS;
        tracing::debug!("Executing title extraction script");
        let extracted_title = match page
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
        tracing::debug!("Extracted title: {}", extracted_title);

        extracted_title
    };

    let chapter_num = toc::extract_chapter_number(link);
    let title = if chapter_num > 0 {
        format!("Chapter {} - {}", chapter_num, title)
    } else {
        title
    };
    println!("  ✅ Title extracted: {}", title);

    // TODO: create HabrAdapter
    if link.starts_with("https://habr.com") {
        println!("  🏗️ : PREPARE_HABR");
        page.evaluate(PREPARE_HABR).await?;
    }

    // tokio::time::sleep(std::time::Duration::from_millis(10000)).await;

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
    let pdf_path = dir.path().join(format!("page_{:04}.pdf", index));
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
            return Ok(());
        }
        Err(_) => {
            tracing::error!("Timeout saving PDF after 10 seconds");
            println!("  ❌ Timeout saving PDF after 60 seconds");
            return Ok(());
        }
    }

    match std::fs::metadata(&pdf_path) {
        Ok(metadata) => {
            println!("  📊 PDF size: {} bytes", metadata.len());
        }
        Err(e) => {
            println!("  ❌ Failed to get PDF metadata: {}", e);
            return Ok(());
        }
    }

    node.file_path = Some(pdf_path);
    println!("  ✅ Page processing complete\n");

    Ok(())
}
