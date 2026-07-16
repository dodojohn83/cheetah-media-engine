//! WebAssembly-facing G.711 A-law and mu-law encode helpers.

use cheetah_media_bitstream::{G711Kind, encode_buffer_f32};
use wasm_bindgen::prelude::*;

fn map_kind(kind: u8) -> Option<G711Kind> {
    match kind {
        0 => Some(G711Kind::ALaw),
        1 => Some(G711Kind::MuLaw),
        _ => None,
    }
}

/// Encode a buffer of 32-bit float PCM samples (nominal range [-1.0, 1.0]) to
/// 8-bit G.711 A-law (`kind = 0`) or mu-law (`kind = 1`).
///
/// Returns an empty vector for an unknown `kind`.
#[wasm_bindgen(js_name = g711EncodeF32)]
pub fn g711_encode_f32(kind: u8, samples: &[f32]) -> Vec<u8> {
    let Some(kind) = map_kind(kind) else {
        return Vec::new();
    };
    let mut output = vec![0u8; samples.len()];
    encode_buffer_f32(kind, samples, &mut output);
    output
}
