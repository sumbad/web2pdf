use anyhow::Result;
use chromiumoxide::page::Page;

use crate::adapters::{
    mdbook::detector::MdBookDetector,
    traits::{ResourceAdapter, ResourceAdapterWithDetector},
};

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
        tracing::debug!("[MdBookAdapter] FORCE_LIGHT_THEME_JS");
        page.evaluate_on_new_document(FORCE_LIGHT_THEME_JS).await?;

        Ok(())
    }
}

impl ResourceAdapterWithDetector for MdBookAdapter {
    type Detector = MdBookDetector;
}
