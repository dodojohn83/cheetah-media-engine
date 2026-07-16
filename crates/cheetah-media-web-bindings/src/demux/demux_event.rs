//! JavaScript-facing event type for the WASM demuxer bindings.

use wasm_bindgen::prelude::*;

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
    pub(crate) kind: DemuxEventKind,

    pub(crate) track_id: u32,
    pub(crate) track_kind: u8,
    pub(crate) codec: u8,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u32,

    pub(crate) config_slot: u32,
    pub(crate) config_generation: u64,
    pub(crate) config_offset: u64,
    pub(crate) config_len: u32,

    pub(crate) data_slot: u32,
    pub(crate) data_generation: u64,
    pub(crate) data_offset: u64,
    pub(crate) data_len: u32,

    pub(crate) pts_ms: i64,
    pub(crate) dts_ms: i64,
    pub(crate) duration_ms: i64,
    pub(crate) flags: u32,

    pub(crate) error_code: u32,
    pub(crate) error_message: String,
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

    pub(crate) fn eof() -> Self {
        let mut e = Self::empty();
        e.kind = DemuxEventKind::Eof;
        e
    }

    pub(crate) fn error(code: u32, message: &str) -> Self {
        let mut e = Self::empty();
        e.kind = DemuxEventKind::Error;
        e.error_code = code;
        e.error_message = message.into();
        e
    }

    pub(crate) fn empty() -> Self {
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
