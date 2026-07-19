//! WASM bindings for FLV/TS → fMP4 transmux.

use cheetah_container_flv::{FlvMode, FlvToFmp4Transmuxer, TransmuxError};
use cheetah_container_mpegts::{TsToFmp4Transmuxer, TsTransmuxError};
use js_sys::{Array, Uint8Array};
use wasm_bindgen::prelude::*;

fn js_err_flv(e: TransmuxError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn js_err_ts(e: TsTransmuxError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// One fMP4 segment ready for MSE `appendBuffer`.
#[wasm_bindgen]
pub struct Fmp4Segment {
    init: Option<Vec<u8>>,
    media: Option<Vec<u8>>,
    sequence: u32,
}

#[wasm_bindgen]
impl Fmp4Segment {
    #[wasm_bindgen(getter, js_name = hasInit)]
    pub fn has_init(&self) -> bool {
        self.init.is_some()
    }

    #[wasm_bindgen(getter, js_name = hasMedia)]
    pub fn has_media(&self) -> bool {
        self.media.is_some()
    }

    #[wasm_bindgen(getter)]
    pub fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Init segment bytes (`ftyp`+`moov`), or empty if unchanged.
    #[wasm_bindgen(js_name = initSegment)]
    pub fn init_segment(&self) -> Uint8Array {
        match &self.init {
            Some(b) => Uint8Array::from(b.as_slice()),
            None => Uint8Array::new_with_length(0),
        }
    }

    /// Media segment bytes (`moof`+`mdat`), or empty if none.
    #[wasm_bindgen(js_name = mediaSegment)]
    pub fn media_segment(&self) -> Uint8Array {
        match &self.media {
            Some(b) => Uint8Array::from(b.as_slice()),
            None => Uint8Array::new_with_length(0),
        }
    }
}

/// Incremental FLV → fragmented MP4 transmuxer for MSE.
#[wasm_bindgen]
pub struct FlvFmp4Transmuxer {
    inner: FlvToFmp4Transmuxer,
}

#[wasm_bindgen]
impl FlvFmp4Transmuxer {
    /// Create a new transmuxer.
    ///
    /// `mode`: 0 = Auto, 1 = File (PreviousTagSize), 2 = Stream (no PreviousTagSize).
    #[wasm_bindgen(constructor)]
    pub fn new(mode: u8) -> FlvFmp4Transmuxer {
        let flv_mode = match mode {
            1 => FlvMode::File,
            2 => FlvMode::Stream,
            _ => FlvMode::Auto,
        };
        Self {
            inner: FlvToFmp4Transmuxer::with_mode(flv_mode),
        }
    }

    /// Soft sample budget before forcing a media flush.
    #[wasm_bindgen(js_name = setMaxSamplesPerSegment)]
    pub fn set_max_samples_per_segment(&mut self, n: u32) {
        self.inner.set_max_samples_per_segment(n as usize);
    }

    /// Push FLV bytes.
    pub fn push(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.inner.push(data).map_err(js_err_flv)
    }

    /// Finish the stream and flush remaining samples.
    pub fn finish(&mut self) -> Result<(), JsValue> {
        self.inner.finish().map_err(js_err_flv)
    }

    /// Drain all ready segments into a JS array of `Fmp4Segment`.
    pub fn poll(&mut self) -> Array {
        let out = Array::new();
        while let Some(seg) = self.inner.poll() {
            out.push(&JsValue::from(Fmp4Segment {
                init: seg.init_segment,
                media: seg.media_segment,
                sequence: seg.sequence,
            }));
        }
        out
    }
}

/// Incremental MPEG-TS → fragmented MP4 transmuxer for MSE / HLS-TS.
#[wasm_bindgen]
pub struct TsFmp4Transmuxer {
    inner: TsToFmp4Transmuxer,
}

#[wasm_bindgen]
impl TsFmp4Transmuxer {
    /// Create a new MPEG-TS transmuxer.
    #[wasm_bindgen(constructor)]
    pub fn new() -> TsFmp4Transmuxer {
        Self {
            inner: TsToFmp4Transmuxer::new(),
        }
    }

    /// Soft sample budget before forcing a media flush.
    #[wasm_bindgen(js_name = setMaxSamplesPerSegment)]
    pub fn set_max_samples_per_segment(&mut self, n: u32) {
        self.inner.set_max_samples_per_segment(n as usize);
    }

    /// Push MPEG-TS bytes.
    pub fn push(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.inner.push(data).map_err(js_err_ts)
    }

    /// Finish the stream and flush remaining samples.
    pub fn finish(&mut self) -> Result<(), JsValue> {
        self.inner.finish().map_err(js_err_ts)
    }

    /// Drain all ready segments into a JS array of `Fmp4Segment`.
    pub fn poll(&mut self) -> Array {
        let out = Array::new();
        while let Some(seg) = self.inner.poll() {
            out.push(&JsValue::from(Fmp4Segment {
                init: seg.init_segment,
                media: seg.media_segment,
                sequence: seg.sequence,
            }));
        }
        out
    }
}

impl Default for TsFmp4Transmuxer {
    fn default() -> Self {
        Self::new()
    }
}
