use anyhow::Result;
use chromiumoxide::page::Page;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector},
    _adapters::_mdbook::detector::MdBookDetector,
};

const MDBOOK_SANITATION: &str = include_str!("../../../js/mdbook-sanitation.js");

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
        tracing::info!("[MdBookAdapter] MDBOOK_SANITATION");
        match page.evaluate(MDBOOK_SANITATION).await?.into_value::<bool>() {
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

impl ResourceAdapterWithDetector for MdBookAdapter {
    type Detector = MdBookDetector;
}
