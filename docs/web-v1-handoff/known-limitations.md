# Web v1 Known Limitations

## 1. Browser MSE codec and protocol support boundaries

- **Impact:** Not all protocol/codec combinations produce successful MSE playback.
  HTTP-fMP4, WS-fMP4 (HTTP fallback) and HLS fMP4 with H.264 + AAC play successfully
  in Chromium. HTTP/WS-FLV cannot be played directly by MSE. H.265, MP3 and
  G.711 A-law/U-law in fMP4 are not accepted by Chromium's MSE `isTypeSupported`.
- **Workaround:** Use `tests/browser/tests/playback-matrix.spec.ts` to record
  per-browser support boundaries. For FLV or unsupported codecs, add a transmux
  path or use the FFmpeg-WASM codec pack once it is integrated.
- **Scope:** INT-002 functional acceptance; browser/codec/protocol matrix.
- **Issue:** Evidence collected in `testing/fixtures/evidence/` and
  `tests/browser/test-results/playback-matrix-*`.
- **Planned version:** Web v1.1 for FLV/TS transmux and expanded codec packs.

## 2. FFmpeg-WASM codec pack is currently a mock build

- **Impact:** Software decode/encode fallback and the threads+SIMD/SIMD/baseline
  matrix cannot be exercised end-to-end.
- **Workaround:** Use WebCodecs or MSE for supported codecs.
- **Scope:** Browsers where WebCodecs/MSE is unsupported or disabled.
- **Issue:** `codec-packs/ffmpeg-wasm/README.md` documents the full build.
- **Planned version:** Web v1.1 or v2 (FFmpeg source integration).

## 3. Hardware-bound performance and soak gates

- **Impact:** Latency, throughput, copy budget and soak metrics are measured on
  the CI/development VM only (`docs/web-v1-handoff/benchmark-report.md`).
  Target-device gates (PERF-001–005) are not yet met.
- **Workaround:** Run `cargo bench` and `scripts/run-acceptance.sh` on the
  representative deployment hardware, then compare against the VM baseline.
- **Scope:** Performance release gate.
- **Issue:** `cargo bench -p cheetah-media-types --features std` provides
  reproducible VM baseline data; device-specific soak tests are pending.
- **Planned version:** Web v1 RC.

## 4. `dodojohn83/cheetah-signaling` server facade is not yet integrated

- **Impact:** WP-15 (server facade migration) and live production deployment
  paths are not verified.
- **Workaround:** Use standalone media URLs or a different signaling server.
- **Scope:** Server-side media session control.
- **Issue:** TBD after `cheetah-signaling` codebase is ready.
- **Planned version:** Web v1.1.

## 5. Native/Desktop/Mobile SDKs are out of scope for Web v1

- **Impact:** Do not claim full Jessibuca Pro parity or Native client support.
- **Workaround:** Use the Web SDK in a WebView or browser.
- **Scope:** All non-Web platforms.
- **Issue:** Future backlog.
- **Planned version:** v2 or separate SDKs.

## 6. Bidirectional real-time (publish) is out of scope for Web v1

- **Impact:** The SDK is playback-only in v1.
- **Workaround:** Use a separate WebRTC/WS publisher.
- **Scope:** Camera control, talkback and upstream media.
- **Issue:** Future backlog.
- **Planned version:** v2.
