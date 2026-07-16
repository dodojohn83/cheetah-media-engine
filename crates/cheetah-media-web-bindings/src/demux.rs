//! WASM bindings for Annex-B and MPEG-PS demuxers.
//!
//! These are thin wrappers around `cheetah_container_annexb` and
//! `cheetah_container_mpegps`. They use a per-instance `MemoryArena` so that
//! emitted payload and configuration bytes can be read directly from WASM
//! linear memory by the JavaScript runtime.

use std::sync::atomic::{AtomicU64, Ordering};

use cheetah_container_annexb::{AnnexBConfig, AnnexBDemuxer as InnerAnnexBDemuxer, AnnexbEvent};
use cheetah_container_mpegps::{MpegPsConfig, MpegPsDemuxer as InnerMpegPsDemuxer, MpegPsEvent};
use cheetah_media_abi::{AbiError, Handle, MemoryArena, MemoryDescriptor};
use cheetah_media_types::{CodecId, MediaPacket, MediaTime, TimeBase, TrackInfo, TrackKind};
use wasm_bindgen::prelude::*;

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

fn codec_to_u8(codec: CodecId) -> u8 {
    match codec {
        CodecId::H264 => 0,
        CodecId::H265 => 1,
        CodecId::Aac => 2,
        CodecId::G711A => 3,
        CodecId::G711U => 4,
        CodecId::Mp3 => 5,
        CodecId::Opus => 6,
        CodecId::PcmU8 => 7,
        CodecId::PcmS16 => 8,
        CodecId::Unknown(_) => 255,
    }
}

fn ticks_to_ms(time: MediaTime, ts: Option<cheetah_media_types::Timestamp>) -> i64 {
    let Some(ts) = ts else { return 0 };
    time.timebase
        .rescale_i64(ts.ticks(), TimeBase::DEFAULT)
        .unwrap_or(0)
}

fn write_track(
    arena: &mut MemoryArena,
    track: &TrackInfo,
) -> Result<(DemuxEvent, Option<Handle>), AbiError> {
    let (config_handle, config_desc) = if let Some(bytes) = track.codec_config.bytes() {
        let (handle, desc) = arena.store(bytes)?;
        let instance_id = arena.instance_id();
        (
            Some(Handle {
                instance_id,
                slot: handle.slot,
                generation: handle.generation,
            }),
            desc,
        )
    } else {
        (
            None,
            MemoryDescriptor {
                region: 0,
                offset: 0,
                length: 0,
                capacity: 0,
                generation: 0,
                flags: 0,
            },
        )
    };

    let (width, height) = track
        .video_format
        .map(|vf| (vf.visible_width, vf.visible_height))
        .unwrap_or((0, 0));
    let (sample_rate, channels) = track
        .audio_format
        .map(|af| (af.sample_rate, af.channel_layout.channels()))
        .unwrap_or((0, 0));

    let event = DemuxEvent {
        kind: DemuxEventKind::Track,
        track_id: track.id.get(),
        track_kind: match track.kind {
            TrackKind::Video => 0,
            TrackKind::Audio => 1,
            TrackKind::Data => 2,
        },
        codec: codec_to_u8(track.codec),
        width,
        height,
        sample_rate,
        channels,
        config_slot: config_handle.as_ref().map_or(0, |h| h.slot),
        config_generation: config_handle.as_ref().map_or(0, |h| h.generation),
        config_offset: config_desc.offset,
        config_len: if config_handle.is_some() {
            config_desc.length
        } else {
            0
        },
        data_slot: 0,
        data_generation: 0,
        data_offset: 0,
        data_len: 0,
        pts_ms: 0,
        dts_ms: 0,
        duration_ms: 0,
        flags: 0,
        error_code: 0,
        error_message: String::new(),
    };
    Ok((event, config_handle))
}

