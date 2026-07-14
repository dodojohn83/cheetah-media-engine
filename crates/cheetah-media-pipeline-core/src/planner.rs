//! Platform-neutral pipeline planner.
//!
//! The planner is a pure function that maps a `PipelineRequest` and a
//! `CapabilitySnapshot` to an ordered list of `PipelinePlan` candidates.
//! It never calls browser or platform APIs; all capability discovery is supplied
//! by the caller in the snapshot.

use alloc::string::String;
use alloc::vec::Vec;

use cheetah_media_abi::AbiError;
use cheetah_media_types::{CodecId, MediaLimits, TrackInfo, TrackKind};

/// Input transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Flv,
    MpegTs,
    Isobmff,
    Hls,
    Rtsp,
    Rtmp,
    WebSocket,
    WebTransport,
}

/// Demuxer selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemuxKind {
    Flv,
    MpegTs,
    Isobmff,
    Hls,
    Raw,
}

/// Decoder selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePath {
    /// Browser WebCodecs API.
    WebCodecs,
    /// Software decoder in a WASM module (e.g. FFmpeg LGPL pack).
    FFmpegWasm,
    /// Generic or fallback software decoder.
    Software,
}

/// Render output path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderTarget {
    VideoElement,
    WebGL,
    WebGPU,
    Canvas2d,
}

/// Audio output path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPath {
    WebAudio,
    AudioWorklet,
    Software,
}

/// Latency target for the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyMode {
    Low,
    Normal,
    HighQuality,
}

/// Isolation requirements for SharedArrayBuffer/WebGPU/etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationState {
    Enabled,
    Disabled,
}

/// User-facing constraints applied to the pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserConstraints {
    /// Maximum decoded width.
    pub max_width: Option<u32>,
    /// Maximum decoded height.
    pub max_height: Option<u32>,
    /// Preferred codecs in priority order (empty means no preference).
    pub preferred_codecs: Vec<CodecId>,
    /// Maximum total bitrate in bits per second.
    pub max_bitrate_bps: Option<u64>,
    /// Whether recording is requested.
    pub record: bool,
}

/// Request for a pipeline plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRequest {
    pub transport: TransportKind,
    pub tracks: Vec<TrackInfo>,
    pub latency: LatencyMode,
    pub isolation: IsolationState,
    pub constraints: UserConstraints,
}

/// Probed capability for a single codec/operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    Hardware,
    Software,
    Unsupported,
}

/// Snapshot of platform capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySnapshot {
    /// WebCodecs support per codec.
    pub webcodecs: Vec<(CodecId, SupportLevel)>,
    /// FFmpeg WASM pack availability per codec.
    pub ffmpeg_wasm: Vec<(CodecId, SupportLevel)>,
    /// Whether WebGPU is available.
    pub webgpu: bool,
    /// Whether WebGL is available.
    pub webgl: bool,
    /// Whether SharedArrayBuffer / cross-origin isolation is active.
    pub cross_origin_isolated: bool,
    /// Confidence in the snapshot (0..100).
    pub confidence: u8,
    /// Expiry timestamp or generation; checked by the caller.
    pub expires_at_ms: u64,
    /// Failure reason if probing failed.
    pub failure_reason: Option<String>,
}

/// A planned pipeline path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelinePlan {
    pub transport: TransportKind,
    pub demux: DemuxKind,
    pub video_decode: DecodePath,
    pub audio_decode: DecodePath,
    pub render: RenderTarget,
    pub audio: AudioPath,
    pub record: bool,
    /// Free-form reason codes for the chosen path.
    pub reason_codes: Vec<&'static str>,
    /// Estimated bytes copied per video frame through the hot path.
    pub estimated_copy_bytes_per_frame: u64,
    /// Candidate score; higher is preferred.
    pub score: i32,
}

fn demux_for(transport: TransportKind) -> DemuxKind {
    match transport {
        TransportKind::Flv | TransportKind::Rtmp => DemuxKind::Flv,
        TransportKind::MpegTs | TransportKind::Rtsp => DemuxKind::MpegTs,
        TransportKind::Isobmff => DemuxKind::Isobmff,
        TransportKind::Hls => DemuxKind::Hls,
        TransportKind::WebSocket | TransportKind::WebTransport => DemuxKind::Raw,
    }
}

fn best_decode_path(codec: CodecId, caps: &CapabilitySnapshot) -> DecodePath {
    if caps
        .webcodecs
        .iter()
        .any(|(c, l)| *c == codec && !matches!(l, SupportLevel::Unsupported))
    {
        return DecodePath::WebCodecs;
    }
    if caps
        .ffmpeg_wasm
        .iter()
        .any(|(c, l)| *c == codec && !matches!(l, SupportLevel::Unsupported))
    {
        return DecodePath::FFmpegWasm;
    }
    DecodePath::Software
}

fn codec_supported(codec: CodecId, caps: &CapabilitySnapshot) -> bool {
    best_decode_path(codec, caps) != DecodePath::Software
}

fn render_for(caps: &CapabilitySnapshot, _constraints: &UserConstraints) -> RenderTarget {
    if caps.webgpu {
        RenderTarget::WebGPU
    } else if caps.webgl {
        RenderTarget::WebGL
    } else {
        RenderTarget::VideoElement
    }
}

fn audio_for(latency: LatencyMode, caps: &CapabilitySnapshot) -> AudioPath {
    if latency == LatencyMode::Low && caps.cross_origin_isolated {
        AudioPath::AudioWorklet
    } else {
        AudioPath::WebAudio
    }
}

