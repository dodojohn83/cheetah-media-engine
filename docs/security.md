# Security and Privacy Guide

This document describes the security boundaries, recommended headers, content
security policy and supply-chain controls for `@cheetah-media/web`.

## Cross-Origin Isolation (COOP/COEP/CORP)

`SharedArrayBuffer`, high-resolution `performance.now()` and some WebCodecs
paths require the page to be cross-origin isolated. The player does not force
isolation; it falls back automatically when isolation is unavailable.

To enable full isolation, serve the application with:

```http
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Resource-Policy: same-origin
```

When isolation cannot be enabled, the player:

- avoids `SharedArrayBuffer` and uses `postMessage`/`ArrayBuffer` transfers,
- disables WebCodecs features that require cross-origin isolation,
- logs a `warning` event with `isolation: false`.

## Content Security Policy

A restrictive CSP that supports the player, the worker and WASM codec packs:

```http
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' blob:;
  worker-src 'self' blob:;
  connect-src 'self' https: wss:;
  media-src 'self' blob:;
  object-src 'none';
  base-uri 'self';
  form-action 'none';
```

Notes:

- `'wasm-unsafe-eval'` is required for WebAssembly codec packs.
- `blob:` is required for module workers and inline worker bootstrap.
- `'unsafe-inline'` is only needed when the application injects an inline
  worker bootstrap script; the player itself does not use `eval` or inline
  scripts.
- No `data:` sources are required for the player core.

## Worker and WASM loading

The runtime loader validates codec pack resources before instantiating them:

- HTTPS or same-origin is required for worker and WASM URLs in production.
- The manifest ABI major version and the loader ABI major version must match;
  a mismatch produces an `abi-mismatch` error and refuses to load the pack.
- Subresource Integrity (`integrity`) is recommended for the `.wasm` and `.js`
  artifacts; the loader verifies the digest before initializing the module.
- MIME types are checked: `application/wasm` for `.wasm` and
  `text/javascript` / `application/javascript` for `.js`.

## Credential and URL redaction

`exportDiagnostics()` and all public `toJSON()` methods redact:

- `Authorization` and `Cookie` headers,
- custom secret headers such as `X-Api-Key` and `X-Auth-Token`,
- `token`, `secret`, `credential`, `password` and `apiKey` fields,
- query strings, fragments and userinfo from URLs.

The redacted bundle keeps only the origin and pathname of source URLs so that
support engineers can identify endpoints without receiving access tokens.

## Supply chain

- Rust crates are checked by `cargo deny` for license compatibility and advisory
  status in CI.
- JavaScript lock files (`pnpm-lock.yaml`) must be present and frozen in CI.
- FFmpeg source tarballs are pinned by SHA-256 in `codec-packs/ffmpeg-wasm`.
- SBOM and `NOTICE` files are produced alongside each codec pack release.

## Reporting

Security issues can be reported by opening a confidential issue in the
repository. Do not include credentials, tokens or media payloads in issue
attachments.
