//! WebAssembly bindings for the Cheetah media engine.
//!
//! This crate exposes a small JS-facing API on top of the platform-neutral
//! engine. Unsafe code is only allowed inside audited FFI shim modules.

use cheetah_media_engine::VERSION;
use wasm_bindgen::prelude::*;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_name_unknown_for_high_index() {
        assert_eq!(codec_name(255), "unknown");
    }
}
