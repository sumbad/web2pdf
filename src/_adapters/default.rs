use anyhow::{Result};

use chromiumoxide::Page;

use crate::_adapter_registry::traits::ResourceAdapter;

const PAGE_CLEANUP_JS: &str = include_str!("../../js/page-cleanup.js");
const LANG_SET_JS: &str = include_str!("../../js/lang-set.js");
const ICONIFY_ICON: &str = include_str!("../../js/iconify-icon.js");

#[derive(Default, Debug)]
pub struct DefaultAdapter;

#[async_trait::async_trait]
impl ResourceAdapter for DefaultAdapter {
    async fn after_page(&self, page: &Page) -> Result<()> {
        println!("  🧹 Clean page for screen readers...");
        let js_remove_result = page.evaluate_function(PAGE_CLEANUP_JS).await?;
        tracing::debug!("Executing page cleanup result {js_remove_result:?}");
        match js_remove_result.into_value::<bool>() {
            Ok(d) => {
                tracing::debug!("Page cleanup completed successfully, {d}");
                println!("  ✅ Page cleaned");
            }
            Err(e) => {
                tracing::warn!("Failed to parse cleanup result: {:?}, but continuing", e);
                println!("  🚨 Page cleaned (with warnings)");
            }
        }

        page.evaluate(LANG_SET_JS).await?;

        page.evaluate(ICONIFY_ICON).await?;

        Ok(())
    }
}
