use anyhow::Result;
use chromiumoxide::page::Page;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector},
    _adapters::_mdbook::detector::MdBookDetector,
};

const SANITATION_CODE_STYLES: &str = include_str!("../../../js/sanitation-code-style.js");

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

        Ok(())
    }
}

impl ResourceAdapterWithDetector for MdBookAdapter {
    type Detector = MdBookDetector;
}
