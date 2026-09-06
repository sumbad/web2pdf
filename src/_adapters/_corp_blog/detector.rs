use scraper::Selector;

use crate::_adapter_registry::traits::ResourceDetector;

#[derive(Default, Debug)]
pub struct CorpBlogDetector;

#[async_trait::async_trait]
impl ResourceDetector for CorpBlogDetector {
    fn detect_fast(&self, html: &str) -> bool {
        let doc = scraper::Html::parse_document(html);

        // body structure
        if !doc
            .select(&Selector::parse("body #wrapper #content").unwrap())
            .next()
            .is_some()
        {
            return false;
        }

        // special elements
        if !doc
            .select(&Selector::parse("#intdev-shared-menu").unwrap())
            .next()
            .is_some()
        {
            return false;
        }

        html.contains("blog_entry_ics")
    }
}
