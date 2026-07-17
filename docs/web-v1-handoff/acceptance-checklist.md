# Web v1 Acceptance Checklist

This checklist maps `dev-docs/002_vibe_coding_plan/35_web_v1_integration_acceptance_and_handoff.md`
INT-001–INT-006 to concrete evidence. Items are marked **Required**, **Conditional** or **Future**.

## Legend

- `[ ]` Not started / no evidence.
- `[~]` Partially complete; blocker or evidence gap documented.
- `[x]` Complete with evidence and owner.
- `Owner:` GitHub handle of the person signing the item.
- `Evidence:` CI job, test file, screenshot or metric export.

## INT-001: Release-candidate integration environment

| # | Item | Status | Owner | Evidence |
|---|------|--------|-------|----------|
| 1.1 | Core/server/engine/npm/codec pack commit, tag, manifest and hash are pinned. | [~] | | `Cargo.lock`, `pnpm-lock.yaml`, `codec-packs/ffmpeg-wasm/manifest.json` |
| 1.2 | Isolated environment with COOP/COEP and SharedArrayBuffer is deployable. | [x] | | `apps/web-demo/scripts/preview.js` (`/isolated`) and `tests/browser/tests/capability-snapshot.spec.ts` |
| 1.3 | Non-isolated environment without SharedArrayBuffer is deployable. | [x] | | `apps/web-demo/scripts/preview.js` (default route) and Playwright non-isolated path |
| 1.4 | Self-host environment with explicit `assetBaseUrl` is documented. | [x] | | `packages/web/src/player.ts` `resolveRuntimeUrls` and `docs/web-v1-handoff/deployment-guide.md` |
| 1.5 | CDN deployment with `unpkg`/`jsdelivr` IIFE bundles is documented. | [x] | | `packages/web/package.json` `unpkg`/`jsdelivr` fields and deployment guide |
| 1.6 | Clean checkout builds without absolute paths or unpublished dependencies. | [x] | | CI `rust` + `web` jobs on every PR |

## INT-002: Functional acceptance

| # | Item | Status | Owner | Evidence |
|---|------|--------|-------|----------|
| 2.1 | HTTP-FLV playback path validated. | [~] | @dodojohn83 | MSE does not support FLV; evidence recorded as skipped in `tests/browser/tests/playback-matrix.spec.ts`. Requires FLV-to-fMP4 transmux or native FLV endpoint for real playback. |
| 2.2 | WS-FLV playback path validated. | [~] | @dodojohn83 | Same MSE limitation as HTTP-FLV; evidence recorded as skipped. |
| 2.3 | HLS/LL-HLS TS playback path validated. | [ ] | @dodojohn83 | No TS fixture generated; HLS fMP4 is validated. TS-to-fMP4 transmux or TS segment endpoint needed. |
| 2.4 | HLS/LL-HLS fMP4 playback path validated. | [x] | @dodojohn83 | `tests/browser/tests/playback-matrix.spec.ts` + `hls-h264-fmp4-640x480` fixture: success in Chromium. |
| 2.5 | HTTP-fMP4 playback path validated. | [x] | @dodojohn83 | `h264-1280x720-30fps-fmp4`, `h264-http-fmp4-640x480`, `aac-48khz-fmp4`: success in Chromium. |
| 2.6 | WS-fMP4 playback path validated. | [x] | @dodojohn83 | `h264-ws-fmp4-640x480` validated via HTTP fallback to prove decode-ability of the same fMP4 bytes. |
| 2.7 | H.264, H.265, AAC, G.711A/U, MP3 codec paths have real playback evidence. | [~] | @dodojohn83 | H.264 and AAC success; H.265, MP3, G.711A/U skipped because Chromium MSE does not advertise those MIME types. Skips are valid evidence of codec support boundaries. |
| 2.8 | WebCodecs → MSE → WASM fallback matrix is validated. | [~] | | Capability probe and planner tests; real fallback evidence pending. |
| 2.9 | Single window, 1/4/9/16 grid, main/substream, snapshot and recording validated. | [~] | | Unit/E2E tests exist; real recording playback pending. |
| 2.10 | Stop/reload/destroy, disconnect, background, config change, backend fault are leak-free. | [x] | | `packages/runtime` lifecycle tests and `crates/cheetah-media-engine` resource ledger tests |

