# Changelog

## 0.1.0 (unreleased)

### Added

- Engine recovery, latency control and resource cleanup (WP-25).
- Web SDK public API, events and error mapping (WP-26):
  - `createPlayer(config?)` returning `CheetahPlayer`.
  - `CheetahMediaError` with stable `code`, `stage` and `recoverable` fields.
  - Config hierarchy covering transport, latency, backend, memory, render, audio, recording, security and diagnostics.
  - Event system with `statechange`, `tracks`, `firstframe`, `backendchange`, `variantchange`, `buffering`, `stats`, `warning`, `error` and `recording`.
  - Public methods: `load`, `play`, `pause`, `stop`, `destroy`, `snapshot`, `startRecording`, `stopRecording`, `switchVariant`, `getStats`, `exportDiagnostics`.
  - Runtime `request()` extension and new worker message types for future wiring.

### Notes

- `snapshot`, `startRecording`, `stopRecording` and `switchVariant` expose the public contract and forward requests to the runtime; the backend implementations remain in future work packages.
- Sensitive config fields (`security.token`, `security.credentials`, `transport.headers`) are redacted in `exportDiagnostics()` and `toJSON()`.
