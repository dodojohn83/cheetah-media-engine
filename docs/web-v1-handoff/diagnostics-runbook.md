# Web SDK Diagnostics Runbook

## Collecting diagnostics

```ts
import { createPlayer } from '@cheetah-media/web';

const player = createPlayer({
  diagnostics: { maxEventHistory: 500, statsIntervalMs: 250 },
});

// Later, e.g. in a bug report or support ticket:
const bundle = player.exportDiagnostics();
console.log(JSON.stringify(bundle, null, 2));
```

The bundle contains:

- `playerId`, `version`, `state`, `epoch`.
- Redacted `config` (credentials, tokens, URLs masked as `<redacted>`).
- `lastStats`, `metrics` and recent events.

## Common errors and checks

### `workerUrl` missing or 404

- Check `assetBaseUrl` or explicit `runtime.workerUrl`.
- Ensure the worker is served with `Content-Type: text/javascript` and
  `Cross-Origin-Resource-Policy: cross-origin` in isolated mode.

### WASM MIME / hash / ABI mismatch

- Confirm `Content-Type: application/wasm`.
- Check browser devtools Network tab for `cheetah_media_web_bindings_bg.wasm`.
- Verify `codec-packs/ffmpeg-wasm/manifest.json` `hash` and `abi_version`.

### Isolated mode not enabled

- Check `window.crossOriginIsolated` and `window.SharedArrayBuffer`.
- Confirm main document returns `COOP: same-origin` and `COEP: require-corp`.
- Confirm worker and wasm resources return `COEP: require-corp` and `CORP: cross-origin`.

### `MediaError` with recoverable=false

- Look at `code`, `stage` and `message`.
- If `stage` is `webcodecs` and recoverable is `false`, the browser may not
  support the codec; the fallback controller should try MSE or WASM. If it does
  not, check the capability probe and planner output.

### Latency drift / dropped frames

- Call `getStats()` repeatedly and watch `bufferedMs`, `decodedFrames`, `droppedFrames`.
- If `bufferedMs` grows above the hard target, the latency controller will drop
  or jump-to-live. Check `latency` config and network bandwidth.

### Resource leak on destroy

- After `await player.destroy()`, check that the worker process / WASM memory is
  released (browser Performance tab).
- Ensure `stop()` is awaited before `destroy()`.

## Escalation path

1. Reproduce with `scripts/integration-smoke.sh`.
2. Collect `exportDiagnostics()` JSON and capability snapshot from
   `tests/browser/tests/capability-snapshot.spec.ts`.
3. Attach browser version, OS, GPU, `crossOriginIsolated` value and a HAR.
4. File an issue with the above, the `codec`/`protocol`/`backend` matrix cell,
   and the regression range.
