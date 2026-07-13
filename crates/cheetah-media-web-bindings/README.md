# cheetah-media-web-bindings

WebAssembly bindings for the Cheetah media engine.

## Responsibility

- Expose a stable JS-facing API via `wasm-bindgen`.
- Bridge engine events and metrics to TypeScript runtime.

## Constraints

- `unsafe_code` is denied; audited FFI shims may use `unsafe` locally with Safety comments.
- Build target: `wasm32-unknown-unknown`.
