# WP-37 H.264 Annex-B 裸流解析器

## 1. 目标

交付 `crates/cheetah-container-annexb`，提供增量式 H.264 Annex-B 字节流解复用，输出 `cheetah-media-types::MediaPacket`。本 WP 先聚焦 H.264；H.265 扩展在 WP-38 实现，避免在本阶段引入未验证的 H.265 VPS/SPS 解析。

## 2. 依赖

- `cheetah-media-types`：Track/Packet/TimeBase/CodecId
- `cheetah-media-bitstream`：H.264 NAL 拆分、SPS 解析、`unescape_rbsp`、AVCC 构建辅助

## 3. 交付物

### 3.1 Crate 骨架

- `crates/cheetah-container-annexb/Cargo.toml`：对齐 workspace 版本、edition、license、lint；`default-features = false` 支持 `no_std + alloc`。
- `crates/cheetah-container-annexb/README.md`：职责、允许/禁止依赖、feature 说明。
- 源文件拆分：
  - `lib.rs`：公共导出；
  - `error.rs`：`AnnexbError`；
  - `demuxer.rs`：`AnnexBConfig`、`AnnexBDemuxer`、`AnnexbEvent`；
  - `param_sets.rs`：参数集缓存与 `AvcC` 配置生成；
  - `tests.rs`：单元/回归测试。

### 3.2 功能

- 支持 3 字节 (`00 00 01`) 和 4 字节 (`00 00 00 01`) start code 混合出现。
- 识别 start code 时跳过 H.264 emulation prevention three bytes (`00 00 03 XX`，`XX <= 0x03`)，避免把 payload 中的转义序列误判为 start code。
- 按 NAL unit 输出 `MediaPacket`：
  - `payload` 为原始 Annex-B NAL 数据（含 NAL header），保持 EPB 不变，供后端解码器直接使用；
  - `flags.is_keyframe` 对 `nal_type == 5` (IDR) 置位；
  - 时间戳由调用方通过 `AnnexBConfig::timebase` 和内部单调 `sequence` 推导（Annex-B 裸流本身无时间戳）。
- 检测并缓存 SPS (`nal_type == 7`) 和 PPS (`nal_type == 8`)。
- 当首次收集到 SPS+PPS 或参数集变更时，生成 `Track` 事件，附带 `AvcC` 配置记录和从 SPS 解析的宽/高/codec string。
- 可配置 `max_nal_size_bytes`（默认 16 MiB）和 `max_buffer_bytes`（默认 32 MiB）；超限返回稳定 `AnnexbError::NalTooLarge` / `AnnexbError::BufferExceeded`。

### 3.3 接口

```rust
pub struct AnnexBConfig { ... }
pub enum AnnexbEvent { Track(TrackInfo), Packet(MediaPacket<'static>), Eof }
pub struct AnnexBDemuxer { ... }

impl AnnexBDemuxer {
    pub fn new(config: AnnexBConfig) -> Self;
    pub fn push(&mut self, data: &[u8]);
    pub fn next_event(&mut self) -> Result<Option<AnnexbEvent>, AnnexbError>;
    pub fn end(&mut self) -> Result<(), AnnexbError>;
    pub fn reset(&mut self);
    pub fn buffer_len(&self) -> usize;
}
```

## 4. 完成定义

- [x] `cargo fmt --all --check` 通过。
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- [x] `cargo test --workspace --all-features` 通过。
- [x] `cargo test --workspace --no-default-features` 通过。
- [x] `cargo build --workspace --target wasm32-unknown-unknown --no-default-features` 通过。
- [x] `cargo deny check` 通过。
- [x] 测试覆盖：
  - golden 正常 H.264 Annex-B 流（生成的 fixture）产生正确 Track 和 Packet 数量；
  - SPS/PPS 变更触发新的 `Track` 事件；
  - 3 字节与 4 字节 start code 边界、跨包切片、首个 start code 前的前导零；
  - 包含 emulation prevention bytes 的 payload 不被误判为 start code；
  - 畸形输入（无 start code、超大 NAL、无 SPS 的 IDR）返回稳定错误；
  - 空 `push`、`end` 后 `Eof`、多次 `reset`。
