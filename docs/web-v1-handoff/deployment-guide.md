# Web SDK Deployment Guide

## Deployment patterns

### 1. Isolated mode (SharedArrayBuffer + threads + SIMD)

Serve the main document from a path that returns these headers:

```http
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: cross-origin
```

`apps/web-demo/scripts/preview.js` does this for `/isolated`. All subresources
(`worker.js`, `*.wasm`, module scripts) must be served with `COEP: require-corp`
and `CORP: cross-origin`.

HTML:

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
  </head>
  <body>
    <cheetah-player
      src="wss://example.com/live/stream.flv"
      autoplay
      muted
    ></cheetah-player>
    <script type="module" src="https://cdn.example.com/cheetah-media.iife.js"></script>
  </body>
</html>
```

### 2. Non-isolated mode (SIMD-only or baseline)

Do not send COOP/COEP. The SDK will detect `crossOriginIsolated === false` and
plan a fallback that avoids `SharedArrayBuffer`.

```html
<script src="https://cdn.example.com/cheetah-media.iife.js"></script>
<cheetah-player src="https://example.com/live/stream.m3u8"></cheetah-player>
```

### 3. Self-hosted build

Build the SDK from source and host worker + wasm on your own origin:

```bash
pnpm install --frozen-lockfile
pnpm build
```

Then set `assetBaseUrl` to the directory containing `worker.js` and `wasm/`:

```ts
import { createPlayer } from '@cheetah-media/web';

const player = createPlayer({
  runtime: { assetBaseUrl: '/assets/cheetah-media' },
});
await player.load('/live/stream.flv');
```

Expected files:

```
/assets/cheetah-media/worker.js
/assets/cheetah-media/wasm/cheetah_media_web_bindings.js
/assets/cheetah-media/wasm/cheetah_media_web_bindings_bg.wasm
```

### 4. CDN npm/UNPKG/JSDelivr

The npm packages expose IIFE bundles:

- `@cheetah-media/web` → `dist/cheetah-media.iife.js`
- `@cheetah-media/components` → `dist/cheetah-media-components.iife.js`

UNPKG/JSDelivr use the `unpkg` and `jsdelivr` fields in `package.json`.

```html
<script type="module">
  import { createPlayer } from 'https://cdn.jsdelivr.net/npm/@cheetah-media/web';
  const player = createPlayer({ runtime: { assetBaseUrl: 'https://cdn.jsdelivr.net/npm/@cheetah-media/web@0.1.0/dist' } });
  await player.load('https://example.com/live/stream.m3u8');
</script>
```

The `assetBaseUrl` must point to a directory where the worker and WASM bundle
are reachable; do not rely on the package root, which may not serve `.wasm` with
`application/wasm`.

## Codec and browser capability matrix

| Codec | WebCodecs | MSE | FFmpeg-WASM | Notes |
|-------|-----------|-----|-------------|-------|
| H.264 | Yes | Yes (avc1) | Yes | Best latency with WebCodecs. |
| H.265 | Chromium + flag | Yes (hev1) | Yes | WebCodecs support is limited. |
| AAC | Yes | Yes (mp4a) | Yes | |
| G.711A/U | Yes (decode) | No | Yes | MSE route typically transmuxes to AAC/Opus. |
| MP3 | No | Yes (audio/mpeg) | Yes | Use MSE for progressive MP3. |

| Browser | COOP/COEP | WebCodecs | SIMD | Threads | Recommended plan |
|---------|-----------|-----------|------|---------|------------------|
| Chrome 123+ | Yes | Yes | Yes | Yes | WebCodecs, threads+SIMD WASM |
| Firefox 124+ | No default | Partial | Yes | No | SIMD-only WASM or MSE |
| Safari 17+ | No default | Yes | Yes | No | WebCodecs or MSE |

## CSP example

```http
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'wasm-unsafe-inline' https://cdn.example.com;
  worker-src 'self' blob:;
  connect-src 'self' wss: https:;
  media-src 'self' blob:;
```

`wasm-unsafe-inline` may be required for WebAssembly `WebAssembly.instantiateStreaming`
in some CSP configurations. Use `script-src` nonces/hashes where possible.

## CORS/CORP for subresources

```http
# worker.js and JS modules
Cross-Origin-Resource-Policy: cross-origin
Cross-Origin-Embedder-Policy: require-corp

# wasm
Content-Type: application/wasm
Cross-Origin-Resource-Policy: cross-origin
Cross-Origin-Embedder-Policy: require-corp
```

## Common mistakes

1. **Wrong WASM MIME type.** Browsers will refuse to compile `.wasm` served as
   `application/octet-stream` when COEP/CSP are strict. Use `application/wasm`.
2. **Missing CORP on worker.** The worker must be served with `Cross-Origin-Resource-Policy:
   cross-origin` in isolated mode.
3. **COOP only on some pages.** Isolated mode requires the opener and the opened
   page to both carry compatible COOP/COEP headers.
4. **Mixed versions.** The worker, JS bundle and WASM bundle must come from the
   same SDK version. Use `assetBaseUrl` or a versioned CDN path.
