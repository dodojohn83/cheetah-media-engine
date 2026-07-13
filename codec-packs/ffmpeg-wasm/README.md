# cheetah-ffmpeg-wasm

Optional FFmpeg codec pack for the Cheetah Media Engine.

## Scope

- Provides H.264, H.265, AAC, and MP3 decoding when the browser does not offer a suitable WebCodecs/MSE path.
- Built with FFmpeg 8.1.2 as an Emscripten 6.0.2 WebAssembly module.
- Disabled `--enable-gpl` and `--enable-nonfree`; only LGPL decoder/util/resampler components are enabled.
- G.711 A/U is implemented in pure Rust and is not included in the FFmpeg pack.

## Independence

The pack is downloaded separately, has its own manifest, and can be removed or replaced by the user. The SDK does not statically depend on it.

## Build

See `manifest.json` for FFmpeg configure flags and `scripts/build.sh` for the Emscripten build.
