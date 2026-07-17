//! Fixture store with validation and offline-aware status.

use alloc::string::String;
use alloc::vec::Vec;

use crate::{FixtureError, FixtureManifest, FixtureManifestEntry, validate_manifest};

/// Availability status of a fixture at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureStatus {
    /// The fixture is present and validated.
    Available,
    /// The fixture is a remote/download fixture and the source is not cached.
    SkippedOffline,
    /// The fixture manifest entry is invalid.
    Invalid,
}

/// In-memory fixture store.
#[derive(Debug, Clone)]
pub struct FixtureStore {
    manifest: FixtureManifest,
    errors: Vec<FixtureError>,
}

impl FixtureStore {
    /// Load and validate a manifest.
    pub fn new(manifest: FixtureManifest) -> Self {
        let errors = validate_manifest(&manifest).err().unwrap_or_default();
        Self { manifest, errors }
    }

    /// Load the workspace manifest embedded in the crate.
    pub fn workspace() -> Self {
        Self::new(crate::workspace_manifest().expect("embedded manifest is valid JSON"))
    }

    /// Return validation errors from the loaded manifest.
    pub fn errors(&self) -> &[FixtureError] {
        &self.errors
    }

    /// Return the manifest.
    pub fn manifest(&self) -> &FixtureManifest {
        &self.manifest
    }

    /// Find a fixture by id.
    pub fn find(&self, id: &str) -> Option<&FixtureManifestEntry> {
        self.manifest.fixtures.iter().find(|f| f.id == id)
    }

    /// Return the runtime status of a fixture.
    ///
    /// Download/recorded fixtures are skipped when `online` is `false` unless a
    /// hash is provided (hash-only validation is allowed offline). Synthetic
    /// fixtures are always available if the manifest entry is valid.
    pub fn status(&self, entry: &FixtureManifestEntry, online: bool) -> FixtureStatus {
        if !self.errors_for(&entry.id).is_empty() {
            return FixtureStatus::Invalid;
        }
        match entry.source.r#type {
            crate::SourceType::Synthetic => FixtureStatus::Available,
            crate::SourceType::Download | crate::SourceType::Recorded => {
                if online || !entry.hash.is_empty() {
                    FixtureStatus::Available
                } else {
                    FixtureStatus::SkippedOffline
                }
            }
        }
    }

    /// Return all validation errors for a specific fixture id.
    pub fn errors_for(&self, id: &str) -> Vec<FixtureError> {
        self.errors
            .iter()
            .filter(|e| error_fixture_id(e).as_deref() == Some(id))
            .cloned()
            .collect()
    }
}

fn error_fixture_id(err: &FixtureError) -> Option<String> {
    use crate::FixtureError::*;
    let id = match err {
        MissingSchema | InvalidSchemaVersion(_) => return None,
        DuplicateFixtureId(id) => id,
        MissingField { fixture, .. }
        | InvalidField { fixture, .. }
        | InvalidProtocol { fixture, .. }
        | InvalidCodec { fixture, .. }
        | MissingHash { fixture }
        | InvalidHash { fixture, .. }
        | InconsistentMetadata { fixture, .. }
        | UnknownAnomaly { fixture, .. } => fixture,
    };
    Some(id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_store_loads() {
        let store = FixtureStore::workspace();
        if !store.errors().is_empty() {
            eprintln!("{:#?}", store.errors());
        }
        assert!(store.errors().is_empty());
        let f = store.find("h264-1280x720-30fps-2s").expect("fixture");
        assert_eq!(store.status(f, false), FixtureStatus::Available);
    }
}
