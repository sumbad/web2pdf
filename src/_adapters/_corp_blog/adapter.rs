use std::time::Duration;

use anyhow::Result;
use chromiumoxide::page::Page;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector},
    _adapters::_corp_blog::detector::CorpBlogDetector,
};

const CORP_BLOG_CLEANUP: &str = include_str!("../../../js/corp_blog-cleanup.js");
const CORP_BLOG_WAIT: &str = include_str!("../../../js/corp_blog-wait.js");

/// Minimal text length inside #content that means the article is rendered
/// (skeletons/spinners give far less).
const MIN_CONTENT_TEXT_LEN: i64 = 3000;

/// 75 attempts x 200 ms = 15 s max wait for the SPA to render content.
const MAX_WAIT_ATTEMPTS: u32 = 75;
const WAIT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Default, Debug)]
pub struct CorpBlogAdapter;

#[async_trait::async_trait]
impl ResourceAdapter for CorpBlogAdapter {
    async fn after_page(&self, page: &Page) -> Result<()> {
        // Wait for the SPA to render the article into #content.
        // #content has wrapper children immediately, so waiting for
        // "has children" is useless — poll real text length instead.
        let mut text_len: i64 = -1;
        for attempt in 1..=MAX_WAIT_ATTEMPTS {
            text_len = page
                .evaluate_function(CORP_BLOG_WAIT)
                .await?
                .into_value::<i64>()
                .unwrap_or(-1);

            if text_len >= MIN_CONTENT_TEXT_LEN {
                tracing::info!(
                    "[CorpBlogAdapter] content ready after {attempt} attempts, text_len={text_len}"
                );
                break;
            }

            tokio::time::sleep(WAIT_INTERVAL).await;
        }

        if text_len < MIN_CONTENT_TEXT_LEN {
            tracing::warn!(
                "[CorpBlogAdapter] content wait timeout: text_len={text_len} after {MAX_WAIT_ATTEMPTS} attempts"
            );
        }

        match page
            .evaluate_function(CORP_BLOG_CLEANUP)
            .await?
            .into_value::<String>()
        {
            Ok(removed) => {
                tracing::info!("[CorpBlogAdapter] removed elements: {removed}");
            }
            Err(e) => {
                tracing::warn!("🚨 Failed to parse cleanup result: {e:?}, but continuing");
            }
        };

        Ok(())
    }
}

impl ResourceAdapterWithDetector for CorpBlogAdapter {
    type Detector = CorpBlogDetector;
}
