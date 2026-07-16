# WP-42 WebRTC H.264/H.265 signaling/transport skeleton

## 1. 目标

在 `packages/runtime` 中增加一个基于 `RTCPeerConnection` + `RTCDataChannel` 的 WebRTC 传输骨架，为后续通过 WebRTC 接收 H.264/H.265 媒体码流建立能力探测、信令入口和生命周期基础。

本 WP 仅实现数据通道（`RTCDataChannel`）承载原始字节包的传输与信令交换骨架，不实现浏览器 `ontrack` 解码路径；`ontrack`/`<video>` 渲染将在播放器集成阶段（后续 WP）继续。数据通道路径可直接复用现有 `Transport` 接口和 `AnnexB`/`MPEG-PS` demuxer 管线。

## 2. 依赖

- WP-18 `packages/runtime/src/transport.ts`：HTTP/WebSocket 字节传输抽象与 `Transport` 接口。
- WP-19 `packages/runtime/src/capabilities.ts`：浏览器能力探测与缓存。
- WP-37 `crates/cheetah-container-annexb`：H.264/H.265 Annex-B 解复用。
- WP-39 `crates/cheetah-container-mpegps`：MPEG-PS 解复用（可选，数据通道也可承载 MPEG-PS）。

## 3. 交付物

### 3.1 能力探测

`packages/runtime/src/capabilities.ts`：

- 在 `CapabilityReport` 中新增 `webRtc: boolean`（`RTCPeerConnection` 全局存在）。
- 在 `ProbeDetails` 中新增 `webRtc` 对象，包含：
  - `peerConnection: boolean`
  - `dataChannel: boolean`（能否创建数据通道）
  - `insertableStreams: boolean`（`RTCRtpScriptTransform` / `createEncodedStreams`，仅记录，不作为本 WP 依赖）
  - `getUserMedia: boolean`
- `detectCapabilities()` 同步检查 `globalThis.RTCPeerConnection`。
- `probeCapabilities()` 通过检查构造函数与原型特征给出细化报告；不发起真实 ICE/SDP 网络交换。

### 3.2 WebRTC 传输类

`packages/runtime/src/webrtc.ts`（`WebRtcTransport`）：

- 实现 `Transport` 接口。
- `start()` 校验 URL（仅 `https://` 或 `http://` 用于本地开发；`webrtc://` 等 scheme 也允许，但实际信令仍走 HTTPS），检查 `RTCPeerConnection` 全局存在。
- 创建 `RTCPeerConnection`，打开 `RTCDataChannel`（标签 `media`，`ordered: true`）。
- 调用 `createOffer()`、`setLocalDescription()`。
- 将本地 SDP offer 通过 `fetch` POST 到 `config.url`（Content-Type: `application/sdp`），服务端返回 SDP answer；将 answer 设为 `setRemoteDescription()`。
- 监听数据通道 `onmessage`；收到 `ArrayBuffer` 消息时通过 `onChunk` 交给上层。
- 监听 `onopen`/`onclose`/`onerror` 与 `RTCPeerConnection` 的 `connectionstatechange`/`iceconnectionstatechange`；失败/关闭时通过 `onError` 或 `onEnd` 回调。
- `stop()` 调用 `RTCPeerConnection.close()`，清理事件监听器与状态。
- 错误映射到新的 `TransportErrorCode`：
  - `WebRtcNotSupported`（7012）
  - `WebRtcSignalingFailed`（7013）
  - `WebRtcConnectionFailed`（7014）
  - `WebRtcDataChannelFailed`（7015）

### 3.3 传输入口与 planner 路由

`packages/runtime/src/transport.ts`：

- `createTransport(config, mode?)` 的 `mode` 联合类型增加 `'webrtc'`。
- `mode === 'webrtc'` 时返回 `WebRtcTransport`。
- 无 `mode` 时：URL scheme 为 `webrtc:` 或 `webrtc+https:` 时也路由到 WebRTC（可选，主要依赖显式 `mode`）。
- 从 `transport.ts` 和 `index.ts` 导出 `WebRtcTransport`。

`packages/runtime/src/planner.ts`：

- `Protocol` 增加 `'webrtc'`。
- `TransportMode` 增加 `'webrtc'`。
- `canMseContainer` 对 `webrtc` 返回 `false`。
- `protocolRequiresDemux` 对 `webrtc` 返回 `false`（数据通道裸 Annex-B 可直接喂 decoder）。
- `chooseTransport('webrtc')` 返回 `'webrtc'`。
- backend 选择：与 `http-annexb` 一致，首选 WebCodecs，其次 WASM 软解；MSE 不可用。

### 3.4 测试

- `packages/runtime/src/capabilities.test.ts`：验证 `detectCapabilities` 和 `probeCapabilities` 的 `webRtc` 字段在 `RTCPeerConnection` 存在/缺失时的行为。
- `packages/runtime/src/webrtc.test.ts`：使用 mock `RTCPeerConnection`、`RTCDataChannel` 和 `fetch`，验证：
  - `start()` 创建 offer、POST 到 URL、设置 answer、通道打开。
  - 数据通道收到 `ArrayBuffer` 消息后调用 `onChunk`。
  - 缺少 `RTCPeerConnection` 时报告 `WebRtcNotSupported`。
  - 非 WebRTC 支持 URL 返回错误。
  - `stop()` 调用 `pc.close()`。
- `packages/runtime/src/transport.test.ts`：验证 `createTransport(config, 'webrtc')` 返回 `WebRtcTransport`。
- `packages/runtime/src/planner.test.ts`：验证 `webrtc` 协议至少一条 routing 断言（transport=webrtc，MSE 不可用）。

## 4. 完成定义

- [ ] `corepack pnpm typecheck` 通过。
- [ ] `corepack pnpm test` 通过。
- [ ] `corepack pnpm build` 通过。
- [ ] 新增文件保持在 500 行以内（超出 800 必须拆分）。
- [ ] 无 `todo!()` / `unimplemented!()` / 空 provider。
- [ ] 信令失败、ICE 失败、数据通道关闭均通过 `onError`/`onEnd` 回调报告，不抛未捕获异常。
- [ ] 探测逻辑不发起真实 ICE/SDP 网络请求，避免 CI 不稳定。

## 5. 后续

- 播放器集成阶段实现 `ontrack` 路径：将 `MediaStreamTrack` 直接送入 `<video>` 或 `WebCodecs`（通过 `VideoFrame`/`EncodedVideoChunk`）。
- 在支持 Insertable Streams 的浏览器中支持 `RTCRtpReceiver.createEncodedStreams()` 获取原始 H.264/H.265 NAL，作为数据通道之外的第二条 WebRTC 输入路径。
- 与 `dodojohn83/cheetah-signaling` 的 WHIP/WHEP 信令端点对齐 offer/answer 交换格式。

PR: `wp/42-webrtc-transport` → `wp/16-engine-state-machine`.
