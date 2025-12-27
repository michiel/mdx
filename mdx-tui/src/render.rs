//! Rendering cache and composition

use mdx_core::config::ThemeVariant;
use ratatui::text::Line;

/// Key for render cache
#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct RenderKey {
    pub doc_rev: u64,
    pub width: u16,
    pub theme: ThemeVariant,
}

/// Rendered document output
#[derive(Clone)]
pub struct RenderedDoc {
    pub lines: Vec<Line<'static>>,
    pub source_to_rendered_first: Vec<usize>,
    pub rendered_to_source: Vec<usize>,
}

/// LRU cache for rendered documents
pub struct RendererCache {
    cache: lru::LruCache<RenderKey, RenderedDoc>,
}

impl RendererCache {
    pub fn new() -> Self {
        Self {
            cache: lru::LruCache::new(std::num::NonZeroUsize::new(32).unwrap()),
        }
    }

    pub fn get(&mut self, key: &RenderKey) -> Option<&RenderedDoc> {
        self.cache.get(key)
    }

    pub fn put(&mut self, key: RenderKey, doc: RenderedDoc) {
        self.cache.put(key, doc);
    }
}

impl Default for RendererCache {
    fn default() -> Self {
        Self::new()
    }
}
