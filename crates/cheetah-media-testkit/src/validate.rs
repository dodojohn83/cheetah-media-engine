//! Manifest and fixture validation for the testkit.

use alloc::vec::Vec;
use core::fmt;

use crate::{FixtureManifest, FixtureManifestEntry, SourceType};

/// Errors produced while validating a fixture manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureError {
    MissingSchema,
    InvalidSchemaVersion(String),
    DuplicateFixtureId(String),
    MissingField {
        fixture: String,
        field: &'static str,
    },
    InvalidField {
        fixture: String,
        field: &'static str,
        reason: &'static str,
    },
    InvalidProtocol {
        fixture: String,
        protocol: String,
    },
    InvalidCodec {
        fixture: String,
        codec: String,
    },
    MissingHash {
        fixture: String,
    },
    InvalidHash {
        fixture: String,
        hash: String,
    },
    InconsistentMetadata {
        fixture: String,
        reason: &'static str,
    },
    UnknownAnomaly {
        fixture: String,
        anomaly: String,
    },
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSchema => write!(f, "manifest schema_version is missing"),
            Self::InvalidSchemaVersion(v) => write!(f, "unsupported schema_version: {v}"),
            Self::DuplicateFixtureId(id) => write!(f, "duplicate fixture id: {id}"),
            Self::MissingField { fixture, field } => write!(f, "{fixture}: missing {field}"),
            Self::InvalidField {
                fixture,
                field,
                reason,
            } => write!(f, "{fixture}: invalid {field}: {reason}"),
            Self::InvalidProtocol { fixture, protocol } => {
                write!(f, "{fixture}: unsupported protocol {protocol}")
            }
            Self::InvalidCodec { fixture, codec } => {
                write!(f, "{fixture}: unsupported codec {codec}")
            }
            Self::MissingHash { fixture } => {
                write!(f, "{fixture}: hash is required for non-synthetic sources")
            }
            Self::InvalidHash { fixture, hash } => write!(
                f,
                "{fixture}: hash must be a 64 or 128 character hex string, got {hash}"
            ),
            Self::InconsistentMetadata { fixture, reason } => write!(f, "{fixture}: {reason}"),
            Self::UnknownAnomaly { fixture, anomaly } => {
                write!(f, "{fixture}: unknown anomaly {anomaly}")
            }
        }
    }
}

const KNOWN_PROTOCOLS: &[&str] = &[
    "elementary",
    "adts",
    "pcm",
    "flv",
    "http-flv",
    "ws-flv",
    "fmp4",
    "http-fmp4",
    "ws-fmp4",
    "ts",
    "rtsp",
    "rtmp",
    "hls",
    "ll-hls",
];
const KNOWN_CODECS: &[&str] = &["h264", "h265", "aac", "g711a", "g711u", "mp3"];
const KNOWN_ANOMALIES: &[&str] = &[
    "corrupt header and truncated tag",
    "33-bit timestamp wrap",
    "missing pmt",
    "discontinuity",
    "config change",
];

