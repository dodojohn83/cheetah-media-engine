# Cheetah Media Engine — Web v1 Integration & Handoff

This directory contains the Web v1 integration acceptance package and runbooks.
It is intended for operators, integrators and maintainers who need to deploy,
validate and support the Web SDK in production.

## Contents

| Document | Purpose |
|----------|---------|
| [acceptance-checklist.md](./acceptance-checklist.md) | Required vs. Conditional vs. Future acceptance items, with evidence links and owners. |
| [deployment-guide.md](./deployment-guide.md) | Isolated, non-isolated, self-host and CDN deployment patterns, plus COOP/COEP/CSP templates. |
| [rollback-guide.md](./rollback-guide.md) | Core tag, npm dist-tag, CDN immutable path and codec-pack rollback procedures. |
| [diagnostics-runbook.md](./diagnostics-runbook.md) | How to collect diagnostics, interpret common errors and escalation paths. |
| [known-limitations.md](./known-limitations.md) | Impact, scope, mitigations and planned versions for every known limitation. |

## Quick smoke test

From a clean checkout:

```bash
source ~/.nvm/nvm.sh && nvm use
pnpm install --frozen-lockfile
pnpm build
pnpm typecheck
pnpm test
./scripts/integration-smoke.sh
```

## What is in scope for Web v1

- Single-window playback with H.264/H.265/AAC/G.711A/U/MP3 via HTTP/WS-FLV,
  HLS/LL-HLS TS/fMP4 and HTTP/WS-fMP4.
- WebCodecs → MSE → FFmpeg-WASM (threads+SIMD, SIMD-only, baseline) fallback chain.
- 1/4/9/16 multiview wall with main/substream switching and resource budget.
- Snapshot, MP4/fMP4/FLV recording and diagnostics export.
- Graceful stop/reload/destroy, config change, backend/device fault recovery.

## What is explicitly NOT in scope for Web v1

- Jessibuca Pro full parity / Native client / bidirectional real-time.
- Anything listed under **Future** in the acceptance checklist.

## Evidence convention

Every Required acceptance item is expected to have:

1. A test name or command.
2. The environment where it was run (browser, OS, GPU, isolation mode, server).
3. A log, screenshot, metric export or CI job link.
4. An owner and a sign-off date.

Future items are linked to backlog issues and must not be counted in the
Web v1 completion rate.