fn write_packet(
    arena: &mut MemoryArena,
    packet: &MediaPacket<'static>,
) -> Result<(DemuxEvent, Option<Handle>), AbiError> {
    let payload = packet.payload.as_ref();
    let (data_handle, data_desc) = if payload.is_empty() {
        (
            None,
            MemoryDescriptor {
                region: 0,
                offset: 0,
                length: 0,
                capacity: 0,
                generation: 0,
                flags: 0,
            },
        )
    } else {
        let (handle, desc) = arena.store(payload)?;
        let instance_id = arena.instance_id();
        (
            Some(Handle {
                instance_id,
                slot: handle.slot,
                generation: handle.generation,
            }),
            desc,
        )
    };

    let event = DemuxEvent {
        kind: DemuxEventKind::Packet,
        track_id: packet.track_id.get(),
        track_kind: 0,
        codec: 255,
        width: 0,
        height: 0,
        sample_rate: 0,
        channels: 0,
        config_slot: 0,
        config_generation: 0,
        config_offset: 0,
        config_len: 0,
        data_slot: data_handle.as_ref().map_or(0, |h| h.slot),
        data_generation: data_handle.as_ref().map_or(0, |h| h.generation),
        data_offset: data_desc.offset,
        data_len: if data_handle.is_some() {
            data_desc.length
        } else {
            0
        },
        pts_ms: ticks_to_ms(packet.time, packet.time.pts),
        dts_ms: ticks_to_ms(packet.time, packet.time.dts),
        duration_ms: ticks_to_ms(packet.time, packet.time.duration),
        flags: if packet.flags.is_keyframe { 1 } else { 0 },
        error_code: 0,
        error_message: String::new(),
    };
    Ok((event, data_handle))
}

fn track_event(
    arena: &mut MemoryArena,
    track: TrackInfo,
    last_config: &mut Option<Handle>,
) -> Result<DemuxEvent, AbiError> {
    if let Some(h) = last_config.take() {
        let _ = arena.release(h);
    }
    let (event, handle) = write_track(arena, &track)?;
    *last_config = handle;
    Ok(event)
}

fn packet_event(
    arena: &mut MemoryArena,
    packet: MediaPacket<'static>,
    last_data: &mut Option<Handle>,
) -> Result<DemuxEvent, AbiError> {
    if let Some(h) = last_data.take() {
        let _ = arena.release(h);
    }
    let (event, handle) = write_packet(arena, &packet)?;
    *last_data = handle;
    Ok(event)
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

/// Event kind returned by a demuxer.
#[wasm_bindgen]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemuxEventKind {
    Track = 0,
    Packet = 1,
    Eof = 2,
    Error = 3,
}

/// A demuxer event exposed to JavaScript.
///
/// The event references payload or configuration data through a
/// `MemoryDescriptor`-style address in the demuxer's own arena. Callers must
/// consume or copy the data before the next call to `next_event`.
#[wasm_bindgen]
#[derive(Clone)]
pub struct DemuxEvent {
    kind: DemuxEventKind,

    track_id: u32,
    track_kind: u8,
    codec: u8,
    width: u32,
    height: u32,
    sample_rate: u32,
    channels: u32,

    config_slot: u32,
    config_generation: u64,
    config_offset: u64,
    config_len: u32,

    data_slot: u32,
    data_generation: u64,
    data_offset: u64,
    data_len: u32,

    pts_ms: i64,
    dts_ms: i64,
    duration_ms: i64,
    flags: u32,

    error_code: u32,
    error_message: String,
}

