# WP-40 HTTP/WS MPEG-PS/Annex-B transport 与 planner 路由

## 1. 目标

让 Web runtime 识别、规划和路由通过 HTTP(S) 或 WebSocket 传输的原始 Annex-B H.264/H.265 和 MPEG-PS 码流，为后续在 worker/WASM 中调用 `AnnexBDemuxer` / `MpegPsDemuxer` 并喂给 WebCodecs/WASM 解码器建立入口。

本 WP 先完成传输协议识别、planner 路由和测试；实际 demuxer/decoder 管线在 40b/40c 子任务中落地，避免把未验证的端到端播放伪造成完成。

## 2. 依赖

- WP-18 `packages/runtime/src/transport.ts`：Fetch/WebSocket 字节传输。
- WP-37 `crates/cheetah-container-annexb`：H.264 Annex-B 解复用。
- WP-38 `crates/cheetah-container-annexb`：H.265 Annex-B 解复用。
- WP-39 `crates/cheetah-container-mpegps`：MPEG-PS 解复用。

## 3. 交付物

### 3.1 Planner 扩展

- `packages/runtime/src/planner.ts`：
  - `Protocol` 新增 `http-annexb`、`ws-annexb`、`http-mpegps`、`ws-mpegps`。
  - `chooseTransport` 把 `ws-*` 映射为 `websocket`，`http-*` 映射为 `fetch`。
  - `canMseContainer` 对原始 Annex-B 和 MPEG-PS 返回 `false`。
  - 对 `http-annexb` / `ws-annexb`：若 `webcodecs` 支持对应 codec 可选 `webcodecs`（H.264/H.265 Annex-B 可直接作为 `EncodedVideoChunk` 输入）；否则使用 WASM 软解。
  - 对 `http-mpegps` / `ws-mpegps`：因需先 demux，MSE 不可直接消费，首选 WASM 软解管线；WebCodecs 仅在有显式 demuxer 前置后可用。
  - 给出明确 `reason` 字符串，说明原始流需要 demux/解码器。

### 3.2 传输入口

- `packages/runtime/src/transport.ts`：`createTransport` 已按 scheme 选择 Fetch/WebSocket，无需修改；URL scheme 仍只接受 `http/https/ws/wss`。
- 真正的 `RawStreamBackend`（Transport + demuxer + decoder 管线）在 40c 子任务中实现，避免在本阶段提交无法真正播放的壳。

### 3.3 测试

- `packages/runtime/src/planner.test.ts`：新增 4 个协议（Annex-B H.264/H.265、MPEG-PS H.264）的 routing 断言。

## 4. 完成定义

- `corepack pnpm typecheck` 通过。
- `corepack pnpm test` 通过。
- `corepack pnpm build` 通过。
- 新增协议在 planner 测试中覆盖，且不破坏现有 FLV/fMP4/HLS 路由。
- 无 `todo!()` / `unimplemented!()` / 空 provider。

## 5. 后续子任务（不在本 PR）

- 40b：在 `cheetah-media-web-bindings` 中暴露 `AnnexBDemuxer` 和 `MpegPsDemuxer`，支持 `push`/`poll` 出 `MediaPacket` 描述符。
- 40c：在 `RawStreamBackend` 中接入 WASM demuxer，并把视频 `MediaPacket` 喂给 `WebCodecs` / WASM 解码器，音频喂给 `AudioWorklet` / WebAudio。

PR: `wp/40-raw-stream-transport` → `wp/16-engine-state-machine`。
