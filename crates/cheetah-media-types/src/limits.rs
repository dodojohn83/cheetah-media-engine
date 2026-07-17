//! Per-resource limits and boundary checking.

use crate::MediaError;

/// Resource limits for media parsing, buffering, and rendering.
///
/// All fields are public so callers can inspect configured values, but the
/// `check_*` helpers enforce the limits and return structured errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaLimits {
    /// Maximum number of tracks in a presentation.
    pub max_tracks: u32,
    /// Maximum ISOBMFF box / FLV tag / PES packet size in bytes.
    pub max_box_tag_pes_size: u64,
    /// Maximum number of parameter sets (SPS/PPS/VPS etc.) per track.
    pub max_parameter_sets: u32,
    /// Maximum coded resolution width/height in pixels.
    pub max_resolution: (u32, u32),
    /// Maximum decoded frame size in bytes.
    pub max_frame_size_bytes: u64,
    /// Maximum buffer/cache duration in milliseconds.
    pub max_cache_duration_ms: u64,
    /// Maximum per-track queue depth before back-pressure.
    pub max_queue_depth: u32,
    /// Maximum number of bytes allowed for an elementary stream read.
    pub max_read_chunk_bytes: u64,
}

impl Default for MediaLimits {
    /// Default limits tuned for the Web v1 use case.
    fn default() -> Self {
        Self {
            max_tracks: 16,
            max_box_tag_pes_size: 16 * 1024 * 1024, // 16 MiB
            max_parameter_sets: 64,
            max_resolution: (7680, 4320),            // 8K
            max_frame_size_bytes: 128 * 1024 * 1024, // 128 MiB
            max_cache_duration_ms: 30_000,           // 30 s
            max_queue_depth: 256,
            max_read_chunk_bytes: 1024 * 1024, // 1 MiB
        }
    }
}

impl MediaLimits {
    /// Check that `value` is within `[min, max]` (inclusive).
    ///
    /// `name` is used for diagnostics. No payload is recorded.
    pub fn check_u64(
        &self,
        name: &'static str,
        value: u64,
        min: u64,
        max: u64,
    ) -> Result<(), MediaError> {
        if value < min || value > max {
            return Err(MediaError::ResourceLimit {
                name,
                current: value,
                limit: max,
            });
        }
        Ok(())
    }

    /// Check that `count` tracks does not exceed `max_tracks`.
    pub fn check_track_count(&self, count: u32) -> Result<(), MediaError> {
        if count > self.max_tracks {
            Err(MediaError::ResourceLimit {
                name: "track_count",
                current: count as u64,
                limit: self.max_tracks as u64,
            })
        } else {
            Ok(())
        }
    }

    /// Check a parsed container chunk size.
    pub fn check_chunk_size(&self, size: u64) -> Result<(), MediaError> {
        if size > self.max_box_tag_pes_size {
            Err(MediaError::ResourceLimit {
                name: "box_tag_pes_size",
                current: size,
                limit: self.max_box_tag_pes_size,
            })
        } else {
            Ok(())
        }
    }

    /// Check a resolution against `max_resolution`.
    pub fn check_resolution(&self, width: u32, height: u32) -> Result<(), MediaError> {
        let (max_w, max_h) = self.max_resolution;
        if width > max_w {
            return Err(MediaError::ResourceLimit {
                name: "resolution_width",
                current: u64::from(width),
                limit: u64::from(max_w),
            });
        }
        if height > max_h {
            return Err(MediaError::ResourceLimit {
                name: "resolution_height",
                current: u64::from(height),
                limit: u64::from(max_h),
            });
        }
        Ok(())
    }

    /// Check that queue depth is within `max_queue_depth`.
    pub fn check_queue_depth(&self, depth: u32) -> Result<(), MediaError> {
        if depth > self.max_queue_depth {
            Err(MediaError::ResourceLimit {
                name: "queue_depth",
                current: depth as u64,
                limit: self.max_queue_depth as u64,
            })
        } else {
            Ok(())
        }
    }

    /// Check a read chunk size.
    pub fn check_read_chunk(&self, bytes: u64) -> Result<(), MediaError> {
        if bytes > self.max_read_chunk_bytes {
            Err(MediaError::ResourceLimit {
                name: "read_chunk_bytes",
                current: bytes,
                limit: self.max_read_chunk_bytes,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_allow_1080p() {
        let limits = MediaLimits::default();
        assert!(limits.check_resolution(1920, 1080).is_ok());
    }

    #[test]
    fn default_limits_reject_excessive_resolution_width() {
        let limits = MediaLimits::default();
        let err = limits.check_resolution(8192, 4320).unwrap_err();
        assert_eq!(err.code(), 5001);
    }

    #[test]
    fn default_limits_reject_excessive_resolution_height() {
        let limits = MediaLimits::default();
        let err = limits.check_resolution(7680, 4321).unwrap_err();
        assert_eq!(err.code(), 5001);
        assert_eq!(err.stage(), "limit");
    }

    #[test]
    fn chunk_size_rejects_large_input() {
        let limits = MediaLimits::default();
        assert!(limits.check_chunk_size(17 * 1024 * 1024).is_err());
    }

    #[test]
    fn track_count_limit() {
        let limits = MediaLimits {
            max_tracks: 2,
            ..Default::default()
        };
        assert!(limits.check_track_count(2).is_ok());
        assert!(limits.check_track_count(3).is_err());
    }
}
