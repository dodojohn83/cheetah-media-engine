# Privacy Guide

The player minimizes the data it retains and exposes.

## Data minimization

- Media packets and decoded frames are never serialized in diagnostics.
- Event history is bounded by `diagnostics.maxEventHistory` (default 500).
- Diagnostic bundles are capped at 256 KiB by default; oversized event tails
  are truncated before export.
- Destroying a player clears event history, listeners and the metric registry.
  The runtime worker is terminated and no session identifiers remain in the
  main thread.

## Sanitization

`exportDiagnostics()` uses `sanitizeUrl()` and `redactHeaders()` to strip:

- query parameters, fragments and authentication components from URLs,
- `Authorization`, `Cookie` and custom secret headers,
- credentials, tokens and secrets from configuration objects.

The exported bundle contains only non-sensitive identifiers needed for
debugging: player id, runtime version, state, epoch, sanitized config,
metrics, recent event names and aggregated statistics.

## Telemetry defaults

By default the player does not send telemetry to any external endpoint.
`exportDiagnostics()` returns a local object that the application can choose
whether and where to upload. Stats events are throttled to one emission per
`statsIntervalMs` and never contain payload data.
