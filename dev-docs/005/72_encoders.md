# WP-72: 平台编码器能力

## 1. 目标

为 `cheetah-media-engine` 的 `broadcast` 双向引擎补充真实编码器占位与注册表能力选择，使 `BroadcastPipeline` 能够驱动 G.711 音频编码并通过注册表选择编码器。

- 扩展 `Encoder` trait：
  - `encode` 接收 `TrackId`、`StreamEpoch`、`SequenceNumber`，以便生成完整的 `MediaPacket`。
  - 新增 `capabilities(&self) -> &[EncoderCapability]`，`supports(codec)` 提供基于 capabilities 的默认实现。
- 提供 `G711Encoder`：纯 Rust 实现，把 S16/F32 PCM `AudioFrame` 编码为 A-law/μ-law 字节流，返回 `MediaPacket`。
- 提供 `H264Encoder`、`H265Encoder`、`OpusEncoder`、`AacEncoder` 主机占位，未链接平台 SDK 时返回 `MediaError::Unsupported`。
- 提供 `MockEncoder` 用于 pipeline 头less 测试。
- 更新 `BroadcastPipeline` 在 `tick` 时把 `track_id`/`stream_epoch`/`sequence` 传给 `Encoder::encode`。
- 更新 `EncoderRegistry`：按 `capabilities` 中匹配 codec 的 `priority` 选择最优编码器。
- `cheetah-media-engine` 在 `bidirectional` feature 下启用 `cheetah-media-bitstream` 依赖，用于 G.711 编解码函数。

## 2. 交付

- `crates/cheetah-media-engine/src/broadcast/encoders.rs`：所有具体编码器与 `MockEncoder`。
- `crates/cheetah-media-engine/src/broadcast/encoder.rs`：trait 扩展与 `UnsupportedEncoder` 更新。
- `crates/cheetah-media-engine/src/broadcast/pipeline.rs`：把 packet 元数据传入 `encode`。
- `crates/cheetah-media-engine/src/broadcast/registry.rs`：按 capability priority 选择。
- `crates/cheetah-media-engine/src/broadcast/mod.rs`：重导出新增类型。
- `crates/cheetah-media-engine/Cargo.toml`：`bidirectional` feature 加入 `cheetah-media-bitstream`。
- `dev-docs/005/72_encoders.md`、状态更新。

## 3. 接口草图

```rust
pub trait Encoder: Send {
    fn configure(&mut self, codec: CodecId, width: u32, height: u32, fps: u32) -> Result<(), MediaError>;
    fn encode(
        &mut self,
        frame: &MediaFrame<'static>,
        track_id: TrackId,
        stream_epoch: StreamEpoch,
        sequence: SequenceNumber,
    ) -> Result<MediaPacket<'static>, MediaError>;
    fn request_keyframe(&mut self) -> Result<(), MediaError>;
    fn set_bitrate(&mut self, bps: u32) -> Result<(), MediaError>;
    fn kind(&self) -> &'static str;
    fn capabilities(&self) -> &[EncoderCapability];
    fn supports(&self, codec: CodecId) -> bool {
        self.capabilities().iter().any(|c| c.codec == codec)
    }
}
```

```rust
pub struct G711Encoder { kind: G711Kind }
```

`G711Encoder` 在 `configure` 中接受 `CodecId::G711A` / `CodecId::G711U`，`encode` 中：
- 仅接受 `MediaFrame::Audio`。
- 支持 `SampleFormat::S16` 与 `SampleFormat::F32`（native little-endian 解释）。
- 输出 `MediaPacket` 关键帧标记为 `true`（音频）。

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

- [x] 创建 WP-72 计划文档
- [x] 扩展 `Encoder` trait 与 `UnsupportedEncoder`
- [x] 实现 G.711 编码器与平台占位编码器
- [x] 更新 `BroadcastPipeline` packet 元数据传递
- [x] 更新 `EncoderRegistry` 按 priority 选择
- [x] Rust/JS 验证矩阵通过
- [x] CI / Devin Review 通过并合并（PR #73）