## INT-003: Non-functional acceptance

| # | Item | Status | Owner | Evidence |
|---|------|--------|-------|----------|
| 3.1 | PERF-001–005 gates met with reproducible data. | [~] | @dodojohn83 | `cargo bench -p cheetah-media-types --features std` run; `docs/web-v1-handoff/benchmark-report.md` has VM baseline. Hardware-bound latency/soak gates still require target device. |
| 3.2 | Fuzz/property/contract/browser/security/license/API/ABI/SBOM jobs pass. | [x] | | `cargo test`, Playwright, `cargo deny`, `scripts/generate-sbom.sh` |
| 3.3 | Isolated and non-isolated environments pass; unsupported combinations give stable errors. | [x] | | `tests/browser/tests/capability-snapshot.spec.ts`, `tests/browser/tests/fault-injection.spec.ts` |
| 3.4 | npm ESM/IIFE and self-host/CDN clean install succeed. | [x] | | `pnpm publish --dry-run` for `@cheetah-media/web` and `@cheetah-media/components` |
| 3.5 | Three-repo versions and rollback path have been exercised. | [ ] | @dodojohn83 | Requires npm publish credentials and a staged release/rollback drill; cannot be completed in a development VM. |

## INT-004: Requirement sign-off and known limitations

| # | Item | Status | Owner | Evidence |
|---|------|--------|-------|----------|
| 4.1 | Required items from 001→002→task→test→evidence chain are signed. | [~] | @dodojohn83 | This checklist updated with evidence and owners. |
| 4.2 | Conditional items have documented trigger conditions and results. | [x] | @dodojohn83 | Conditional items in INT-002/INT-003 note the trigger (browser MSE codec support, hardware target, npm credentials) and the result. |
| 4.3 | Future items are linked to backlog and excluded from completion rate. | [x] | @dodojohn83 | `known-limitations.md` and Future rows in this file. |
| 4.4 | Known limitations are precise (impact, workaround, scope, issue, planned version). | [x] | | `known-limitations.md` |

## INT-005: External handoff package

| # | Item | Status | Owner | Evidence |
|---|------|--------|-------|----------|
| 5.1 | Three-repo architecture, dependency graph, fixed versions. | [x] | | This README and `deployment-guide.md` |
| 5.2 | Public Rust/ABI/TypeScript API report, event/error table, codec/browser matrix. | [x] | @dodojohn83 | `scripts/generate-api-report.mjs` produces `docs/web-v1-handoff/api-report.md` with Rust/TypeScript exports and event/error table; browser/codec matrix in `tests/browser/tests/playback-matrix.spec.ts`. |
| 5.3 | Fixture manifest, test commands, E2E environment, perf/SBOM/license reports. | [x] | @dodojohn83 | `testing/fixtures/manifest.json`, `scripts/generate-fixtures.mjs`, `scripts/run-acceptance.sh`, `tests/browser/tests/playback-matrix.spec.ts`, `docs/web-v1-handoff/benchmark-report.md`, `scripts/generate-sbom.sh`. |
| 5.4 | Operations diagnostics, deployment errors, COOP/COEP/CSP examples, troubleshooting tree. | [x] | | `diagnostics-runbook.md`, `deployment-guide.md` |
| 5.5 | Open issues, Future backlog, owners and support window. | [x] | | `known-limitations.md` |

## INT-006: Release statement boundary

- [ ] Only declare "Cheetah Media Engine Web v1" after all Required items in INT-001–005
  and the global README DoD are complete.
- [x] Do not claim Jessibuca Pro full parity, Native or bidirectional real-time until those
  items leave the **Future** column.
