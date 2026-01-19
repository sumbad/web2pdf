use anyhow::{Result, bail};
use chromiumoxide::Browser;

use crate::adapters::traits::{ResourceAdapter, ResourceAdapterWithDetector, ResourceDetector};

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
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn register<A: ResourceAdapterWithDetector>(&mut self) {
        self.entries.push(AdapterEntry::new::<A>());
    }

    pub async fn detect(
        &self,
        html: &str,
        browser: &Browser,
        url: &str,
    ) -> Result<&dyn ResourceAdapter> {
        // FAST
        for entry in &self.entries {
            if entry.detector.detect_fast(html) {
                return Ok(entry.adapter.as_ref());
            }
        }

        // SLOW
        for entry in &self.entries {
            if entry.detector.detect_slow(browser, url).await? {
                return Ok(entry.adapter.as_ref());
            }
        }

        bail!("No suitable adapter found")
    }
}

// impl AdapterRegistry {
//     pub fn new() -> Self {
//         Self {
//             entries: Vec::new(),
//         }
//     }
//
//     pub fn register(
//         &mut self,
//         detector: impl ResourceDetector + 'static,
//         adapter: impl ResourceAdapter + 'static,
//     ) {
//         self.entries.push(AdapterEntry {
//             detector: Box::new(detector),
//             adapter: Box::new(adapter),
//         });
//     }
//
//     pub async fn resolve<'a>(&'a self, page: &'a Page) -> Result<Option<&'a dyn ResourceAdapter>> {
//         for entry in &self.entries {
//             if entry.detector.detect(page).await? {
//                 return Ok(Some(entry.adapter.as_ref()));
//             }
//         }
//         Ok(None)
//     }
// }
