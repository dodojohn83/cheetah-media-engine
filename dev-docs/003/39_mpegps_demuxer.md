# WP-39 MPEG-PS 容器解复用

## 1. 目标

交付 `crates/cheetah-container-mpegps`，将 MPEG-2 Program Stream（MPEG-PS）字节流解析为 `MediaPacket`。作为 `003_web_pro_feature_parity.md` Phase 4 的第三个工作包，为后续 HTTP/WS MPEG-PS 传输（WP-40）提供输入。

## 2. 依赖

- `cheetah-media-types`：`TrackInfo`、`MediaPacket`、`CodecId`、`MediaTime`、`TimeBase`、`Timestamp`。
- `cheetah-media-bitstream`：PES 时间戳解析、`ByteCursor`、codec 识别辅助（ADTS/MP3/H.264/H.265 NAL）。
- `cheetah-container-annexb`：视频 PES 负载通常已是 Annex-B ES；本 crate 需要把视频 PES 组装后喂给 Annex-B demuxer 或复用其 `find_start_code` 逻辑来切片并生成 `MediaPacket`。

## 3. 交付物

### 3.1 Crate 骨架

- `crates/cheetah-container-mpegps/Cargo.toml`：workspace 对齐、`no_std + alloc` 支持、`unsafe_code = "forbid"`。
- `crates/cheetah-container-mpegps/README.md`：职责、允许/禁止依赖、feature 说明。
- 源文件拆分（均 <=500 行）：
  - `lib.rs`：公共导出。
  - `error.rs`：`MpegPsError`。
  - `pack.rs`：`PackHeader`、SCR 解析、pack stuffing、system header 跳过。
  - `pes.rs`：PES packet 解析、PTS/DTS 提取、`is_video_stream` / `is_audio_stream`。
  - `demuxer.rs`：`MpegPsConfig`、`MpegPsDemuxer`、`MpegPsEvent`（`Track`、`Packet`、`NeedMore`、`Eof`）。
  - `tests.rs`：单元/回归测试。

### 3.2 功能

- 识别 `pack_start_code` (`0x00 0x00 0x01 0xBA`)，解析 pack SCR（90 kHz）、mux rate、stuffing bytes。
- 跳过 `system_header` (`0x000001BB`) 和 `program_stream_map` (`0x000001BC`) 等本 WP 不消费的包（识别并丢弃，避免误判为 PES）。
- 识别 PES start code（`0x000001` + stream_id），按 PES `packet_length` 组装完整 PES packet。
- 解析 PES header，提取 PTS/DTS，构造 `MediaTime`（timebase 90 kHz）。
- 视频流（stream_id `0xE0..0xEF`）：将 PES payload 作为 Annex-B 裸流，使用 `cheetah-container-annexb` 的 start-code 扫描切片并生成 `MediaPacket`；支持 `CodecId::H264` / `H265`，由配置指定或从首个 NAL header 推断（H.265 为 2 字节）。
- 音频流（stream_id `0xC0..0xDF` 或 `0xBD`）：本 WP 先支持 AAC ADTS 识别与分帧；MP3/G.711 作为后续 WP 扩展。
- 可配置的 `max_packet_size_bytes`（默认 4 MiB）和 `max_buffer_bytes`（默认 32 MiB）；超限返回稳定 `MpegPsError`。

### 3.3 接口

```rust
pub struct MpegPsConfig {
    pub video_codec: CodecId, // H264 or H265
    pub max_packet_size_bytes: usize,
    pub max_buffer_bytes: usize,
}

pub enum MpegPsEvent {
    Track(TrackInfo),
    Packet(MediaPacket<'static>),
    Eof,
}

pub struct MpegPsDemuxer { ... }

impl MpegPsDemuxer {
    pub fn new(config: MpegPsConfig) -> Self;
    pub fn push(&mut self, data: &[u8]);
    pub fn next_event(&mut self) -> Result<Option<MpegPsEvent>, MpegPsError>;
    pub fn end(&mut self) -> Result<(), MpegPsError>;
    pub fn reset(&mut self);
}
```

## 4. 完成定义

- [x] `cargo fmt --all --check` 通过。
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- [x] `cargo test --workspace --all-features` 通过。
- [x] `cargo test --workspace --no-default-features` 通过。
- [x] `cargo build --workspace --target wasm32-unknown-unknown --no-default-features` 通过。
- [x] `cargo deny check` 通过（仅有预存 license-not-encountered 警告）。
- [x] `corepack pnpm typecheck/test/build` 通过。
- [x] 测试覆盖：
  - 正常 PS pack + 视频 PES（H.264）产生正确 Track 和 Packet；
  - pack stuffing、system header 跳过；
  - 跨包/不完整 PES、PES length 边界；
  - PTS/DTS 提取与时间基转换；
  - 畸形/空输入返回稳定错误；
  - 音频 AAC PES 识别；
- [x] 无 `todo!()`/`unimplemented!()`；对外部输入无 `unwrap()`/`expect()`。
