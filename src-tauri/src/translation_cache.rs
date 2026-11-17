//! In-memory cache of per-paragraph translations.
//!
//! Re-selecting the same text (scrolling back, re-opening the selector on
//! the same page, a pinned region that re-renders) should show its
//! translation instantly, like Lens does, instead of paying for another
//! model round-trip. Keyed by target language + exact source paragraph.
//! Bounded, oldest entries evicted first; cleared whenever the provider
//! setup changes so a new model or prompt is never masked by old output.

use std::collections::{HashMap, VecDeque};

use parking_lot::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedTranslation {
    pub text: String,
    pub provider: String,
    pub via_cloud: bool,
}

type Key = (String, String);

pub struct TranslationCache {
    capacity: usize,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    map: HashMap<Key, CachedTranslation>,
    order: VecDeque<Key>,
}

impl TranslationCache {
    pub const DEFAULT_CAPACITY: usize = 2000;

    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::new(Inner::default()),
        }
    }

    fn key(target: &str, source: &str) -> Key {
        (
            target.trim().to_ascii_lowercase(),
            source.trim().to_string(),
        )
    }

    pub fn get(&self, target: &str, source: &str) -> Option<CachedTranslation> {
        self.inner
            .lock()
            .map
            .get(&Self::key(target, source))
            .cloned()
    }

    pub fn put(&self, target: &str, source: &str, value: CachedTranslation) {
        if source.trim().is_empty() || value.text.trim().is_empty() {
            return;
        }
        let key = Self::key(target, source);
        let mut inner = self.inner.lock();
        if inner.map.insert(key.clone(), value).is_none() {
            inner.order.push_back(key);
        }
        while inner.order.len() > self.capacity {
            if let Some(old) = inner.order.pop_front() {
                inner.map.remove(&old);
            }
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.map.clear();
        inner.order.clear();
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(t: &str) -> CachedTranslation {
        CachedTranslation {
            text: t.into(),
            provider: "p".into(),
            via_cloud: false,
        }
    }

    #[test]
    fn keyed_by_target_and_trimmed_source() {
        let c = TranslationCache::new(10);
        c.put("ja", " Hello ", v("こんにちは"));
        assert_eq!(c.get("JA", "Hello").unwrap().text, "こんにちは");
        assert!(c.get("en", "Hello").is_none());
    }

    #[test]
    fn evicts_oldest_beyond_capacity() {
        let c = TranslationCache::new(2);
        c.put("ja", "a", v("A"));
        c.put("ja", "b", v("B"));
        c.put("ja", "a", v("A2")); // update keeps its slot
        c.put("ja", "c", v("C"));
        assert!(c.get("ja", "a").is_none());
        assert_eq!(c.get("ja", "b").unwrap().text, "B");
        assert_eq!(c.get("ja", "c").unwrap().text, "C");
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn ignores_empty_and_clears() {
        let c = TranslationCache::new(5);
        c.put("ja", "", v("x"));
        c.put("ja", "a", v("  "));
        assert!(c.is_empty());
        c.put("ja", "a", v("A"));
        c.clear();
        assert!(c.get("ja", "a").is_none());
    }
}
