use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct TocNode {
    pub title: Option<String>,
    pub href: String,
    pub children: Vec<TocNode>,
}

pub async fn parse_toc(url: &String) -> Result<Vec<TocNode>> {
    toc_from_sitemap(url).await
}

async fn toc_from_sitemap(url: &String) -> Result<Vec<TocNode>> {
    tracing::debug!("Fetching TOC from a sitemap for URL: {}", url);
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

    let sitemap_links: Vec<String> = if sitemap_links.is_empty() {
        println!("🚨 No sitemap links found, using direct URL");
        vec![url.to_string()]
    } else {
        sitemap_links.iter().cloned().collect()
    };

    let mut nodes: Vec<TocNode> = Vec::new();

    for href in sitemap_links {
        nodes.push(TocNode {
            href,
            title: None,
            children: vec![],
        });
    }

    Ok(nodes)
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
