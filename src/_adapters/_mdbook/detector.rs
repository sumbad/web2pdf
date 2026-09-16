use scraper::Selector;

use crate::_adapter_registry::traits::ResourceDetector;

#[derive(Default, Debug)]
pub struct MdBookDetector;

#[async_trait::async_trait]
impl ResourceDetector for MdBookDetector {
    fn detect_fast(&self, html: &str) -> bool {
        let doc = scraper::Html::parse_document(html);

        let meta = Selector::parse(r#"meta[name="generator"]"#).unwrap();

        if let Some(el) = doc.select(&meta).next()
            && let Some(c) = el.value().attr("content")
        {
            return c.to_lowercase().contains("mdbook");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_generator_meta() {
        let html = r#"<html><head><meta name="generator" content="mdBook 0.4.40"></head><body></body></html>"#;
        assert!(MdBookDetector.detect_fast(html));
    }

    #[test]
    fn detects_by_heuristic_score() {
        // ul.chapter(2) + li.chapter-item(2) + #content(1) + book.js(3) = 8 >= 5
        let html = r#"
        <html><body>
        <ul class="chapter"><li class="chapter-item"><a href="a.html">A</a></li></ul>
        <main id="content"></main>
        <script src="book.js"></script>
        </body></html>"#;
        assert!(MdBookDetector.detect_fast(html));
    }

    #[test]
    fn rejects_unrelated_page() {
        let html = r#"<html><body><article>Just a blog post</article></body></html>"#;
        assert!(!MdBookDetector.detect_fast(html));
        // A single weak signal stays below the threshold
        let weak = r#"<html><body><ul class="chapter"></ul></body></html>"#;
        assert!(!MdBookDetector.detect_fast(weak));
    }
}
