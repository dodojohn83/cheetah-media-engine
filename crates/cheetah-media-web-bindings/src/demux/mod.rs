//! WASM bindings for Annex-B and MPEG-PS demuxers.
//!
//! These are thin wrappers around `cheetah_container_annexb` and
//! `cheetah_container_mpegps`. They use a per-instance `MemoryArena` so that
//! emitted payload and configuration bytes can be read directly from WASM
//! linear memory by the JavaScript runtime.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use cheetah_container_annexb::{AnnexBConfig, AnnexBDemuxer as InnerAnnexBDemuxer, AnnexbEvent};
use cheetah_container_mpegps::{MpegPsConfig, MpegPsDemuxer as InnerMpegPsDemuxer, MpegPsEvent};
use cheetah_media_abi::{Handle, MemoryArena};
use cheetah_media_types::{CodecId, TimeBase, TrackInfo};
use wasm_bindgen::prelude::*;

mod demux_arena;
mod demux_event;

pub use demux_event::{DemuxEvent, DemuxEventKind};

use demux_arena::{metadata_event, packet_event, track_event};

static INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_instance_id() -> u64 {
    INSTANCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn codec_from_u8(value: u8) -> Option<CodecId> {
    match value {
        0 => Some(CodecId::H264),
        1 => Some(CodecId::H265),
        _ => None,
    }
}

fn annexb_error_to_code(e: &cheetah_container_annexb::AnnexbError) -> u32 {
    match e {
        cheetah_container_annexb::AnnexbError::BufferExceeded { .. } => 7001,
        cheetah_container_annexb::AnnexbError::NalTooLarge { .. } => 7002,
        cheetah_container_annexb::AnnexbError::InvalidInput => 7003,
        cheetah_container_annexb::AnnexbError::UnsupportedCodec => 7004,
    }
}

fn mpegps_error_to_code(e: &cheetah_container_mpegps::MpegPsError) -> u32 {
    match e {
        cheetah_container_mpegps::MpegPsError::NeedMoreData => 8001,
        cheetah_container_mpegps::MpegPsError::BufferExceeded { .. } => 8002,
        cheetah_container_mpegps::MpegPsError::PacketTooLarge { .. } => 8003,
        cheetah_container_mpegps::MpegPsError::InvalidInput => 8004,
        cheetah_container_mpegps::MpegPsError::UnsupportedVideoCodec => 8005,
        cheetah_container_mpegps::MpegPsError::UnrecognizedStreamId => 8006,
    }
}

/// WASM binding for the Annex-B H.264/H.265 demuxer.
#[wasm_bindgen]
pub struct AnnexBDemuxer {
    inner: InnerAnnexBDemuxer,
    arena: MemoryArena,
    last_data: Option<Handle>,
    last_metadata: Option<Handle>,
    last_config: Option<Handle>,
    tracks: BTreeMap<u32, TrackInfo>,
    errored: bool,
}

#[wasm_bindgen]
impl AnnexBDemuxer {
    /// Create a new Annex-B demuxer.
    ///
    /// `video_codec`: 0 = H.264, 1 = H.265.
    #[wasm_bindgen(constructor)]
    pub fn new(
        video_codec: u8,
        max_nal_size: u32,
        max_buffer: u32,
    ) -> Result<AnnexBDemuxer, JsValue> {
        let codec = codec_from_u8(video_codec).ok_or("unsupported video codec")?;
        let track_id = cheetah_media_types::TrackId::new(1)
            .ok_or_else(|| JsValue::from_str("invalid track id"))?;
        let mut config = AnnexBConfig::h264(track_id, TimeBase::TS_90K);
        config.codec = codec;
        config.max_nal_size_bytes = max_nal_size as usize;
        config.max_buffer_bytes = max_buffer as usize;
        Ok(Self {
            inner: InnerAnnexBDemuxer::new(config),
            arena: MemoryArena::new(next_instance_id()),
            last_data: None,
            last_metadata: None,
            last_config: None,
            tracks: BTreeMap::new(),
            errored: false,
        })
    }

    /// Push more Annex-B bytes.
    pub fn push(&mut self, data: &[u8]) {
        if !self.errored && !data.is_empty() {
            self.inner.push(data);
        }
    }

    /// Signal end of stream.
    pub fn end(&mut self) -> Result<(), JsValue> {
        self.inner.end().map_err(|e| {
            let msg = e.to_string();
            JsValue::from_str(&msg)
        })
    }

    /// Reset the demuxer and release all arena slots.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.arena = MemoryArena::new(next_instance_id());
        self.last_data = None;
        self.last_metadata = None;
        self.last_config = None;
        self.tracks.clear();
        self.errored = false;
    }

    /// Return the next event, or `undefined` if more data is needed.
    pub fn next_event(&mut self) -> Option<DemuxEvent> {
        if self.errored {
            return None;
        }

        match self.inner.next_event() {
            Ok(Some(AnnexbEvent::Track(track))) => {
                match track_event(
                    &mut self.arena,
                    track,
                    &mut self.last_config,
                    &mut self.tracks,
                ) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(AnnexbEvent::Packet(packet))) => {
                match packet_event(&mut self.arena, packet, &mut self.last_data, &self.tracks) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(AnnexbEvent::Metadata(items))) => {
                match metadata_event(&mut self.arena, items, &mut self.last_metadata) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(AnnexbEvent::Eof)) => Some(DemuxEvent::eof()),
            Ok(None) => None,
            Err(e) => {
                self.errored = true;
                let msg = e.to_string();
                Some(DemuxEvent::error(annexb_error_to_code(&e), &msg))
            }
        }
    }
}

