# Migration Inventory

## Scope

This repository is the single monorepo for `cheetah-media-engine` (Web v1) and `cheetah-media-core` crates. The original `cheetah-media-core-rs` and `cheetah-media-server-rs` repositories are not present, so this inventory is scoped to the crates in `crates/` and the codec pack in `codec-packs/`.

## Crate inventory

| crate | owner | purpose | Move/Adapt/Keep/Replace | target crate |
|-------|-------|---------|--------------------------|--------------|
| `cheetah-media-types` | core | media types, codec, track, timestamp | Keep | - |
| `cheetah-media-bitstream` | core | byte reader for containers | Keep | - |
| `cheetah-container-flv` | core | FLV parser | Keep | - |
| `cheetah-container-mpegts` | core | MPEG-TS parser | Keep | - |
| `cheetah-container-isobmff` | core | ISOBMFF/fMP4/MP4 box parser | Keep | - |
| `cheetah-media-timeline` | core | timeline and synchronization | Keep | - |
| `cheetah-media-abi` | core | platform-neutral ports | Keep | - |
| `cheetah-media-pipeline-core` | core | pipeline scheduler | Keep | - |
| `cheetah-hls-client` | core | HLS/LL-HLS playlist client | Keep | - |
| `cheetah-media-core` | core | facade crate | Keep | - |
| `cheetah-media-backend-api` | engine | backend capability probe | Keep | - |
| `cheetah-media-engine` | engine | engine orchestration | Keep | - |
| `cheetah-media-web-bindings` | engine | WASM bindings | Keep | - |
| `cheetah-media-testkit` | engine | fixtures, testkit, contract harness | Keep | - |

## Server-side code

Server-side session, driver, module, HTTP handler and signaling logic remain in `cheetah-signaling` and `cheetah-media-server` (not this repo). The engine repo only extracts and re-implements pure container/media capabilities, per MIG-001.

## Dependencies

- `serde` and `serde_json` were added for the fixture manifest and testkit. Both are MIT/Apache-2.0, no_std-capable (with `serde` `default-features = false`), and required by the manifest schema.
- FFmpeg is not linked by any Rust crate; the `codec-packs/ffmpeg-wasm` manifest is separate and downloaded on demand.

## Fixtures

Fixture metadata is centralized in `testing/fixtures/manifest.json`. Large binary fixtures are out-of-tree; the manifest records source URL, hash, license, protocol, codec, and expected output.

## License

Project code is `MIT OR Apache-2.0`. Third-party dependencies are tracked in `Cargo.lock` and audited by `cargo deny`.
