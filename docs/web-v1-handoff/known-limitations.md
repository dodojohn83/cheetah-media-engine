# Web v1 Known Limitations

## 1. Real encoded media playback evidence is pending

- **Impact:** INT-002 functional acceptance (HTTP/WS-FLV, HLS/LL-HLS, fMP4,
  codec playback matrix) cannot be signed off in CI.
- **Workaround:** Run `scripts/integration-smoke.sh` against a staging server
  with real media endpoints.
- **Scope:** All browser/codec/protocol combinations.
- **Issue:** TBD after media endpoints are provisioned.
- **Planned version:** Web v1 RC or v1.1.

## 2. FFmpeg-WASM codec pack is currently a mock build

- **Impact:** Software decode/encode fallback and the threads+SIMD/SIMD/baseline
  matrix cannot be exercised end-to-end.
- **Workaround:** Use WebCodecs or MSE for supported codecs.
- **Scope:** Browsers where WebCodecs/MSE is unsupported or disabled.
- **Issue:** `codec-packs/ffmpeg-wasm/README.md` documents the full build.
- **Planned version:** Web v1.1 or v2 (FFmpeg source integration).

## 3. PERF-001–005 hardware-bound benchmarks cannot run in CI

- **Impact:** Latency, throughput, copy budget and soak metrics are validated by
  unit/integration tests only, not on target hardware.
- **Workaround:** Run `cargo bench` and `scripts/integration-smoke.sh` on a
  representative device.
- **Scope:** Performance release gate.
- **Issue:** TBD.
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
