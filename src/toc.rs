use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};
use url::Url;

#[derive(Debug, Clone)]
pub struct TocNode {
    pub title: Option<String>,
    pub href: String,
    pub level: u8,
}

// TODO: move to adapters

///
/// Generate Table of contents by some URL
/// It will find a sitemap if it is or parse a navbar, sidebar, etc.
///
pub async fn generate_toc(url: &String) -> Result<Vec<TocNode>> {
    // Try to use sitemap for TOC
    if let Some(t) = toc_from_sitemap(url).await? {
        return Ok(t);
    }

    // Try to use navbar for TOC
    if let Some(t) = toc_from_navbar(url).await? {
        return Ok(t);
    }

    Ok(vec![TocNode {
        title: None,
        href: url.to_string(),
        level: 0,
    }])
}

async fn toc_from_navbar(url: &String) -> Result<Option<Vec<TocNode>>> {
    let html = reqwest::get(url).await?.text().await?;

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
            href,
            title: None,
            level: 0,
        });
    }

    Ok(Some(nodes))
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