#[wasm_bindgen]
impl DemuxEvent {
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> DemuxEventKind {
        self.kind
    }
    #[wasm_bindgen(getter)]
    pub fn track_id(&self) -> u32 {
        self.track_id
    }
    #[wasm_bindgen(getter)]
    pub fn track_kind(&self) -> u8 {
        self.track_kind
    }
    #[wasm_bindgen(getter)]
    pub fn codec(&self) -> u8 {
        self.codec
    }
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
    #[wasm_bindgen(getter)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    #[wasm_bindgen(getter)]
    pub fn channels(&self) -> u32 {
        self.channels
    }
    #[wasm_bindgen(getter)]
    pub fn config_slot(&self) -> u32 {
        self.config_slot
    }
    #[wasm_bindgen(getter)]
    pub fn config_generation(&self) -> u64 {
        self.config_generation
    }
    #[wasm_bindgen(getter)]
    pub fn config_offset(&self) -> u64 {
        self.config_offset
    }
    #[wasm_bindgen(getter)]
    pub fn config_len(&self) -> u32 {
        self.config_len
    }
    #[wasm_bindgen(getter)]
    pub fn data_slot(&self) -> u32 {
        self.data_slot
    }
    #[wasm_bindgen(getter)]
    pub fn data_generation(&self) -> u64 {
        self.data_generation
    }
    #[wasm_bindgen(getter)]
    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }
    #[wasm_bindgen(getter)]
    pub fn data_len(&self) -> u32 {
        self.data_len
    }
    #[wasm_bindgen(getter)]
    pub fn pts_ms(&self) -> i64 {
        self.pts_ms
    }
    #[wasm_bindgen(getter)]
    pub fn dts_ms(&self) -> i64 {
        self.dts_ms
    }
    #[wasm_bindgen(getter)]
    pub fn duration_ms(&self) -> i64 {
        self.duration_ms
    }
    #[wasm_bindgen(getter)]
    pub fn flags(&self) -> u32 {
        self.flags
    }
    #[wasm_bindgen(getter)]
    pub fn error_code(&self) -> u32 {
        self.error_code
    }
    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> String {
        self.error_message.clone()
    }

    fn eof() -> Self {
        let mut e = Self::empty();
        e.kind = DemuxEventKind::Eof;
        e
    }

    fn error(code: u32, message: &str) -> Self {
        let mut e = Self::empty();
        e.kind = DemuxEventKind::Error;
        e.error_code = code;
        e.error_message = message.into();
        e
    }

    fn empty() -> Self {
        Self {
            kind: DemuxEventKind::Error,
            track_id: 0,
            track_kind: 0,
            codec: 255,
            width: 0,
            height: 0,
            sample_rate: 0,
            channels: 0,
            config_slot: 0,
            config_generation: 0,
            config_offset: 0,
            config_len: 0,
            data_slot: 0,
            data_generation: 0,
            data_offset: 0,
            data_len: 0,
            pts_ms: 0,
            dts_ms: 0,
            duration_ms: 0,
            flags: 0,
            error_code: 0,
            error_message: String::new(),
        }
    }
}

/// WASM binding for the Annex-B H.264/H.265 demuxer.
#[wasm_bindgen]
pub struct AnnexBDemuxer {
    inner: InnerAnnexBDemuxer,
    arena: MemoryArena,
    last_data: Option<Handle>,
    last_config: Option<Handle>,
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
            last_config: None,
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
        self.last_config = None;
        self.errored = false;
    }

    /// Return the next event, or `undefined` if more data is needed.
    pub fn next_event(&mut self) -> Option<DemuxEvent> {
        if self.errored {
            return None;
        }

        match self.inner.next_event() {
            Ok(Some(AnnexbEvent::Track(track))) => {
                match track_event(&mut self.arena, track, &mut self.last_config) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(AnnexbEvent::Packet(packet))) => {
                match packet_event(&mut self.arena, packet, &mut self.last_data) {
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
    last_config: Option<Handle>,
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
            last_config: None,
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
        self.last_config = None;
        self.errored = false;
    }

    /// Return the next event, or `undefined` if more data is needed.
    pub fn next_event(&mut self) -> Option<DemuxEvent> {
        if self.errored {
            return None;
        }

        match self.inner.next_event() {
            Ok(Some(MpegPsEvent::Track(track))) => {
                match track_event(&mut self.arena, track, &mut self.last_config) {
                    Ok(event) => Some(event),
                    Err(e) => {
                        self.errored = true;
                        Some(DemuxEvent::error(9001, e.as_str()))
                    }
                }
            }
            Ok(Some(MpegPsEvent::Packet(packet))) => {
                match packet_event(&mut self.arena, packet, &mut self.last_data) {
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
