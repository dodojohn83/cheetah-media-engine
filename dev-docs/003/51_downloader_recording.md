# WP-51 直播/回放下载器、合成录制、VR/AI 扩展入口

## 1. 目标

在 Web SDK 与 Runtime 中补齐 Jessibuca Pro 的“下载器 + 合成录制”能力，并为 VR/全景与 AI 扩展预留入口，但不提前实现完整 renderer 或模型推理。

## 2. 交付物

### 2.1 51a — 直播/回放下载器

已完成：

- `packages/runtime/src/fetch/downloader.ts`：基于 `fetch` / `ReadableStream` 的直播/回放片段下载器，支持 HTTP 拉流，存储到任意 `DownloadSink`（默认 `BlobSink`）。
  - `StreamDownloader.start/resume/pause/stop` 状态机，支持 `Range` 续传。
  - `DownloadProgress` 包含 `bytesWritten`、`startedAt`、`state`。
  - `DownloadOptions` 支持 `headers`、`credentials`、`transform`（用于解密/过滤）、`onProgress/onError/onComplete`。
- `packages/web/src/player.ts`：
  - `startDownload(options)` / `stopDownload()` / `pauseDownload()` / `resumeDownload()`。
  - `CheetahPlayer.downloadActive`、`downloadProgress` 只读属性。
  - 下载失败映射为 `CheetahMediaError`；仅允许 `http:`/`https:` URL。
- 测试：Vitest 覆盖启动/停止/暂停/续传/错误、transform 过滤、HTTP 错误、非 http URL 拒绝、播放器事件。

已完成（随 51b PR 补齐）：

- `packages/components/src/player-element.ts`：`download` 属性/按钮与 `downloadprogress` 事件。
- 下载按钮仅在设置 `download` 属性时显示；点击调用 `startDownload({ url, filename })` / `stopDownload()`。
- `download` 事件映射为 `downloadprogress` CustomEvent，包含 `progress`、`completed`、`error`。

待 51c：

- HLS 子片段拼接复用 `HlsClient`；加密流 transform 示例。

### 2.2 51b — 合成录制

已实现：

- `packages/runtime/src/video/composite-recorder.ts`：基于 `HTMLCanvasElement.captureStream()` + `MediaRecorder` 的合成录制器。
  - `CompositeRecorder` 支持 `start`/`pause`/`resume`/`stop`。
  - 每一帧通过 `requestAnimationFrame` 将 source（`HTMLVideoElement`/`HTMLCanvasElement`/`ImageData`/image）绘制到内部 canvas，并叠加文字/图片水印。
  - 支持单个 `watermark` 或多个 `watermarks` 同时叠加。
  - 复用 `packages/runtime/src/video/recorder.ts` 的 `startRecording` 作为 `MediaRecorder` 后端。
  - `BlobStreamSink` 将 `WritableStream<Uint8Array>` 汇聚为最终 `Blob`。
  - 输入校验：拒绝尺寸为 0 或过大的 source，捕获 `MediaRecorder` 错误并抛出 `RendererError`。
- `packages/web/src/player.ts`：
  - `startCompositeRecording(options)` / `pauseCompositeRecording()` / `resumeCompositeRecording()` / `stopCompositeRecording()`。
  - `compositeRecordingActive` 只读属性。
  - 通过 `compositeRecording` 事件对外报告进度；`stopCompositeRecording()` 返回 `{ blob, mimeType, durationMs, bytes }`。
  - 设置 `filename` 时通过 `URL.createObjectURL` + 临时 `<a>` 触发浏览器下载，1 分钟后 `revokeObjectURL`。
- `packages/components/src/player-element.ts`：
  - 录制按钮使用合成录制 API，源取自 `slot="surface"` 的 video/canvas。
  - `recordingactive` boolean 属性/attribute，由 `compositeRecording` 事件同步。
  - `recordingprogress` CustomEvent 对外暴露进度/结果/错误。
  - 水印从 `watermarks` 属性解析，text/image 类型按百分比位置映射为像素坐标后传入 `CompositeRecorder`。
- 完成定义：录制 1 秒后输出非空 Blob；暂停后继续不丢帧；带文字/图片水印的合成帧正确叠加水印。

### 2.3 51c — VR/AI 扩展入口

- `packages/web/src/vr/`：
  - `VrRenderer` 接口与 `NoopVrRenderer` 占位；仅在探测到 360/equirectangular metadata 且浏览器支持 WebGL/WebGPU 时启用。
  - `AiFrameProcessor` 接口与 `NoopAiFrameProcessor` 占位；通过 `requestVideoFrameCallback` 或 `VideoFrame` 回调注入，预算不足时自动 skip。
- `packages/web/src/player.ts`：
  - `setVrRenderer(renderer)` / `setAiProcessor(processor)` 扩展入口。
  - `vrActive`、`aiActive` 状态，作为 capability 暴露，不冒充标准支持。
- 文档：`docs/web-v1-handoff/vr-ai-extension.md` 说明接口、激活条件、预算与默认占位行为。

## 3. 边界与约束

- 下载器不自行做协议状态机；HTTP/WS 流复用 `Transport` 或 `FetchStreamReader`，HLS 复用 `HlsClient` 的子片段下载。
- 合成录制在浏览器不支持 `MediaRecorder` 或 `CanvasCaptureMediaStream` 时返回稳定 `Unsupported`。
- VR/AI 仅作为扩展点，本 WP 不实现完整 360° renderer 或目标检测模型。
- 所有外部输入校验大小、时长、MIME type；禁止 `unwrap`/`expect`。

## 4. 验收标准

- `cargo fmt/clippy/test --workspace --all-features` 通过（本次以 JS 为主，Rust 侧无新 unsafe）。
- `corepack pnpm typecheck/test/build` 通过。
- Playwright：下载器启动/停止、加密流 transform、录制 1 秒非空、水印合成可视觉断言。
- VR/AI 入口：`setVrRenderer`/`setAiProcessor` 接受/拒绝、默认 no-op、预算不足 skip。

## 5. 拆分策略

| 子包 | PR | 范围 | 依赖 |
| --- | --- | --- | --- |
| 51a | #55 | 直播/回放下载器 | WP-18 transport, WP-13 HLS client, WP-48 crypto transforms |
| 51b | #56 | 合成录制 | WP-29 snapshot/recorder, WP-47 watermark |
| 51c | #57 | VR/AI 扩展入口 | WP-24 renderer, WP-30 metrics |

## 6. 状态

- [x] 51a 范围文档
- [x] 51a 实现与 PR
- [x] 51b 实现与 PR
- [x] 51c 实现与 PR
