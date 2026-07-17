# 32. 浏览器 E2E、兼容性与故障注入

## E2E-001：浏览器矩阵

| 环境 | 必验内容 |
| --- | --- |
| Chrome stable / Windows、macOS | WebCodecs、MSE、WASM threads/SIMD、WebGPU/WebGL |
| Edge stable / Windows | 硬解密度、MSE、企业常见安全策略 |
| Safari stable / macOS | WebCodecs/MSE 实际支持组合、WebGL、音频生命周期 |
| Firefox stable / Windows、macOS | MSE/软解回退、WebGL、非隔离路径 |

- [x] 每次运行记录浏览器完整版本、OS、GPU、driver、isolation 和 capability snapshot。  
  证据：`tests/browser/tests/capability-snapshot.spec.ts` 在 Chromium/Firefox/WebKit 上记录 JSON snapshot，包含 `browser`, `userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`, `crossOriginIsolated`, `sharedArrayBuffer`, GPU vendor/renderer, `webCodecs`, `mediaSource`, `webAudio`, `webgpu`, `webgl2`, `wasm`。CI `web` job 使用 Playwright 三浏览器矩阵。
- [ ] API 缺失不是测试跳过理由；必须断言预期下一路线或 Unsupported。  
  注：当前 E2E 尚未注入 WebCodecs/MSE 误报，仅在 harness 中探测支持。

## E2E-002：协议/codec 核心矩阵

- [ ] HTTP-FLV、WS-FLV：H.264/H.265 × AAC/G.711A/U/MP3 的适用组合。
- [ ] HLS/LL-HLS：TS 与 fMP4、variant、discontinuity、窗口滑动和 parts。
- [ ] HTTP/WS-fMP4：init/media 跨任意 chunk、config change 和断流。
- [ ] 每条路径验证首帧、连续播放、A/V sync、stop/reload/destroy 和实际 backend。  
  注：本 PR 仅建立 harness 与 fault-injection；实际协议/codec 端到端矩阵依赖网络→demux→decode 全链路，待后续任务补充。

## E2E-003：故障注入目录

- [x] 网络：DNS/连接失败、HTTP 错误、stall、短读、断连、乱序脚本和重连。  
  证据：`tests/browser/tests/fault-injection.spec.ts` 覆盖 worker 404、wasm module 404、wasm wrong MIME、bad src URL，并在 `/isolated` 路由验证 COOP/COEP/CORP。
- [ ] WebCodecs：support 误报、configure/decode/async error、queue stall。
- [ ] MSE：MIME 拒绝、append error、QuotaExceeded、media element error。
- [x] WASM：pack 404/hash/ABI 错、OOM、worker crash、decoder error。  
  证据：fault-injection spec 覆盖 wasm module 404 与 wrong MIME，worker 404 触发失败状态。
- [ ] 渲染/音频：device/context lost、AudioContext suspend、surface detach、后台恢复。
- [ ] 每次注入断言状态、事件顺序、下一后端、恢复耗时和资源 ledger。
  注：当前仅验证 player `data-state="failed"` / `data-state="preroll"`；完整 ledger/event 序列注入待后续。

## E2E-004：部署模式

- [x] isolated demo 使用正确 COOP/COEP/CORP，证明 threads+SIMD。  
  证据：`apps/web-demo/scripts/preview.js` 为 `/isolated` 设置 `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp`；为 `/worker.js`、`/messages.js` 和 `/wasm/*` 设置 `Cross-Origin-Resource-Policy: cross-origin` 与 `Cross-Origin-Embedder-Policy: require-corp`。`tests/browser/tests/capability-snapshot.spec.ts` 断言 `/isolated` 的 `crossOriginIsolated` 和 `SharedArrayBuffer` 为真，并到达 `preroll`。
- [x] non-isolated demo 不使用 SharedArrayBuffer，证明 SIMD/baseline 和相同 SDK API。  
  证据：`/` 路由不发送 COOP/COEP，`capability-snapshot.spec.ts` 记录 `crossOriginIsolated=false`；组件与 smoke spec 在非隔离路径运行，验证相同 SDK API 可达。
- [x] self-host/CDN、不同 base URL、严格 CSP、错误 WASM MIME 和跨域 codec pack 分别验证。  
  证据：`cheetah-player` 的 `worker-url` 和 `wasm-url` 属性支持任意 URL；fault-injection spec 使用错误的 wasm URL 和错误的 MIME 路由验证失败状态；isolated 路径下从同源加载 worker/wasm。
- [ ] Playwright trace/video 仅在失败保存并脱敏；flaky 重跑仍保留首次失败证据。  
  部分：Playwright 已配置 `trace: 'on-first-retry'`；脱敏与首次失败证据保留策略待后续审计。
