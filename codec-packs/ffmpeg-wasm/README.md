# cheetah-ffmpeg-wasm

Optional LGPL FFmpeg WASM codec pack for the Cheetah Media Engine.

## Scope

- Provides H.264, H.265, AAC and MP3 decoding when the browser does not offer a
  suitable WebCodecs/MSE path.
- Built from FFmpeg 8.1.2 using Emscripten 6.0.2.
- Disabled `--enable-gpl` and `--enable-nonfree`; only LGPL
  decoder/parser/resampler/util components are enabled.
- G.711 A/U is implemented in pure Rust and is not part of this pack.

## Variants

| variant        | requirements                              | use case                         |
| -------------- | ----------------------------------------- | -------------------------------- |
| `baseline`     | WebAssembly only                          | maximum compatibility            |
| `simd`         | WebAssembly SIMD128                       | faster software decode           |
| `threads-simd` | COOP/COEP, SharedArrayBuffer, Atomics     | multi-core, highest throughput   |

All variants share the same stable C ABI and manifest so the JS loader does not
need to know FFmpeg internals.

## Build

```bash
# Build the baseline mock pack (default for the build-system PR)
pnpm build

# Build all variants with the real FFmpeg decoder shim (slow)
FFMPEG=1 pnpm build:all
```

The build script checks the FFmpeg 8.1.2 source archive against the recorded
SHA-256, runs `configure` with the flags listed in `manifest.json`, and writes
`dist/cheetah_ffmpeg.{variant}.{js,wasm}` plus `dist/offer-{variant}.json`.

## Split PR note

The current PR contains the build system, manifest, ABI header, JS loader and a
mock pack that returns `UNSUPPORTED` for all decode operations. The real FFmpeg
decoder shim will be integrated in a follow-up PR.
