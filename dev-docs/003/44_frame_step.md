# WP-44：逐帧 / 逐关键帧 / 暂停显示但保持连接

## 1. 目标

在现有直播/VOD 播放能力之上，实现精确的单帧前进/后退、只跳到关键帧、以及暂停时保留最后一帧画面同时保持网络/解码管线活跃。该能力对应 `07_jessibuca_pro_feature_parity.md` 中的“逐帧/逐关键帧”和“暂停显示但保持连接”。

## 2. 依赖

- WP-14：Timeline、GOP cache、A/V sync。
- WP-21：MSE backend 生命周期、append queue、live control。
- WP-20：WebCodecs backend 生命周期、队列边界。
- WP-43：seek、playbackRate 控制面。

## 3. 交付物

### 3.1 控制面 API 契约

- `packages/runtime/src/messages.ts`：新增 `frame-step` 和 `pause-display` 消息类型。
  - `FrameStepPayload`：`{ direction: 'forward' | 'backward'; keyframeOnly?: boolean }`
  - `PauseDisplayPayload`：`{ keepConnection: boolean }`（`true` 时暂停显示但保持解码器/网络；`false` 时回到普通 pause）
- `EngineRuntime` / `worker.ts`：暴露 `frameStep(direction, keyframeOnly?)` 和 `pauseDisplay(keepConnection)`。
- `WebEngine` Rust bindings：记录暂停显示状态、帧步进目标；`stop()` 重置。

### 3.2 WebCodecs / MSE backend 行为

- `packages/runtime/src/webcodecs.ts`：
  - `pauseDisplay(keepConnection = true)`：暂停 `VideoDecoder`/`AudioDecoder` 输出，清空渲染队列，保留解码器实例，保持网络下载；当 `keepConnection = false` 时进入普通 pause。
  - `frameStep(keyframeOnly = false)`：
    - 若当前处于 `pauseDisplay` 状态，从 GOP cache 或 timeline 取下一帧/上一帧。
    - 向前步进：推送下一帧到 decoder 并只输出该帧，渲染后重新进入 `pauseDisplay`。
    - 向后步进：仅在有完整 GOP cache 时支持；否则返回 `Unsupported`。
    - `keyframeOnly` 为 `true` 时只选择关键帧（IDR/CRA/BLA/IRAP）。
  - 添加单元测试（mock VideoDecoder/VideoFrame）。

- `packages/runtime/src/mse.ts`：
  - `pauseDisplay(keepConnection = true)`：暂停 `video` 播放但不释放 MediaSource/SourceBuffer，停止 live control，停止音频渲染；`keepConnection = false` 时进入普通 pause。
  - `frameStep(keyframeOnly = false)`：
    - 在 MSE 路径下，帧步进需要解码器反馈，因此主要依赖 `video.currentTime` 微调：每次向前/向后移动一帧时间（基于 `video` 的 `frameRate` 或默认 25fps）。
    - `keyframeOnly` 为 `true` 时直接 seek 到下一个/上一个关键帧。由于当前 MSE 只解析了 buffer 时间范围，不解析内部帧类型，`keyframeOnly` 在 MSE backend 上返回 `Unsupported`，或者通过 `requestKeyframe()` + seek 实现近似行为。
  - 单元测试覆盖 `pauseDisplay` 停止 live control 但不释放 buffer；`frameStep` 更新 currentTime；`keyframeOnly` 返回 Unsupported。

### 3.3 Timeline / GOP cache 支持

- `crates/cheetah-media-timeline`（如已存在 GOP 结构）：扩展 `GopCache` 接口，支持按时间/方向查找下一关键帧/任意帧。
- 如果当前 JS 端 GOP cache 仅保留在 `cheetah-media-engine` 中，则在 `packages/runtime/src/timeline.ts` 或 `gop-cache.ts` 中实现；否则跳过 Rust 改动，并在 JS 端用 `VideoFrame` 队列近似。

### 3.4 公开 Player API

- `packages/web/src/player.ts`：`CheetahPlayer` 新增 `frameStep(direction, keyframeOnly?)` 和 `pauseDisplay(keepConnection?)`。
- 文档更新：`packages/web/README.md`。
- 单元测试：`packages/web/src/index.test.ts` mock runtime 更新，添加转发/错误测试。

## 4. 完成定义

- [x] `frame-step` / `pause-display` 消息和 payload 已定义并导出。
- [x] `EngineRuntime` / `worker.ts` 正确转发到 `WebEngine`（或本地 backend）。
- [x] `WebEngine` 存储 `frame_step_pending` 和 `pause_display_keep_connection` 状态，并在 `stop()` 中重置。
- [x] WebCodecs backend 实现 `pauseDisplay` 和 `frameStep`：前向单帧步进可工作；后向步进返回明确 Unsupported（需完整 GOP cache）；`keyframeOnly` 支持等待下一个关键帧。
- [x] MSE backend 实现 `pauseDisplay` 并支持 `frameStep` 基于 `videoFrameRate` 的 currentTime 微调；`keyframeOnly` 返回明确 `Unsupported`。
- [x] `FallbackController` 转发 `frameStep` / `pauseDisplay` 到当前 backend；不支持的 backend 返回错误。
- [x] 公开 `Player` API 更新，README 同步，测试通过。
- [x] 运行完整验证矩阵：
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo test --workspace --no-default-features`
  - `cargo build --workspace --target wasm32-unknown-unknown --no-default-features`
  - `cargo deny check`
  - `corepack pnpm typecheck`
  - `corepack pnpm test`
  - `corepack pnpm build`
- [ ] 无 `todo!()` / `unimplemented!()` / 生产路径 `unwrap()`。

## 5. 后续

- WP-45：PTZ 操作盘与 GB28181 命令生成。
