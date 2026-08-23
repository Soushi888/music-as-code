//! Content-addressed musical fragments: [`Phrase`] and [`RenderCache`].
//!
//! A `Phrase` wraps a `Music` tree in an `Arc` and stores its Blake3 hash.
//! The `RenderCache` maps `(hash, BackendId)` to rendered output, so
//! unchanged subtrees are never re-rendered.

use std::sync::Arc;

use crate::attrs::BackendId;
use crate::music::Music;

/// A reference-counted, content-hashed musical fragment. The unit of caching.
///
/// Two `Phrase`s are equal if and only if their hashes are equal, meaning their
/// `Music` trees have identical structure and content. This makes `Phrase` safe
/// to use as a cache key.
///
/// The hash is computed via [`bincode`] serialization followed by Blake3 hashing.
/// This is deterministic for any given `Music` value.
#[derive(Clone, Debug)]
pub struct Phrase {
    inner: Arc<Music>,
    hash: blake3::Hash,
}

impl Phrase {
    /// Wrap a `Music` tree in a `Phrase`, computing its content hash.
    ///
    /// The `Music` value is serialized with `bincode` and hashed with Blake3.
    /// The result is stored in an `Arc` so cloning the `Phrase` is cheap.
    ///
    /// # Panics
    /// Panics if `bincode` serialization fails. This should never occur for
    /// well-formed `Music` values since all field types are serializable.
    pub fn new(m: Music) -> Self {
        let bytes = bincode::serialize(&m).expect("Music must be serializable");
        let hash = blake3::hash(&bytes);
        Self { inner: Arc::new(m), hash }
    }

    /// Return a reference to the Blake3 content hash of this fragment.
    ///
    /// Two phrases with the same hash have structurally identical `Music` trees.
    pub fn hash(&self) -> &blake3::Hash {
        &self.hash
    }

    /// Borrow the inner `Music` tree.
    pub fn music(&self) -> &Music {
        &self.inner
    }
}

impl PartialEq for Phrase {
    /// Two phrases are equal iff their Blake3 hashes match.
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for Phrase {}

/// A concurrent render cache keyed by `(content_hash, backend_id)`.
///
/// Stores rendered output `O` for each `(Phrase, backend)` pair.
/// When a large composition changes in one section, only that section's
/// hash is new; all unchanged subtrees are cache hits.
///
/// Backed by a [`dashmap::DashMap`] for lock-free concurrent access.
pub struct RenderCache<O> {
    map: dashmap::DashMap<(blake3::Hash, BackendId), O>,
}

impl<O> RenderCache<O> {
    /// Create an empty render cache.
    pub fn new() -> Self {
        Self { map: dashmap::DashMap::new() }
    }

    /// Look up a cached render output by content hash and backend ID.
    ///
    /// Returns `None` if the `(hash, backend)` pair has not been cached yet.
    /// The return type is a `dashmap` read guard; the entry is held for the
    /// lifetime of the guard.
    pub fn get(
        &self,
        hash: &blake3::Hash,
        backend: &BackendId,
    ) -> Option<dashmap::mapref::one::Ref<'_, (blake3::Hash, BackendId), O>> {
        self.map.get(&(hash.clone(), backend.clone()))
    }

    /// Store a rendered output for the given `(hash, backend)` pair.
    ///
    /// If an entry already exists for this key it is overwritten.
    pub fn insert(&self, hash: blake3::Hash, backend: BackendId, output: O) {
        self.map.insert((hash, backend), output);
    }

    /// Return the cached output for `(phrase, backend)`, rendering with `f`
    /// and storing the result when there is none.
    ///
    /// This is how a backend participates in content addressing: the same
    /// `Music` tree under the same backend is rendered once per cache.
    ///
    /// ```
    /// use musecode_core::prelude::*;
    /// let cache: RenderCache<Vec<u8>> = RenderCache::new();
    /// let phrase = Phrase::new(n(C4, q()));
    /// let backend = BackendId("midi".into());
    /// let bytes = cache
    ///     .render_with(&phrase, backend.clone(), |m| render_midi(m, &MidiOptions::default()).unwrap())
    ///     .clone();
    /// assert!(cache.get(phrase.hash(), &backend).is_some());
    /// assert_eq!(&bytes[0..4], b"MThd");
    /// ```
    pub fn render_with<F: FnOnce(&Music) -> O>(
        &self,
        phrase: &Phrase,
        backend: BackendId,
        f: F,
    ) -> dashmap::mapref::one::Ref<'_, (blake3::Hash, BackendId), O> {
        let key = (*phrase.hash(), backend);
        if let Some(hit) = self.map.get(&key) {
            return hit;
        }
        let rendered = f(phrase.music());
        self.map.entry(key).or_insert(rendered).downgrade()
    }
}

impl<O> Default for RenderCache<O> {
    fn default() -> Self {
        Self::new()
    }
}
