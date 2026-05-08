//! Content-addressed musical fragments: [`Phrase`] and [`RenderCache`].
//!
//! A `Phrase` wraps a `Music` tree in an `Arc` and stores its Blake3 hash.
//! The `RenderCache` maps `(hash, BackendId)` to rendered output, so
//! unchanged subtrees are never re-rendered.

use std::sync::Arc;

use crate::attrs::BackendId;
use crate::music::Music;

/// A reference-counted, content-hashed musical fragment. The unit of caching.
#[derive(Clone, Debug)]
pub struct Phrase {
    inner: Arc<Music>,
    hash: blake3::Hash,
}

impl Phrase {
    pub fn new(m: Music) -> Self {
        let bytes = bincode::serialize(&m).expect("Music must be serializable");
        let hash = blake3::hash(&bytes);
        Self {
            inner: Arc::new(m),
            hash,
        }
    }

    pub fn hash(&self) -> &blake3::Hash {
        &self.hash
    }

    pub fn music(&self) -> &Music {
        &self.inner
    }
}

impl PartialEq for Phrase {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for Phrase {}

/// A render cache: compute once per (content-hash, backend) pair.
pub struct RenderCache<O> {
    map: dashmap::DashMap<(blake3::Hash, BackendId), O>,
}

impl<O> RenderCache<O> {
    pub fn new() -> Self {
        Self {
            map: dashmap::DashMap::new(),
        }
    }

    pub fn get(
        &self,
        hash: &blake3::Hash,
        backend: &BackendId,
    ) -> Option<dashmap::mapref::one::Ref<'_, (blake3::Hash, BackendId), O>> {
        self.map.get(&(hash.clone(), backend.clone()))
    }

    pub fn insert(&self, hash: blake3::Hash, backend: BackendId, output: O) {
        self.map.insert((hash, backend), output);
    }
}

impl<O> Default for RenderCache<O> {
    fn default() -> Self {
        Self::new()
    }
}
