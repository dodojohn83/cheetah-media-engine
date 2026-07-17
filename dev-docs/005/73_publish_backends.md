# WP-73: 发布路径与拥塞控制

## 1. 目标

为 `cheetah-media-engine` 的 `broadcast` 双向引擎补充发布后端路径（WebRTC/RTMP）占位与拥塞反馈驱动的动态码率。

- 扩展 `PublisherBackend` trait：
  - 新增 `poll_feedback(&mut self) -> Option<BitrateFeedback>`，由网络后端报告拥塞/丢包/RTT。
- 新增 `BitrateFeedback`：
  - `target_bitrate_bps: Option<u32>`（建议目标码率）。
  - `loss_fraction: Option<u8>`（0-255）。
  - `rtt_ms: Option<u32>`。
- 提供 `WebRtcPublisherBackend` 与 `RtmpPublisherBackend` 主机占位；未链接平台 SDK 时 `connect`/`publish`/`flush` 返回 `MediaError::Unsupported`，`poll_feedback` 返回 `None`。
- 提供 `MockPublisher` 用于 pipeline 测试，可注入反馈。
- `BroadcastPipeline::tick` 在 `publish` 成功后调用 `poll_feedback`；若反馈包含 `target_bitrate_bps`，调用 `Encoder::set_bitrate` 应用。
- 新增 `BroadcastCommand::ApplyFeedback(BitrateFeedback)` 与 `BroadcastEvent::BitrateChanged`。
- `PublisherBackendRegistry` 保留并按 URL scheme 选择（`webrtc://`、`rtmp://`、`mock://`）。

## 2. 交付

- `crates/cheetah-media-engine/src/broadcast/publisher.rs`：`PublisherBackend` trait扩展、`BitrateFeedback`、`UnsupportedPublisherBackend`、`WebRtcPublisherBackend`、`RtmpPublisherBackend`。
- `crates/cheetah-media-engine/src/broadcast/pipeline.rs`：`tick` 调用反馈与 `set_bitrate`。
- `crates/cheetah-media-engine/src/broadcast/engine.rs`：`ApplyFeedback` 命令与 `BitrateChanged` 事件。
- `crates/cheetah-media-engine/src/broadcast/mod.rs`：重导出新增类型。
- `dev-docs/005/73_publish_backends.md`、状态更新。

## 3. 接口草图

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BitrateFeedback {
    pub target_bitrate_bps: Option<u32>,
    pub loss_fraction: Option<u8>,
    pub rtt_ms: Option<u32>,
}

pub trait PublisherBackend: Send {
    ...
    fn poll_feedback(&mut self) -> Option<BitrateFeedback>;
}

pub enum BroadcastCommand {
    ...
    ApplyFeedback(BitrateFeedback),
}

pub enum BroadcastEvent {
    ...
    BitrateChanged { bps: u32 },
}
```

## 4. 验证命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo deny check
( cd crates/cheetah-media-c-bindings && cbindgen --config cbindgen.toml --crate cheetah-media-c-bindings --output /tmp/cheetah_media.h && diff -u include/cheetah_media.h /tmp/cheetah_media.h )
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
```

## 5. 状态

- [x] 创建 WP-73 计划文档
- [x] 扩展 `PublisherBackend` 与 `BitrateFeedback`
- [x] 实现 WebRTC/RTMP 占位发布后端
- [x] `BroadcastPipeline` 根据反馈调整码率
- [x] `BroadcastEngine` 支持 `ApplyFeedback` 命令
- [x] Rust/JS 验证矩阵通过
- [x] CI / Devin Review 通过并合并（PR #74）
