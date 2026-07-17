# WP-70: 双向引擎抽象

## 1. 目标

在 `cheetah-media-engine` 中新增 `broadcast` 模块（feature `bidirectional`），定义 Phase 7 双向实时引擎的核心 trait 与管线：

- `CaptureSource`：采集源抽象（摄像头 / 麦克风 / 屏幕 / 自定义）。
- `Processor`：帧前处理抽象（缩放、水印、格式转换等）。
- `Encoder`：编码器抽象（H.264/H.265/Opus/AAC/G.711）。
- `PublisherBackend`：发布路径抽象（WebRTC / RTMP 等）。
- `BroadcastPipeline`：把上述阶段串成 one-tick 推进管线，并复用 `cheetah-media-engine` 的 `ResourceLedger` 与 `Metrics`。
- `BroadcastEngine`：生命周期状态机（Idle / Starting / Broadcasting / Stopping / Stopped / Failed / Destroyed）。

本 WP 只做抽象、trait 契约与占位实现；真实采集 / 编码 / 发布分别在 WP-71~73 实现。禁止假成功，未实现能力返回稳定的 `MediaError::Unsupported`。

## 2. 交付

- `crates/cheetah-media-engine/src/broadcast/mod.rs`：模块入口与类型重导出。
- `crates/cheetah-media-engine/src/broadcast/frame.rs`：`MediaFrame`（`Video`/`Audio`）与转换辅助。
- `crates/cheetah-media-engine/src/broadcast/source.rs`：`CaptureSource` trait 与 `UnsupportedCaptureSource` 占位。
- `crates/cheetah-media-engine/src/broadcast/processor.rs`：`Processor` trait 与 `PassThroughProcessor`。
- `crates/cheetah-media-engine/src/broadcast/encoder.rs`：`Encoder` trait、`EncoderCapability`、`UnsupportedEncoder`。
- `crates/cheetah-media-engine/src/broadcast/publisher.rs`：`PublisherBackend` trait 与 `UnsupportedPublisherBackend`。
- `crates/cheetah-media-engine/src/broadcast/pipeline.rs`：`BroadcastPipeline` 把 source → processor → encoder → publisher 串起来，每次 `tick` 推进一帧。
- `crates/cheetah-media-engine/src/broadcast/engine.rs`：`BroadcastEngine` 状态机与命令/事件。
- `crates/cheetah-media-engine/src/broadcast/registry.rs`：`CaptureSourceRegistry`、`EncoderRegistry`、`PublisherRegistry`，按 `CodecId`/能力选择后端。
- `cheetah-media-engine/Cargo.toml`：新增 `bidirectional` feature 并暴露模块。
- `dev-docs/005_mobile_and_bidirectional.md`：WP-70 状态更新。

## 3. 接口草图

```rust
pub enum MediaFrame<'a> {
    Video(VideoFrame<'a>),
    Audio(AudioFrame<'a>),
}

pub trait CaptureSource: Send {
    fn start(&mut self) -> Result<(), MediaError>;
    fn stop(&mut self) -> Result<(), MediaError>;
    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError>;
    fn kind(&self) -> &'static str;
}

pub trait Processor: Send {
    fn process(&mut self, frame: &MediaFrame<'static>) -> Result<MediaFrame<'static>, MediaError>;
    fn kind(&self) -> &'static str;
}

pub trait Encoder: Send {
    fn configure(&mut self, codec: CodecId, width: u32, height: u32, fps: u32) -> Result<(), MediaError>;
    fn encode(&mut self, frame: &MediaFrame<'static>) -> Result<MediaPacket<'static>, MediaError>;
    fn request_keyframe(&mut self) -> Result<(), MediaError>;
    fn set_bitrate(&mut self, bps: u32) -> Result<(), MediaError>;
    fn kind(&self) -> &'static str;
}

pub trait PublisherBackend: Send {
    fn connect(&mut self, url: &str) -> Result<(), MediaError>;
    fn publish(&mut self, packet: &MediaPacket<'static>) -> Result<(), MediaError>;
    fn flush(&mut self) -> Result<(), MediaError>;
    fn disconnect(&mut self);
    fn kind(&self) -> &'static str;
}
```

`BroadcastPipeline` 持有一个 source、零个或多个 processor、一个 encoder、一个 publisher，以及 `ResourceLedger` + `Metrics` 引用；`tick()` 执行一次完整推进。

`BroadcastEngine` 的命令：

```rust
pub enum BroadcastCommand {
    Start,
    Stop,
    RequestKeyframe,
    SetBitrate(u32),
    Destroy,
}

pub enum BroadcastEvent {
    StateChanged { from: BroadcastState, to: BroadcastState },
    PacketPublished { sequence: u64, codec: CodecId },
    Error(MediaError),
}
```

## 4. 验证命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p cheetah-media-engine --features bidirectional
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

- [x] 创建 WP-70 计划文档
- [ ] 实现 `broadcast` 模块 trait 与占位
- [ ] 接入 `cheetah-media-engine` 的 `bidirectional` feature
- [ ] Rust/JS 验证矩阵通过
- [ ] CI / Devin Review 通过并合并
