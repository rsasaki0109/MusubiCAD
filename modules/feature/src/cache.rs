//! Content-addressed feature output cache (MCAD-P6-002).

use std::collections::BTreeMap;

use opencad_geometry::{EdgeRefDiscovery, FaceDerivation, FaceRefDiscovery};

use crate::feature::FeatureOutput;

/// A feature output plus the regeneration evidence produced alongside it.
#[derive(Debug, Clone, Default)]
pub struct CachedFeature {
    pub output: FeatureOutput,
    pub face_history: Vec<FaceDerivation>,
    pub face_discoveries: Vec<FaceRefDiscovery>,
    pub edge_discoveries: Vec<EdgeRefDiscovery>,
}

/// In-memory, content-addressed cache of regenerated feature outputs.
///
/// Entries are keyed by a versioned hash over the feature definition, its
/// solved source sketch, upstream output identity, and the kernel backend tag.
/// The cache is disposable and lives outside the `.ocad` source truth. It is
/// bound to one kernel session: callers must reuse the same `GeometryKernel`
/// instance and this cache across regenerations so cached kernel body handles
/// remain valid.
#[derive(Debug, Clone)]
pub struct RegenerationCache {
    backend_tag: String,
    entries: BTreeMap<String, CachedFeature>,
}

impl RegenerationCache {
    pub fn new() -> Self {
        Self::with_backend("default")
    }

    /// Bind the cache to a kernel backend/contract tag that becomes part of
    /// every content key.
    pub fn with_backend(backend_tag: impl Into<String>) -> Self {
        Self {
            backend_tag: backend_tag.into(),
            entries: BTreeMap::new(),
        }
    }

    pub fn backend_tag(&self) -> &str {
        &self.backend_tag
    }

    pub fn get(&self, key: &str) -> Option<&CachedFeature> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: String, feature: CachedFeature) {
        self.entries.insert(key, feature);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for RegenerationCache {
    fn default() -> Self {
        Self::new()
    }
}
