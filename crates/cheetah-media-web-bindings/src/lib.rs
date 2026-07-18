//! WebAssembly bindings for the Cheetah media engine.
//!
//! This crate exposes a small JS-facing API on top of the platform-neutral
//! engine. Unsafe code is only allowed inside audited FFI shim modules.

use cheetah_media_abi::{AbiError, Handle, MemoryArena, MemoryDescriptor as AbiMemoryDescriptor};
use cheetah_media_engine::VERSION;
use js_sys::Error as JsError;
use wasm_bindgen::prelude::*;

mod demux;
mod g711;

pub use demux::{AnnexBDemuxer, DemuxEvent, DemuxEventKind, MpegPsDemuxer};
pub use g711::g711_encode_f32;

const INSTANCE_ID: u64 = 1;

/// Turn a stable `AbiError` into a JavaScript `Error`.
fn js_error(e: AbiError) -> JsValue {
    let msg = format!("{} (code={})", e.as_str(), e.as_u32());
    JsError::new(&msg).into()
}

/// Initialize the WASM module and panic hook.
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Return the engine version string.
#[wasm_bindgen]
pub fn engine_version() -> String {
    VERSION.to_string()
}

/// Return the name of a codec by its discriminant index.
#[wasm_bindgen]
pub fn codec_name(codec_index: u8) -> String {
    match codec_index {
        0 => "h264".to_string(),
        1 => "h265".to_string(),
        2 => "aac".to_string(),
        3 => "g711a".to_string(),
        4 => "g711u".to_string(),
        5 => "mp3".to_string(),
        _ => "unknown".to_string(),
    }
}

/// JS-facing descriptor for a writable or readable memory region.
#[wasm_bindgen]
pub struct MemoryDescriptor {
    slot: u32,
    generation: u64,
    offset: u32,
    length: u32,
    capacity: u32,
    flags: u32,
}

#[wasm_bindgen]
impl MemoryDescriptor {
    #[wasm_bindgen(getter)]
    pub fn slot(&self) -> u32 {
        self.slot
    }

    #[wasm_bindgen(getter)]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[wasm_bindgen(getter)]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> u32 {
        self.length
    }

    #[wasm_bindgen(getter)]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    #[wasm_bindgen(getter)]
    pub fn flags(&self) -> u32 {
        self.flags
    }
}

impl MemoryDescriptor {
    fn from_abi(handle: Handle, desc: AbiMemoryDescriptor) -> Self {
        Self {
            slot: handle.slot,
            generation: handle.generation,
            offset: desc.offset as u32,
            length: desc.length,
            capacity: desc.capacity,
            flags: desc.flags,
        }
    }
}

fn make_handle(slot: u32, generation: u64) -> Handle {
    Handle {
        instance_id: INSTANCE_ID,
        slot,
        generation,
    }
}

/// Web-facing engine context.
///
/// The control surface is exported here: create, configure, load, play, pause,
/// push, poll, release, stop and destroy. Payloads live in a `MemoryArena` and
/// are passed by descriptor so that JS never caches raw pointers across memory
/// growth.
#[wasm_bindgen]
pub struct WebEngine {
    arena: MemoryArena,
    configured: bool,
    loaded_url: Option<String>,
    playing: bool,
    seek_target_ms: Option<u64>,
    playback_rate: f64,
    frame_step_direction: Option<String>,
    frame_step_keyframe_only: bool,
    pause_display_keep_connection: bool,
}

