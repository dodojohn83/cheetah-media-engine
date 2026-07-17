# WP-38 H.265 Annex-B 裸流解析器

## 1. 目标

扩展 `crates/cheetah-container-annexb`，支持增量式 H.265/HEVC Annex-B 字节流解复用，输出 `cheetah-media-types::MediaPacket`。补齐 `cheetah-media-bitstream` 中缺失的最小 H.265 VPS/SPS 解析，使 Annex-B 工作包（WP-37）中的 H.264 实现可复用、不复制 parser。

## 2. 依赖

- `cheetah-media-types`：Track/Packet/TimeBase/CodecId
- `cheetah-media-bitstream`：H.265 NAL 类型、共享 `unescape_rbsp`、VPS/SPS `ProfileTierLevel` 解析、HvcC 构建辅助
- `cheetah-container-annexb` 已有 start code 扫描器（WP-37）

## 3. 交付物

### 3.1 Bitstream 扩展

- `crates/cheetah-media-bitstream/src/rbsp.rs`：H.264/H.265 共享的 emulation prevention 字节去除函数 `unescape_rbsp`（已在 WP-37 提前抽出）。
- `crates/cheetah-media-bitstream/src/h265/parameter_sets.rs`：
  - `ProfileTierLevel`：解析通用 profile/tier/level；支持 sub-layer 跳过。
  - `Vps`：解析视频参数集，提取 `max_sub_layers_minus1` 和 `profile_tier_level`。
  - `Sps`：解析序列参数集，提取 `chroma_format_idc`、`separate_colour_plane_flag`、`pic_width_in_luma_samples`、`pic_height_in_luma_samples`、conformance 裁剪、`bit_depth_luma_minus8`、`bit_depth_chroma_minus8`、`profile_tier_level`、`max_sub_layers_minus1`、`temporal_id_nesting_flag`。
  - 故意在 bit depth 之后停止，不读取 scaling list、short term RPS、VUI 等，因为 Annex-B 解复用只需要宽高/profile/level/色度格式。

### 3.2 Annex-B crate 扩展

- `crates/cheetah-container-annexb/src/param_sets.rs`：
  - 新增 `H265ParameterSetCache`：缓存 VPS/SPS/PPS；构建 `CodecConfig::HevcC`。
  - 新增统一 `ParameterSetCache` 枚举，使 `AnnexBDemuxer` 同时支持 H.264/H.265。
- `crates/cheetah-container-annexb/src/demuxer.rs`：
  - `AnnexBConfig::h265(...)` 构造器。
  - 接受 `CodecId::H264` 或 `CodecId::H265`，否则返回 `AnnexbError::UnsupportedCodec`。
  - 使用 `ParameterSetCache` 消费参数集 NAL，并检测参数集变更后触发 `Track` 事件。
  - H.265 关键帧识别：IRAP NAL 类型 `16..=23`（含 IDR 19/20 及 CRA/BLA）。

### 3.3 公共接口

同 WP-37，仅 `AnnexBConfig` 新增 `h265` 构造器：

```rust
pub struct AnnexBConfig { ... }
pub enum AnnexbEvent { Track(TrackInfo), Packet(MediaPacket<'static>), Eof }
pub struct AnnexBDemuxer { ... }

impl AnnexBConfig {
    pub fn h264(track_id: TrackId, timebase: TimeBase) -> Self;
    pub fn h265(track_id: TrackId, timebase: TimeBase) -> Self;
}

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
- [x] Web 验证矩阵（`pnpm install --frozen-lockfile`、`typecheck`、`test`、`build`）通过。
- [x] 测试覆盖：
  - 正常 VPS/SPS/PPS + slice 流产生 `Track` 和 `Packet`；
  - 无 VPS（仅 SPS/PPS）时回退到 SPS 的 profile/level；
  - 参数集变更触发新 `Track`；
  - 3 字节与 4 字节 start code 边界；
  - H.265 emulation prevention bytes 不被误判为 start code；
  - HvcC 配置记录保留原始 NAL 字节（含 EPB）并 round-trip 可解析；
  - 畸形/空输入返回稳定错误；
  - IRAP 关键帧识别（含 IDR 19/20 与 CRA/BLA 21/22/23）。
- [x] 源文件不超过 500 行；无 `todo!()`/`unimplemented!()`；对外部输入无 `unwrap()`/`expect()`。
