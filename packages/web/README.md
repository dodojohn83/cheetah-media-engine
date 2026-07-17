# @cheetah-media/web

Public TypeScript SDK for the Cheetah media engine.

## Responsibility

- `Player` public API (load, play, pause, seek, setPlaybackRate, frameStep, pauseDisplay, ptz, stop, destroy).
- `GB28181 PtzCmd` encoder via `createGb28181PtzCmd`.
- Backend policy, events, and diagnostics surface.
- Delegate all WASM/DOM operations to `@cheetah-media/runtime`.
