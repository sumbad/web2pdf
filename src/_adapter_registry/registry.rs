use chromiumoxide::Browser;

use crate::{
    _adapter_registry::traits::{ResourceAdapter, ResourceAdapterWithDetector, ResourceDetector},
    _adapters::default::DefaultAdapter,
};

#[derive(Debug)]
pub struct AdapterEntry {
    detector: Box<dyn ResourceDetector>,
    adapter: Box<dyn ResourceAdapter>,
}

impl AdapterEntry {
    pub fn new<A: ResourceAdapterWithDetector>() -> Self {
        Self {
            detector: Box::new(A::Detector::default()),
            adapter: Box::new(A::default()),
        }
    }
}

#[derive(Debug)]
pub struct AdapterRegistry {
    entries: Vec<AdapterEntry>,
    fallback_adapter: Box<dyn ResourceAdapter>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            fallback_adapter: Box::new(DefaultAdapter),
        }
    }

    pub fn register<A: ResourceAdapterWithDetector>(&mut self) {
        self.entries.push(AdapterEntry::new::<A>());
    }

    pub async fn detect(&self, html: &str, browser: &Browser, url: &str) -> &dyn ResourceAdapter {
        // FAST
        for entry in &self.entries {
            if entry.detector.detect_fast(html) {
                return entry.adapter.as_ref();
            }
        }

        // SLOW
        for entry in &self.entries {
            if let Ok(true) = entry.detector.detect_slow(browser, url).await {
                return entry.adapter.as_ref();
            }
        }

        // FALLBACK
        self.fallback_adapter.as_ref()
    }
}