/// Plan a pipeline for `request` given platform `caps` and resource `limits`.
///
/// Returns a sorted list of candidates; the first entry is the recommended plan.
/// The function is pure: the same inputs always produce the same output.
pub fn plan(
    request: &PipelineRequest,
    caps: &CapabilitySnapshot,
    limits: &MediaLimits,
) -> Result<Vec<PipelinePlan>, AbiError> {
    if caps.confidence == 0 {
        return Err(AbiError::NotSupported);
    }

    // Verify every track has a supportable decode path.
    for track in &request.tracks {
        if !codec_supported(track.codec, caps) && !matches!(track.kind, TrackKind::Data) {
            return Err(AbiError::NotSupported);
        }
    }

    let mut plan = base_plan(request, caps);

    // Apply resource and constraint adjustments.
    if request.latency == LatencyMode::Low {
        plan.reason_codes.push("low_latency_prefer_hardware");
        plan.score += 10;
    }

    if request.isolation == IsolationState::Enabled && !caps.cross_origin_isolated {
        plan.reason_codes
            .push("isolation_requested_but_unavailable");
        plan.score -= 20;
    }

    if let Some(max_bps) = request.constraints.max_bitrate_bps {
        // Estimate nominal 1080p30 as ~5 Mbps for comparison.
        if max_bps < 5_000_000 {
            plan.reason_codes.push("bitrate_constrained");
            plan.score -= 5;
        }
    }

    // Estimate copy bytes based on constraints.
    let width = request.constraints.max_width.unwrap_or(1920) as u64;
    let height = request.constraints.max_height.unwrap_or(1080) as u64;
    let bytes_per_frame = width * height * 3 / 2; // conservative NV12-ish
    plan.estimated_copy_bytes_per_frame = bytes_per_frame;

    // Check resolution limits.
    if let (Some(max_w), Some(max_h)) = (
        request.constraints.max_width,
        request.constraints.max_height,
    ) {
        let within_limits = limits.check_resolution(max_w, max_h).is_ok();
        if !within_limits {
            return Err(AbiError::NotSupported);
        }
    }

    // Build the ranked candidate list. For now the planner returns the chosen
    // plan plus a single fallback so callers can observe deterministic ordering.
    let mut candidates = alloc::vec![plan];
    let mut fallback = fallback_plan(request, caps);
    fallback.score = candidates[0].score - 1;
    candidates.push(fallback);

    // Stable, deterministic ordering by score then by reason string.
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.reason_codes.as_slice().cmp(b.reason_codes.as_slice()))
    });

    Ok(candidates)
}

fn base_plan(request: &PipelineRequest, caps: &CapabilitySnapshot) -> PipelinePlan {
    let demux = demux_for(request.transport);

    let mut video_decode = DecodePath::Software;
    let mut audio_decode = DecodePath::Software;

    for track in &request.tracks {
        match track.kind {
            TrackKind::Video => video_decode = best_decode_path(track.codec, caps),
            TrackKind::Audio => audio_decode = best_decode_path(track.codec, caps),
            TrackKind::Data => {}
        }
    }

    let reason_codes = alloc::vec!["base_plan"];

    PipelinePlan {
        transport: request.transport,
        demux,
        video_decode,
        audio_decode,
        render: render_for(caps, &request.constraints),
        audio: audio_for(request.latency, caps),
        record: request.constraints.record,
        reason_codes,
        estimated_copy_bytes_per_frame: 0,
        score: 100,
    }
}

fn fallback_plan(request: &PipelineRequest, caps: &CapabilitySnapshot) -> PipelinePlan {
    let mut plan = base_plan(request, caps);
    plan.video_decode = DecodePath::FFmpegWasm;
    plan.audio_decode = DecodePath::FFmpegWasm;
    plan.render = RenderTarget::Canvas2d;
    plan.audio = AudioPath::Software;
    plan.reason_codes.clear();
    plan.reason_codes.push("software_fallback");
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use cheetah_media_types::{CodecId, TimeBase, TrackId, TrackInfo, TrackKind};

    fn track(kind: TrackKind, codec: CodecId) -> TrackInfo {
        TrackInfo::new(TrackId::new(1).unwrap(), kind, codec, TimeBase::DEFAULT)
    }

    fn caps_with_h264() -> CapabilitySnapshot {
        let mut caps = CapabilitySnapshot::default();
        caps.webcodecs.push((CodecId::H264, SupportLevel::Hardware));
        caps.confidence = 100;
        caps.webgl = true;
        caps.cross_origin_isolated = true;
        caps
    }

    #[test]
    fn plans_webcodecs_for_h264() {
        let caps = caps_with_h264();
        let request = PipelineRequest {
            transport: TransportKind::Flv,
            tracks: vec![track(TrackKind::Video, CodecId::H264)],
            latency: LatencyMode::Normal,
            isolation: IsolationState::Disabled,
            constraints: UserConstraints::default(),
        };
        let plans = plan(&request, &caps, &MediaLimits::default()).unwrap();
        assert!(!plans.is_empty());
        assert_eq!(plans[0].video_decode, DecodePath::WebCodecs);
        assert_eq!(plans[0].demux, DemuxKind::Flv);
    }

    #[test]
    fn unsupported_codec_fails() {
        let caps = CapabilitySnapshot {
            confidence: 100,
            ..Default::default()
        };
        let request = PipelineRequest {
            transport: TransportKind::Flv,
            tracks: vec![track(TrackKind::Video, CodecId::H265)],
            latency: LatencyMode::Normal,
            isolation: IsolationState::Disabled,
            constraints: UserConstraints::default(),
        };
        assert_eq!(
            plan(&request, &caps, &MediaLimits::default()).unwrap_err(),
            AbiError::NotSupported
        );
    }
}
