use anyhow::Result;
use chromiumoxide::page::Page;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector},
    _adapters::_mdbook::detector::MdBookDetector,
};

const SANITATION_CODE_STYLES: &str = include_str!("../../../js/sanitation-code-style.js");
const PAGE_CLEANUP_JS: &str = include_str!("../../../js/page-cleanup.js");

const FORCE_LIGHT_THEME_JS: &str = r#"
try {
  localStorage.setItem('mdbook-theme', 'light');
  document.documentElement.setAttribute('data-theme', 'light');

  console.log('localStorage changed');
} catch (e) {
    console.error(e);
}
"#;

#[derive(Default, Debug)]
pub struct MdBookAdapter;

#[async_trait::async_trait]
impl ResourceAdapter for MdBookAdapter {
    async fn before_page(&self, page: &Page) -> Result<()> {
        tracing::info!("[MdBookAdapter] FORCE_LIGHT_THEME_JS");
        page.evaluate_on_new_document(FORCE_LIGHT_THEME_JS).await?;

        Ok(())
    }

    async fn after_page(&self, page: &Page) -> Result<()> {
        tracing::info!("[MdBookAdapter] SANITATION_STYLES");
        page.evaluate(SANITATION_CODE_STYLES).await?;

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

        Ok(())
    }
}

impl ResourceAdapterWithDetector for MdBookAdapter {
    type Detector = MdBookDetector;
}