impl Default for WebEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WebEngine {
    /// Create a new engine context.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            arena: MemoryArena::new(INSTANCE_ID),
            configured: false,
            loaded_url: None,
            playing: false,
            seek_target_ms: None,
            playback_rate: 1.0,
            frame_step_direction: None,
            frame_step_keyframe_only: false,
            pause_display_keep_connection: false,
        }
    }

    /// Return the engine version string.
    #[wasm_bindgen(js_name = version)]
    pub fn js_version(&self) -> String {
        VERSION.to_string()
    }

    /// Apply configuration. Currently accepts the string and marks the engine as
    /// configured; detailed parsing will come with the backend ports.
    pub fn configure(&mut self, _json: &str) -> Result<(), JsValue> {
        self.configured = true;
        Ok(())
    }

    /// Load a media URL and prepare the engine for playback.
    ///
    /// `is_live` is stored for later transport selection. The URL is validated
    /// but actual network/demux integration is added in later tasks.
    pub fn load(&mut self, url: &str, is_live: bool) -> Result<(), JsValue> {
        if url.is_empty()
            || url
                .split_once("://")
                .is_none_or(|(scheme, _)| scheme.is_empty())
        {
            return Err(js_error(AbiError::InvalidData));
        }
        // Insert the `live=` query before any URL fragment so the fragment is
        // not overwritten and the query is appended to the path/query portion.
        let (base, fragment) = url.split_once('#').unwrap_or((url, ""));
        let sep = if base.contains('?') { '&' } else { '?' };
        let full = if fragment.is_empty() {
            format!("{base}{sep}live={is_live}")
        } else {
            format!("{base}{sep}live={is_live}#{fragment}")
        };
        self.loaded_url = Some(full);
        self.playing = false;
        Ok(())
    }

    /// Start playback.
    pub fn play(&mut self) -> Result<(), JsValue> {
        if self.loaded_url.is_none() {
            return Err(js_error(AbiError::InvalidData));
        }
        self.playing = true;
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&mut self) -> Result<(), JsValue> {
        self.playing = false;
        Ok(())
    }

    /// Seek to a target time in milliseconds.
    ///
    /// The target is stored for later integration with the pipeline; this
    /// control surface call is validated but does not itself decode media.
    pub fn seek(&mut self, time_ms: u64) -> Result<(), JsValue> {
        self.seek_target_ms = Some(time_ms);
        Ok(())
    }

    /// Set the desired playback rate.
    ///
    /// Supported range is 0.1x to 16x. The value is stored for the backend
    /// scheduler; this call does not change browser video playback rate directly.
    pub fn set_playback_rate(&mut self, rate: f64) -> Result<(), JsValue> {
        if !rate.is_finite() || !(0.1..=16.0).contains(&rate) {
            return Err(js_error(AbiError::InvalidData));
        }
        self.playback_rate = rate;
        Ok(())
    }

    /// Request a single frame step in the given direction.
    ///
    /// Valid directions are "forward" and "backward". The request is stored
    /// for the backend scheduler; this call does not render frames itself.
    pub fn frame_step(&mut self, direction: &str, keyframe_only: bool) -> Result<(), JsValue> {
        if direction != "forward" && direction != "backward" {
            return Err(js_error(AbiError::InvalidData));
        }
        self.frame_step_direction = Some(direction.to_string());
        self.frame_step_keyframe_only = keyframe_only;
        Ok(())
    }

    /// Pause display while optionally keeping the network/decoder connection.
    ///
    /// When `keep_connection` is true the backend freezes output without
    /// tearing down the decoder or transport. This call only records the state.
    pub fn pause_display(&mut self, keep_connection: bool) -> Result<(), JsValue> {
        self.playing = false;
        self.pause_display_keep_connection = keep_connection;
        Ok(())
    }

    /// Return whether the engine is currently playing.
    #[wasm_bindgen(getter)]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Request a writable region of `size` bytes.
    pub fn request_write_region(&mut self, size: u32) -> Result<MemoryDescriptor, JsValue> {
        let (handle, desc) = self.arena.request(size as usize).map_err(js_error)?;
        Ok(MemoryDescriptor::from_abi(handle, desc))
    }

    /// Commit the first `len` bytes of a previously requested region.
    pub fn commit_region(&mut self, slot: u32, generation: u64, len: u32) -> Result<(), JsValue> {
        self.arena
            .commit(make_handle(slot, generation), len as usize)
            .map_err(js_error)
    }

    /// Release a region back to the engine.
    pub fn release_region(&mut self, slot: u32, generation: u64) -> Result<(), JsValue> {
        self.arena
            .release(make_handle(slot, generation))
            .map_err(js_error)
    }

    /// Push a compressed packet described by the region `slot`/`generation`.
    ///
    /// The handle is validated but decoding is not yet implemented for all
    /// codecs; unsupported paths return `AbiError::NotSupported`.
    #[allow(clippy::too_many_arguments)]
    pub fn push_packet(
        &mut self,
        slot: u32,
        generation: u64,
        _track_id: u32,
        _pts_ms: i64,
        _dts_ms: i64,
        _duration_ms: i64,
        _flags: u32,
    ) -> Result<(), JsValue> {
        // Validate the handle and release the region so the caller does not
        // leak the committed slot while decoding is still unsupported.
        let handle = make_handle(slot, generation);
        self.arena.read(handle).map_err(js_error)?;
        let _ = self.arena.release(handle);
        Err(js_error(AbiError::NotSupported))
    }

    /// Poll for a decoded output region.
    ///
    /// Returns `NotSupported` until a decoder backend is attached.
    pub fn poll_output(&mut self) -> Result<Option<MemoryDescriptor>, JsValue> {
        Err(js_error(AbiError::NotSupported))
    }

    /// Stop the engine, releasing all borrowed regions and clearing the active
    /// media URL so a new load is required before playback can resume.
    pub fn stop(&mut self) -> Result<(), JsValue> {
        self.arena = MemoryArena::new(INSTANCE_ID);
        self.loaded_url = None;
        self.playing = false;
        self.seek_target_ms = None;
        self.playback_rate = 1.0;
        self.frame_step_direction = None;
        self.frame_step_keyframe_only = false;
        self.pause_display_keep_connection = false;
        Ok(())
    }

    /// Destroy the engine context and release all resources.
    pub fn destroy(self) {
        drop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_name_unknown_for_high_index() {
        assert_eq!(codec_name(255), "unknown");
    }

    #[test]
    fn webengine_seek_and_playback_rate_store_state() {
        let mut engine = WebEngine::new();
        engine.load("http://example.com/test.m3u8", false).unwrap();
        engine.seek(12345).unwrap();
        engine.set_playback_rate(2.0).unwrap();
        assert_eq!(engine.seek_target_ms, Some(12345));
        assert!((engine.playback_rate - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn webengine_stop_resets_seek_and_rate() {
        let mut engine = WebEngine::new();
        engine.load("http://example.com/test.m3u8", false).unwrap();
        engine.seek(12345).unwrap();
        engine.set_playback_rate(2.0).unwrap();
        engine.stop().unwrap();
        assert_eq!(engine.seek_target_ms, None);
        assert!((engine.playback_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn webengine_frame_step_and_pause_display_store_state() {
        let mut engine = WebEngine::new();
        engine.load("http://example.com/test.m3u8", false).unwrap();
        engine.frame_step("forward", true).unwrap();
        assert_eq!(engine.frame_step_direction.as_deref(), Some("forward"));
        assert!(engine.frame_step_keyframe_only);
        engine.pause_display(true).unwrap();
        assert!(!engine.is_playing());
        assert!(engine.pause_display_keep_connection);
    }

    #[test]
    fn webengine_stop_resets_frame_step_and_pause_display() {
        let mut engine = WebEngine::new();
        engine.load("http://example.com/test.m3u8", false).unwrap();
        engine.frame_step("backward", false).unwrap();
        engine.pause_display(true).unwrap();
        engine.stop().unwrap();
        assert_eq!(engine.frame_step_direction, None);
        assert!(!engine.pause_display_keep_connection);
    }

    #[test]
    fn memory_descriptor_from_abi_round_trips() {
        let handle = Handle {
            instance_id: INSTANCE_ID,
            slot: 7,
            generation: 42,
        };
        let desc = AbiMemoryDescriptor {
            region: 0,
            offset: 1024,
            length: 16,
            capacity: 32,
            generation: 42,
            flags: 0,
        };
        let wrapped = MemoryDescriptor::from_abi(handle, desc);
        assert_eq!(wrapped.slot, 7);
        assert_eq!(wrapped.generation, 42);
        assert_eq!(wrapped.offset, 1024);
        assert_eq!(wrapped.length, 16);
        assert_eq!(wrapped.capacity, 32);
    }

    #[test]
    fn load_appends_live_before_fragment() {
        let mut engine = WebEngine::new();
        engine
            .load("http://example.com/test.m3u8#t=10", false)
            .unwrap();
        assert_eq!(
            engine.loaded_url,
            Some("http://example.com/test.m3u8?live=false#t=10".to_string())
        );
    }

    // These tests exercise error paths that create JS Error objects, which
    // requires a wasm32 runtime. They are compiled on wasm32 but skipped on
    // host targets where js_sys functions panic.
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn load_rejects_empty_scheme() {
        let mut engine = WebEngine::new();
        assert!(engine.load("://example.com/test.m3u8", false).is_err());
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn push_packet_releases_committed_region() {
        let mut engine = WebEngine::new();
        let desc = engine.request_write_region(4).unwrap();
        engine
            .commit_region(desc.slot(), desc.generation(), 4)
            .unwrap();
        assert!(
            engine
                .push_packet(desc.slot(), desc.generation(), 1, 0, 0, 0, 0)
                .is_err()
        );
        // The region should be released after the unsupported push.
        assert_eq!(engine.arena.occupied_count(), 0);
    }
}
