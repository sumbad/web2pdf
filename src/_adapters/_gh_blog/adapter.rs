use anyhow::Result;
use chromiumoxide::page::Page;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector},
    _adapters::_gh_blog::detector::GHBlogDetector,
};

const SANITATION: &str = include_str!("../../../js/ghblog-sanitation.js");

#[derive(Default, Debug)]
pub struct GHBlogAdapter;

#[async_trait::async_trait]
impl ResourceAdapter for GHBlogAdapter {
    async fn after_page(&self, page: &Page) -> Result<()> {
        tracing::info!("[GHBlogAdapter] SANITATION");
        match page.evaluate(SANITATION).await?.into_value::<bool>() {
            Ok(d) => {
                tracing::debug!("✅ Page script completed successfully, {d}");
            }
            Err(e) => {
                tracing::warn!("🚨 Failed to parse cleanup result: {:?}, but continuing", e);
            }
        };

        Ok(())
    }
}

impl ResourceAdapterWithDetector for GHBlogAdapter {
    type Detector = GHBlogDetector;
}
