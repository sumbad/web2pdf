use std::fmt::Debug;

use anyhow::Result;
use chromiumoxide::Browser;
use chromiumoxide::page::Page;

#[async_trait::async_trait]
pub trait ResourceDetector: Send + Sync + Debug {
    fn detect_fast(&self, _html: &str) -> bool {
        false
    }

    async fn detect_slow(&self, _browser: &Browser, _url: &str) -> Result<bool> {
        Ok(false)
    }
}

#[async_trait::async_trait]
pub trait ResourceAdapter: Send + Sync + Debug + 'static {
    async fn before_page(&self, _page: &Page) -> Result<()> {
        Ok(())
    }
    async fn after_page(&self, _page: &Page) -> Result<()> {
        Ok(())
    }
}

pub trait ResourceAdapterWithDetector: ResourceAdapter + Default {
    type Detector: ResourceDetector + Default + 'static;
}