/// WASM binding for the MPEG-PS demuxer.
#[wasm_bindgen]
pub struct MpegPsDemuxer {
    inner: InnerMpegPsDemuxer,
    arena: MemoryArena,
    last_data: Option<Handle>,
    last_metadata: Option<Handle>,
    last_config: Option<Handle>,
    tracks: BTreeMap<u32, TrackInfo>,
    errored: bool,
}

#[wasm_bindgen]
impl MpegPsDemuxer {
    /// Create a new MPEG-PS demuxer.
    ///
    /// `video_codec`: 0 = H.264, 1 = H.265.
    #[wasm_bindgen(constructor)]
    pub fn new(
        video_codec: u8,
        max_packet_size: u32,
        max_buffer: u32,
        max_nal_size: u32,
    ) -> Result<MpegPsDemuxer, JsValue> {
        let codec = codec_from_u8(video_codec).ok_or("unsupported video codec")?;
        let config = MpegPsConfig {
            video_codec: codec,
            max_packet_size_bytes: max_packet_size as usize,
            max_buffer_bytes: max_buffer as usize,
            max_nal_size_bytes: max_nal_size as usize,
        };
        Ok(Self {
            inner: InnerMpegPsDemuxer::new(config),
            arena: MemoryArena::new(next_instance_id()),
            last_data: None,
            last_metadata: None,
            last_config: None,
            tracks: BTreeMap::new(),
            errored: false,
        })
    }

    /// Push more MPEG-PS bytes.
    pub fn push(&mut self, data: &[u8]) {
        if !self.errored && !data.is_empty() {
            self.inner.push(data);
        }
    }

    /// Signal end of stream.
    pub fn end(&mut self) -> Result<(), JsValue> {
        self.inner.end().map_err(|e| {
            let msg = e.to_string();
            JsValue::from_str(&msg)
        })
    }

    /// Reset the demuxer and release all arena slots.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.arena = MemoryArena::new(next_instance_id());
        self.last_data = None;
        self.last_metadata = None;
        self.last_config = None;
        self.tracks.clear();
        self.errored = false;
    }

    /// Return the next event, or `undefined` if more data is needed.
    pub fn next_event(&mut self) -> Option<DemuxEvent> {
        if self.errored {
            return None;
        }

        match self.inner.next_event() {
            Ok(Some(MpegPsEvent::Track(track))) => {
                match track_event(
                    &mut self.arena,
                    track,
                    &mut self.last_config,
                    &mut self.tracks,
                ) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(MpegPsEvent::Packet(packet))) => {
                match packet_event(&mut self.arena, packet, &mut self.last_data, &self.tracks) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(MpegPsEvent::Metadata(items))) => {
                match metadata_event(&mut self.arena, items, &mut self.last_metadata) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(MpegPsEvent::Eof)) => Some(DemuxEvent::eof()),
            Ok(None) => None,
            Err(e) => {
                self.errored = true;
                let msg = e.to_string();
                Some(DemuxEvent::error(mpegps_error_to_code(&e), &msg))
            }
        }
    }
}
