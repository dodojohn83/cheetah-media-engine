# WP-41 WebTransport transport 骨架与 capability 探测

## 1. 目标

在 `packages/runtime` 中增加 `WebTransport` 传输入口和浏览器能力探测，为后续支持基于 WebTransport 的媒体流播放建立类型、路由和生命周期基础。本 WP 只实现骨架与探测，不承诺真实端到端播放（在 WP-42 WebRTC 之后的播放器集成中继续）。

## 2. 依赖

- WP-18 `packages/runtime/src/transport.ts`：HTTP/WebSocket 字节传输抽象。
- WP-19 `packages/runtime/src/capabilities.ts`：浏览器能力探测与缓存。
- WP-40 `packages/runtime/src/planner.ts`：协议/能力到 transport mode 的路由。

## 3. 交付物

### 3.1 能力探测

`packages/runtime/src/capabilities.ts`：

- 在 `CapabilityReport` 中新增 `webTransport: boolean`。
- 在 `ProbeDetails` 中新增 `webTransport` 对象，包含：
  - `datagrams: boolean`
  - `incomingUnidirectionalStreams: boolean`
  - `incomingBidirectionalStreams: boolean`
  - `byob: boolean`（是否支持 `ReadableStreamBYOBReader`）
- `detectCapabilities()` 同步检查 `globalThis.WebTransport`。
- `probeCapabilities()` 通过检查 `WebTransport.prototype` getter 或构造器原型，给出更细化的能力报告；不发起真实网络连接。

### 3.2 WebTransport 传输类

`packages/runtime/src/webtransport.ts`（`WebTransportTransport`）：

- 实现 `Transport` 接口。
- `start()` 校验 URL（仅 `https://`），检查 `WebTransport` 全局存在。
- 使用 `new WebTransport(url)` 建立会话，等待 `.ready`。
- 读取 `incomingUnidirectionalStreams`，把每个 `WebTransportReceiveStream` 的 chunk 通过 `onChunk` 回调交给上层。
- 若 `incomingUnidirectionalStreams` 不可用但 `datagrams.readable` 存在，可降级读取 datagrams（不可靠，本 WP 仅作为探测/占位）。
- `stop()` 调用 `transport.close()` 并取消读取循环。
- 错误映射到 `TransportErrorCode`：
  - `WebTransportNotSupported`
  - `WebTransportClosed`

### 3.3 传输入口与 planner 路由

`packages/runtime/src/transport.ts`：

- `createTransport(config, mode?)` 增加可选 `mode` 参数（`'fetch' | 'websocket' | 'webtransport'`）。
- 当 `mode === 'webtransport'` 时返回 `WebTransportTransport`。
- 无 `mode` 时保持现有 URL scheme 推断行为。

`packages/runtime/src/planner.ts`：

- `Protocol` 增加 `'webtransport'`。
- `TransportMode` 增加 `'webtransport'`。
- `chooseTransport('webtransport')` 返回 `'webtransport'`。

### 3.4 测试

- `packages/runtime/src/capabilities.test.ts`：mock `globalThis.WebTransport` 验证 `detectCapabilities` 和 `probeCapabilities` 的 `webTransport` 字段。
- `packages/runtime/src/transport.test.ts`：mock `WebTransport` 构造函数和流接口，验证 `WebTransportTransport` 的 `start`/`stop`、chunk 分发和错误路径。
- `packages/runtime/src/planner.test.ts`：新增 `webtransport` 协议至少一条 routing 断言。

## 4. 完成定义

- [x] `corepack pnpm typecheck` 通过。
- [x] `corepack pnpm test` 通过。
- [x] `corepack pnpm build` 通过。
- [x] 新增文件保持在 500 行以内（超出 800 必须拆分）。
- [x] 无 `todo!()` / `unimplemented!()` / 空 provider。
- [x] 不对 `WebTransport` API 调用做 `any` 类型逃逸；提供最小 TypeScript 声明。
- [x] 探测逻辑不发起真实网络请求，避免 CI/测试不稳定。
- [x] PR 通过 CI/Devin Review 并合并。

## 5. 后续

- WP-42 WebRTC transport 骨架。
- 在播放器集成阶段把 `WebTransportTransport` 与 `RawStreamBackend` 或 `WebCodecs` 管线对接。

PR: `wp/41-webtransport-skeleton` → `wp/16-engine-state-machine` (#43).
