use scraper::Selector;

use crate::_adapter_registry::traits::ResourceDetector;

#[derive(Default, Debug)]
pub struct MdBookDetector;

#[async_trait::async_trait]
impl ResourceDetector for MdBookDetector {
    fn detect_fast(&self, html: &str) -> bool {
        let doc = scraper::Html::parse_document(html);

        let meta = Selector::parse(r#"meta[name="generator"]"#).unwrap();

        if let Some(el) = doc.select(&meta).next() {
            if let Some(c) = el.value().attr("content") {
                return c.to_lowercase().contains("mdbook");
            }
        }

        let mut score = 0;

        // TOC structure
        if doc
            .select(&Selector::parse("ul.chapter").unwrap())
            .next()
            .is_some()
        {
            score += 2;
        }

        if doc
            .select(&Selector::parse("li.chapter-item").unwrap())
            .next()
            .is_some()
        {
            score += 2;
        }

        // Main content
        if doc
            .select(&Selector::parse("main#content, #content").unwrap())
            .next()
            .is_some()
        {
            score += 1;
        }

        // Scripts
        if html.contains("book.js") {
            score += 3;
        }

        if html.contains("elasticlunr") {
            score += 2;
        }

        // Meta
        if html.contains("mdBook") {
            score += 1;
        }

        score >= 5
    }
}
