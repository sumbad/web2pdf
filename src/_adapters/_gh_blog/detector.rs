use scraper::Selector;
use url::Url;

use crate::_adapter_registry::traits::ResourceDetector;

#[derive(Default, Debug)]
pub struct GHBlogDetector;

#[async_trait::async_trait]
impl ResourceDetector for GHBlogDetector {
    fn detect_fast(&self, html: &str, url: &str) -> bool {
        let is_gh_blog = Url::parse(url)
            .ok()
            .as_ref()
            .and_then(|u| u.host_str())
            .is_some_and(|h| h.ends_with("github.blog"));

        if !is_gh_blog {
            return false;
        }

        let doc = scraper::Html::parse_document(html);

        let meta = Selector::parse(r#"meta[property="og:site_name"]"#).unwrap();

        if let Some(el) = doc.select(&meta).next()
            && let Some(c) = el.value().attr("content")
        {
            return c.to_lowercase().contains("github blog");
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.blog/2025/10/23/some-post/";

    #[test]
    fn detects_by_site_name_meta() {
        let html = r#"<html><head><meta property="og:site_name" content="The GitHub Blog"></head><body></body></html>"#;
        assert!(GHBlogDetector.detect_fast(html, URL));
    }

    #[test]
    fn site_name_match_is_case_insensitive() {
        let html = r#"<html><head><meta property="og:site_name" content="THE GITHUB BLOG"></head><body></body></html>"#;
        assert!(GHBlogDetector.detect_fast(html, URL));
    }

    #[test]
    fn rejects_other_host() {
        let html = r#"<html><head><meta property="og:site_name" content="The GitHub Blog"></head><body></body></html>"#;
        assert!(!GHBlogDetector.detect_fast(html, "https://example.com/post/"));
    }

    #[test]
    fn rejects_lookalike_hosts_and_query_params() {
        let html = r#"<html><head><meta property="og:site_name" content="The GitHub Blog"></head><body></body></html>"#;
        assert!(!GHBlogDetector.detect_fast(html, "https://notgithub.blog.example.com/"));
        assert!(!GHBlogDetector.detect_fast(html, "https://example.com/?ref=github.blog"));
    }

    #[test]
    fn rejects_invalid_url() {
        let html = r#"<html><head><meta property="og:site_name" content="The GitHub Blog"></head><body></body></html>"#;
        assert!(!GHBlogDetector.detect_fast(html, "not a url"));
    }

    #[test]
    fn rejects_without_site_name_meta() {
        let html = r#"<html><head></head><body></body></html>"#;
        assert!(!GHBlogDetector.detect_fast(html, URL));
    }

    #[test]
    fn rejects_other_site_name() {
        let html = r#"<html><head><meta property="og:site_name" content="Some Other Blog"></head><body></body></html>"#;
        assert!(!GHBlogDetector.detect_fast(html, URL));
    }
}
