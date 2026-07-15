# @cheetah-media/web Public API Report

## Exports

### Functions

- `createPlayer(config?: PlayerConfig): CheetahPlayer` — create a new player instance without starting network activity.
- `createPlayerWithRuntime(config, factory)` — internal test factory to inject a mock runtime.

### Classes

- `CheetahMediaError` — public error class with `code`, `stage`, `recoverable` and `message`. `toJSON()` excludes the cause chain for safety.

### Types

- `CheetahPlayer` — public player contract.
- `PlayerConfig` / `TransportConfig` / `LatencyConfig` / `BackendConfig` / `MemoryConfig` / `RenderConfig` / `AudioConfig` / `RecordingConfig` / `SecurityConfig` / `DiagnosticsConfig` — configuration hierarchy.
- `CheetahPlayerEventType` / `CheetahPlayerEvent` / `EventListener` — event system.
- `PlayerState` / `PlayerStats` / `DiagnosticsSnapshot` — runtime state and diagnostics.
- `MemoryDescriptor` / `PacketDescriptor` / `FrameDescriptor` / `AbiFeatureFlags` — stable ABI descriptors re-exported from the runtime.

## Methods

- `load(url, options?)`
- `play()`
- `pause()`
- `stop()`
- `destroy()`
- `snapshot(options?)`
- `startRecording(options?)`
- `stopRecording()`
- `switchVariant({ bandwidth?, index? })`
- `getStats()`
- `exportDiagnostics()`
- `addEventListener(type, listener)`
- `removeEventListener(type, listener)`

## Events

All events carry `playerId`, `epoch`, `sequence` and `timestamp`.

- `statechange`
- `tracks`
- `firstframe`
- `backendchange`
- `variantchange`
- `buffering`
- `stats` (throttled by `diagnostics.statsIntervalMs`)
- `warning`
- `error`
- `recording`

## Version

Current public API version follows the package version (`0.1.0`).