/// Validate a fixture manifest, returning all detected errors.
pub fn validate_manifest(manifest: &FixtureManifest) -> Result<(), Vec<FixtureError>> {
    let mut errors = Vec::new();

    if manifest.schema_version.is_empty() {
        errors.push(FixtureError::MissingSchema);
    } else if manifest.schema_version != "1.0" {
        errors.push(FixtureError::InvalidSchemaVersion(
            manifest.schema_version.clone(),
        ));
    }

    let mut seen_ids = Vec::new();
    for fixture in &manifest.fixtures {
        if seen_ids.contains(&fixture.id) {
            errors.push(FixtureError::DuplicateFixtureId(fixture.id.clone()));
        }
        seen_ids.push(fixture.id.clone());
        validate_entry(fixture, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_entry(entry: &FixtureManifestEntry, errors: &mut Vec<FixtureError>) {
    if entry.id.is_empty() {
        errors.push(FixtureError::MissingField {
            fixture: "<unnamed>".into(),
            field: "id",
        });
    }
    let fixture = entry.id.clone();

    if entry.description.is_empty() {
        errors.push(FixtureError::MissingField {
            fixture: fixture.clone(),
            field: "description",
        });
    }
    if entry.license.is_empty() {
        errors.push(FixtureError::MissingField {
            fixture: fixture.clone(),
            field: "license",
        });
    }
    if !KNOWN_PROTOCOLS.contains(&entry.protocol.as_str()) {
        errors.push(FixtureError::InvalidProtocol {
            fixture: fixture.clone(),
            protocol: entry.protocol.clone(),
        });
    }
    if !KNOWN_CODECS.contains(&entry.codec.as_str()) {
        errors.push(FixtureError::InvalidCodec {
            fixture: fixture.clone(),
            codec: entry.codec.clone(),
        });
    }

    match entry.source.r#type {
        SourceType::Synthetic => {
            if entry.source.generator.as_deref().unwrap_or("").is_empty() {
                errors.push(FixtureError::InvalidField {
                    fixture: fixture.clone(),
                    field: "source.generator",
                    reason: "synthetic sources must name a generator",
                });
            }
        }
        SourceType::Download | SourceType::Recorded => {
            if entry.hash.is_empty() {
                errors.push(FixtureError::MissingHash {
                    fixture: fixture.clone(),
                });
            }
        }
    }

    if !entry.hash.is_empty() && !is_valid_hash(&entry.hash) {
        errors.push(FixtureError::InvalidHash {
            fixture: fixture.clone(),
            hash: entry.hash.clone(),
        });
    }

    if entry.duration_ms == 0 && entry.anomaly.is_none() {
        errors.push(FixtureError::InvalidField {
            fixture: fixture.clone(),
            field: "duration_ms",
            reason: "duration must be positive unless an anomaly is declared",
        });
    }

    let is_video = entry.codec == "h264" || entry.codec == "h265";
    if is_video {
        if entry.resolution.is_none() {
            errors.push(FixtureError::MissingField {
                fixture: fixture.clone(),
                field: "resolution",
            });
        }
        if entry.frame_rate.is_none() {
            errors.push(FixtureError::MissingField {
                fixture: fixture.clone(),
                field: "frame_rate",
            });
        }
    } else {
        if entry.sample_rate.is_none() {
            errors.push(FixtureError::MissingField {
                fixture: fixture.clone(),
                field: "sample_rate",
            });
        }
        if entry.channels.is_none() {
            errors.push(FixtureError::MissingField {
                fixture: fixture.clone(),
                field: "channels",
            });
        }
    }

    if let Some(anomaly) = &entry.anomaly
        && !KNOWN_ANOMALIES.contains(&anomaly.as_str())
    {
        errors.push(FixtureError::UnknownAnomaly {
            fixture: fixture.clone(),
            anomaly: anomaly.clone(),
        });
    }

    if let Some(expected) = &entry.expected {
        if expected.is_empty() {
            errors.push(FixtureError::InvalidField {
                fixture: fixture.clone(),
                field: "expected",
                reason: "expected description must not be empty",
            });
        }
    } else {
        errors.push(FixtureError::MissingField {
            fixture: fixture.clone(),
            field: "expected",
        });
    }
}

fn is_valid_hash(hash: &str) -> bool {
    let len = hash.len();
    if len != 64 && len != 128 {
        return false;
    }
    hash.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FixtureManifest, FixtureManifestEntry, FixtureSource, SourceType};

    fn base_entry() -> FixtureManifestEntry {
        FixtureManifestEntry {
            id: "test".into(),
            description: "test fixture".into(),
            source: FixtureSource {
                r#type: SourceType::Synthetic,
                generator: Some("cheetah-media-testkit".into()),
                url: None,
                commit: None,
            },
            license: "MIT-0".into(),
            hash: "".into(),
            protocol: "elementary".into(),
            codec: "h264".into(),
            resolution: Some("1280x720".into()),
            frame_rate: Some(30),
            sample_rate: None,
            channels: None,
            duration_ms: 1000,
            anomaly: None,
            expected: Some("valid".into()),
        }
    }

    fn manifest(entries: Vec<FixtureManifestEntry>) -> FixtureManifest {
        FixtureManifest {
            schema_version: "1.0".into(),
            fixtures: entries,
        }
    }

    #[test]
    fn valid_manifest_passes() {
        let m = manifest(vec![base_entry()]);
        assert!(validate_manifest(&m).is_ok());
    }

    #[test]
    fn rejects_missing_schema_version() {
        let mut m = manifest(vec![base_entry()]);
        m.schema_version = "".into();
        let errors = validate_manifest(&m).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, FixtureError::MissingSchema))
        );
    }

    #[test]
    fn rejects_duplicate_ids() {
        let m = manifest(vec![base_entry(), base_entry()]);
        let errors = validate_manifest(&m).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, FixtureError::DuplicateFixtureId(_)))
        );
    }

    #[test]
    fn rejects_unknown_protocol() {
        let mut e = base_entry();
        e.protocol = "rtp".into();
        let m = manifest(vec![e]);
        let errors = validate_manifest(&m).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, FixtureError::InvalidProtocol { .. }))
        );
    }

    #[test]
    fn requires_hash_for_download() {
        let mut e = base_entry();
        e.source.r#type = SourceType::Download;
        e.source.url = Some("https://example.com/f.flv".into());
        let m = manifest(vec![e]);
        let errors = validate_manifest(&m).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, FixtureError::MissingHash { .. }))
        );
    }

    #[test]
    fn rejects_video_without_resolution() {
        let mut e = base_entry();
        e.resolution = None;
        let m = manifest(vec![e]);
        let errors = validate_manifest(&m).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            FixtureError::MissingField {
                field: "resolution",
                ..
            }
        )));
    }

    #[test]
    fn accepts_anomaly_with_zero_duration() {
        let mut e = base_entry();
        e.duration_ms = 0;
        e.anomaly = Some("corrupt header and truncated tag".into());
        let m = manifest(vec![e]);
        assert!(validate_manifest(&m).is_ok());
    }
}
