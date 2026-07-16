# @cheetah-media/web

Public TypeScript SDK for the Cheetah media engine.

## Responsibility

- `Player` public API (load, play, pause, seek, setPlaybackRate, stop, destroy).
- Backend policy, events, and diagnostics surface.
- Delegate all WASM/DOM operations to `@cheetah-media/runtime`.
