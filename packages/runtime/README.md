# @cheetah-media/runtime

TypeScript runtime layer that owns the Web Worker and WASM module lifecycle.

## Responsibility

- Load and cache WASM modules.
- Spawn and manage workers.
- Expose a stable public API to `@cheetah-media/web`.
