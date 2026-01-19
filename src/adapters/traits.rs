use std::fmt::Debug;

use anyhow::Result;
use chromiumoxide::Browser;
use chromiumoxide::page::Page;

// pub trait ResourceDetector: Send + Sync {
//     fn detect_fast(&self, url: &str, html: &str) -> Option<bool>;
//
//     async fn detect_slow(&self, url: &str, browser: &Browser) -> anyhow::Result<bool>;
// }

#[async_trait::async_trait]
pub trait ResourceDetector: Send + Sync + Debug {
    /// Быстрый детект — только HTML
    fn detect_fast(&self, html: &str) -> bool {
        false
    }

    /// Медленный детект — с браузером
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

