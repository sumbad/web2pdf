use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::browser_utils::USER_AGENT;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("reqwest client should build")
});

#[derive(Debug, Clone)]
pub struct TocNode {
    pub file_path: Option<PathBuf>,
    pub title: Option<String>,
    pub href: String,
    pub level: u8,
}

// TODO: move to adapters

///
/// Generate Table of contents by some URL
/// It will find a sitemap if it is or parse a navbar, sidebar, etc.
///
pub async fn generate_toc(url: &String) -> Vec<TocNode> {
    // Try to use sitemap for TOC
    if let Some(t) = toc_from_sitemap(url).await.unwrap_or_else(|e| {
        tracing::debug!("TOC from sitemap failed, trying navbar: {e:#}");
        None
    }) {
        return t;
    }

    // Try to use navbar for TOC
    if let Some(t) = toc_from_navbar(url).await.unwrap_or_else(|e| {
        tracing::debug!("TOC from navbar failed, falling back to entry page: {e:#}");
        None
    }) {
        return t;
    }

    vec![TocNode {
        file_path: None,
        title: None,
        href: url.to_string(),
        level: 0,
    }]
}

async fn toc_from_navbar(url: &String) -> Result<Option<Vec<TocNode>>> {
    let html = HTTP_CLIENT.get(url).send().await?.text().await?;

    let base_url = Url::parse(url)?;

    // TODO: support different navbars

    match parse_mdbook_toc(&html, &base_url) {
        Ok(t) => Ok(Some(t)),
        Err(e) => {
            println!("{:?}", e);
            Ok(None)
        }
    }
}

fn parse_mdbook_toc(html: &str, base_url: &Url) -> Result<Vec<TocNode>> {
    let document = Html::parse_document(html);

    let sidebar_selector = Selector::parse("nav#sidebar ol.chapter").expect("valid selector");

    let ol = document
        .select(&sidebar_selector)
        .next()
        .context("mdBook TOC not found: nav#sidebar ol.chapter")?;

    let mut nodes: Vec<TocNode> = Vec::new();

    parse_ol(&mut nodes, ol, base_url, 0)?;

    Ok(nodes)
}

fn parse_ol(nodes: &mut Vec<TocNode>, ol: ElementRef, base_url: &Url, level: u8) -> Result<()> {
    let li_selector = Selector::parse(":scope > li").expect("valid selector");

    for li in ol.select(&li_selector) {
        parse_li(nodes, li, base_url, level)?;
    }

    Ok(())
}

fn parse_li(nodes: &mut Vec<TocNode>, li: ElementRef, base_url: &Url, level: u8) -> Result<()> {
    let a_selector = Selector::parse(":scope > a").expect("valid selector");
    let ol_selector = Selector::parse(":scope > ol").expect("valid selector");

    let a = li.select(&a_selector).next();
    let ol = li.select(&ol_selector).next();

    if let Some(ol_el) = ol {
        return parse_ol(nodes, ol_el, base_url, level + 1);
    }

    if let Some(a_el) = a {
        let title = a_el.text().collect::<String>().trim().to_string();

        let href_raw = a_el.value().attr("href").context("TOC link without href")?;

        // mdbook uses relative links
        let href = base_url
            .join(href_raw)
            .context("invalid TOC href")?
            .to_string();

        nodes.push(TocNode {
            file_path: None,
            title: Some(title),
            href,
            level,
        })
    }

    Ok(())
}

async fn toc_from_sitemap(url: &String) -> Result<Option<Vec<TocNode>>> {
    tracing::debug!("Fetching TOC from a sitemap for URL: {}", url);
    let sitemap_links = get_sitemap_url(url).await?;

    if sitemap_links.is_empty() {
        println!("No sitemap links found");
        return Ok(None);
    }

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

    let mut nodes: Vec<TocNode> = Vec::new();

    for href in sitemap_links {
        nodes.push(TocNode {
            file_path: None,
            title: None,
            href,
            level: 0,
        });
    }

    Ok(Some(nodes))
}

async fn get_sitemap_url(base_url: &str) -> Result<Vec<String>> {
    let base = base_url.trim_end_matches('/');
    let sitemap_url = format!("{base}/sitemap.xml");
    tracing::debug!("Fetching sitemap from: {}", sitemap_url);

    let response = HTTP_CLIENT.get(&sitemap_url).send().await?;
    tracing::debug!("Sitemap response status: {}", response.status());

    let xml = response.text().await?;
    tracing::debug!("Sitemap XML length: {} bytes", xml.len());

    let mut reader = quick_xml::Reader::from_str(&xml);

    let mut buf = Vec::new();
    let mut links = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            quick_xml::events::Event::Start(e) if e.name().as_ref() == "loc" => {
                if let Ok(quick_xml::events::Event::Text(t)) = reader.read_event_into(&mut buf) {
                    let url = t.xml10_content();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_chapter_number_from_last_segment() {
        assert_eq!(
            extract_chapter_number("https://example.com/book/chapter-01.html"),
            1
        );
        assert_eq!(
            extract_chapter_number("https://example.com/book/page-12.html"),
            12
        );
        assert_eq!(extract_chapter_number("https://example.com/2.html"), 2);
    }

    #[test]
    fn returns_zero_without_number_in_last_segment() {
        assert_eq!(extract_chapter_number("https://example.com/index.html"), 0);
        assert_eq!(
            extract_chapter_number("https://example.com/book/chapter-1/index.html"),
            0
        );
        // "2-3" is not a valid number
        assert_eq!(extract_chapter_number("https://example.com/ch-2-3.html"), 0);
        assert_eq!(extract_chapter_number("https://example.com/"), 0);
    }

    #[test]
    fn parses_mdbook_sidebar_into_hierarchy() -> anyhow::Result<()> {
        let html = r#"
        <html><body>
        <nav id="sidebar">
          <ol class="chapter">
            <li><a href="intro.html">Introduction</a></li>
            <li><a href="ch1.html">Chapter 1</a>
              <ol>
                <li><a href="sec-1-1.html">Section 1.1</a></li>
                <li><a href="sec-1-2.html">Section 1.2</a></li>
              </ol>
            </li>
          </ol>
        </nav>
        </body></html>"#;

        let base = Url::parse("https://example.com/book/")?;
        let nodes = parse_mdbook_toc(html, &base)?;

        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].title.as_deref(), Some("Introduction"));
        assert_eq!(nodes[0].href, "https://example.com/book/intro.html");
        assert_eq!(nodes[0].level, 0);
        // Parent items with sub-lists are replaced by their children
        assert_eq!(nodes[1].title.as_deref(), Some("Section 1.1"));
        assert_eq!(nodes[1].level, 1);
        assert_eq!(nodes[2].href, "https://example.com/book/sec-1-2.html");

        Ok(())
    }

    #[test]
    fn missing_sidebar_is_an_error() {
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_mdbook_toc("<html><body></body></html>", &base);
        assert!(result.is_err());
    }
}
