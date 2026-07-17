# WP-43 MP4/HLS 点播、seek、倍速

## 1. 目标

在已有 Web v1 骨架上补齐 VOD（点播/回放）能力：

1. HLS/LL-HLS client 识别 VOD/EEVENT 与 ENDLIST，支持按时间定位到 segment/part。
2. MSE backend 支持 VOD 模式（关闭直播追赶逻辑）、接收 seek 指令、跟随用户倍速。
3. Runtime/worker/engine 控制面暴露 `seek(timeMs)` 与 `setPlaybackRate(rate)`。
4. 本 WP 按依赖拆分为三个可独立审查的子任务，全部完成后 WP-43 结束。

> 本 WP 只做控制面与播放调度：点播流仍由现有 fMP4/MPEG-TS 等 container pipeline 解码；HLS 密钥/Map/EXT-X-DISCONTINUITY 沿用 WP-13 实现。

## 2. 子任务

### 2.1 43a — 控制面 API 契约

依赖：WP-21 (MSE backend)、WP-14 (engine state machine)、WP-32 (worker/runtime)。

交付：

- `packages/runtime/src/messages.ts`：新增消息类型 `seek`、`set-playback-rate` 与对应 payload。
- `packages/runtime/src/runtime.ts`：`EngineRuntime` 增加 `seek(timeMs: number): Promise<void>` 与 `setPlaybackRate(rate: number): Promise<void>`；`runtime.seek()` 在 worker 未启动时 reject。
- `packages/runtime/src/worker.ts`：dispatch `seek`/`set-playback-rate`，调用 `WebEngineInstance` 的 `seek`/`setPlaybackRate` 方法。
- `crates/cheetah-media-web-bindings/src/lib.rs`：`WebEngine` 增加 `pub fn seek(&mut self, time_ms: u64) -> Result<(), JsValue>` 与 `pub fn set_playback_rate(&mut self, rate: f64) -> Result<(), JsValue>`；当前为 no-op（将状态记录到日志/指标），等 43b/43c 接入。
- `packages/runtime/src/index.ts` 与 `packages/web` 公开 API：导出/暴露 `seek` 与 `playbackRate` 控制（如 `Player.seek`、`Player.playbackRate`）。
- 增加单元测试验证消息 round-trip、runtime 调用转发、worker dispatch、WebEngine 方法存在及越界参数返回错误。

### 2.2 43b — HLS client VOD 与 seek

依赖：WP-13 (HLS client)、43a。

交付：

- `crates/cheetah-hls-client/src/model.rs`：`MediaPlaylist` 增加 `duration: f64`（所有 segment duration 之和；VOD 在解析后计算）。
- `crates/cheetah-hls-client/src/client.rs`：
  - 识别 `playlist_type == Vod` 或 `end_list == true` 时进入 VOD 模式：不发起 live reload，停止 `Tick` 调度。
  - 新增 `HlsEvent::Seek { time_ms: u64 }`；seek 时计算目标 segment（按 `Segment::duration` 累加），生成 `LoadSegment` action 并切换当前读取位置。
  - `HlsEvent::SetPlaybackRate` 本阶段仅记录，不控制 segment 加载速度（播放速度由后端调速）。
  - 边界：seek 超出 `duration` 时返回 `HlsError::SeekOutOfRange`。
- `crates/cheetah-hls-client/src/parser.rs`：保留 `#EXT-X-PLAYLIST-TYPE:VOD` 解析，ENDLIST 已解析，无需改动。
- 单元测试：VOD 不 reload、seek 到指定 segment、seek 越界返回错误、EVENT 结束后再 reload 的边界行为。

### 2.3 43c — MSE backend VOD 与倍速

依赖：43a、43b。

交付：

- `packages/runtime/src/mse.ts`：
  - `MseBackendOptions` 增加 `isLive?: boolean`（默认 `true` 保持现有行为）。
  - VOD 模式（`isLive === false`）关闭 `liveLatencyTargetMs` 追赶 loop，但仍清理缓冲区边界。
  - 新增 `seek(timeMs: number): Promise<void>`：
    - 暂停 SourceBuffer 追加队列；
    - 调用 `sourceBuffer.remove(start, end)` 清除旧缓冲（或 `abort()` 取消 pending append）；
    - 设置 `video.currentTime = timeMs / 1000`；
    - 恢复追加，等待 `seeked` 事件后发出 `seeked` 事件给 runtime。
  - 新增 `setPlaybackRate(rate: number): void`：校验范围 `0.1..16` 后设置 `video.playbackRate = rate`。
  - 处理 `video.seeking` / `seeked` / `error` 事件并转发到 `onError`。
- `packages/runtime/src/fallback.ts`：`MediaBackend` 接口可选增加 `seek?` 与 `setPlaybackRate?`；`FallbackController` 转发到当前 backend。
- `packages/runtime/src/planner.ts`/`transport.ts`：VOD URL（如 `.mp4`, `.m3u8?type=vod` 或 `playlist_type=VOD`）在 planner 中标记 `isLive: false`，随 `PlaybackPlan` 传递给 backend。
- 单元测试：VOD 模式不触发 live catch-up seek；`setPlaybackRate(2)` 设置 video playbackRate；seek 调用 remove 并更新 currentTime；seek 失败报告错误。

## 3. 完成定义

- [x] 43a 实现完成并通过 `packages/runtime` 与 `crates/cheetah-media-web-bindings` 测试。
- [x] 43b 实现完成并通过 `crates/cheetah-hls-client` 测试。
- [x] 43c 实现完成并通过 `packages/runtime` MSE 测试。
- [x] 合并后运行完整矩阵：
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo test --workspace --no-default-features`
  - `cargo build --workspace --target wasm32-unknown-unknown --no-default-features`
  - `cargo deny check`
  - `corepack pnpm typecheck`
  - `corepack pnpm test`
  - `corepack pnpm build`
- [ ] 无 `todo!()` / `unimplemented!()` / `unwrap()` 在生产路径。
- [ ] 公开 API 变更已同步到 `packages/web` 类型与 README。

## 4. 后续

- 43 完成后进入 WP-44（逐帧/逐关键帧/暂停显示但保持连接）。
