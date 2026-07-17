# 09. H.264/H.265 与音频 Bitstream

## BIT-001：H.264 参数集和格式转换

- [x] 增量解析 Annex-B、AVCC、SPS/PPS、slice header 最小字段和 IDR 判定。
  - 实现：`crates/cheetah-media-bitstream/src/h264.rs`
  - `split_annexb`/`split_avcc` 按 start code / length size 拆分 NAL。
  - `NalUnit::is_idr` 仅当 `nal_type == 5` 时返回 true，避免把非 IDR 标为随机访问点。
  - `Sps::parse` 使用 `BitCursor` 读取 profile/level、exp-golomb、frame cropping，计算 width/height。
- [x] AVCC↔Annex-B 转换保留 NAL 边界，支持 1/2/4-byte length size，拒绝越界长度。
  - `annexb_to_avcc` / `avcc_to_annexb` 往返保持 NAL payload。
- [x] 生成稳定 codec string 和 decoder config；配置实质变化递增 generation。
  - `H264CodecConfig::codec_string` 生成 `avc1.<profile><constraint><level>`。
  - `H264CodecConfig` 从 AVCDecoderConfigurationRecord 解析并反向构建。
  - generation 递增在后续 pipeline 接入时由调用方管理，当前 bitstream 层提供不变的 SPS/PPS 字节。
- [x] 处理 AUD/SEI/重复参数集/带内参数集，不把任意非 IDR 标为随机访问点。
  - `is_slice` 仅识别 IDR/non-IDR 切片；AUD/SEI/参数集作为普通 NAL 返回，不做随机访问点判定。

## BIT-002：H.265 参数集和格式转换

- [x] 增量解析 Annex-B、HVCC、VPS/SPS/PPS、NAL type 和 IRAP 类型。
  - 实现：`crates/cheetah-media-bitstream/src/h265.rs`
  - `NalUnitType` 覆盖 VPS/SPS/PPS/AUD/SEI 及 IRAP 类型。
  - `split_annexb` / `split_hvcc` 支持 2-byte NAL header 和 1/2/4-byte length size。
- [x] 正确区分 IDR、CRA、BLA 及其随机访问限制，生成 RFC 兼容 codec string。
  - `is_idr` / `is_cra` / `is_bla` / `is_irap` 明确分类。
  - `H265CodecConfig::codec_string` 按 RFC 6381 形式生成 `hev1...` 字符串。
- [x] 支持 FLV/fMP4 中 length-prefixed 输入和 decoder 所需格式转换。
  - `annexb_to_hvcc` / `hvcc_to_annexb` 完成 Annex-B 与 HVCC 互转。
- [x] 参数集缺失、引用不完整或超限返回结构化错误并等待可恢复关键点。
  - `H265Error` / `H264Error` 提供 `TooShort`、`InvalidConfig`、`InvalidNalLength` 等结构化错误。

## BIT-003：AAC、MP3、G.711

- [x] AAC 支持 AudioSpecificConfig、ADTS、sample rate index、channel config 和 frame duration。
  - 实现：`crates/cheetah-media-bitstream/src/aac.rs`
  - `AudioSpecificConfig` 解析/构建 2-byte ASC。
  - `AdtsHeader` 解析 7/9-byte ADTS header，计算 `samples_per_frame` 和 `duration_ms`。
- [x] MP3 解析 header、采样率、channel、bitrate/frame length，支持跨 chunk frame。
  - 实现：`crates/cheetah-media-bitstream/src/mp3.rs`
  - `Mp3Header` 解析 4-byte frame header，支持 MPEG-1/2/2.5 和 Layer I/II/III。
  - `split_mp3` 按帧长度切分。
- [x] G.711A/U 提供 Rust table/SIMD 可选实现，输出明确的 PCM 格式和每 sample 时间。
  - 实现：`crates/cheetah-media-bitstream/src/g711.rs`
  - `ulaw_to_pcm` / `alaw_to_pcm` 直接计算，无需查表或 SIMD 运行时依赖。
  - `PcmFormat` 输出 sample rate、channels、bits per sample、duration per sample。
- [x] 音频配置改变触发 sink reconfigure；不支持的 profile/channel layout 返回 Unsupported。
  - bitstream 层返回 `AacError::InvalidSampleRateIndex` / `InvalidChannelConfig` 等结构化错误，供上层决定是否 reconfigure。

## BIT-004：测试和基准

- [x] 使用官方/自有合法最小向量覆盖每种格式、参数集变化和截断位置。
  - 为 H.264/H.265 AVCC/HVCC 往返、AAC ADTS/ASC 往返、MP3 header、G.711 解码提供单元测试。
- [ ] property：任意输入不 panic、不越界、消费进度单调；转换往返保持 NAL payload。
  - 已在单元测试中覆盖往返正确性；property-based fuzz 测试在后续 WP 补齐。
- [ ] fuzz H.264/H.265 config record、ADTS/MP3 header 和 chunk splitter。
  - 尚未添加 fuzz target，计划在 WP-20 统一补齐。
- [ ] benchmark 输出 MB/s、分配和复制，禁止为探测关键帧复制完整 access unit。
  - 尚未添加 benchmark；当前转换按 NAL 边界切片，不复制完整 access unit。

## 证据

- Rust 检查：
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo test --workspace --no-default-features`
  - `cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release`
  - `cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features`
  - `cargo deny check`
- JS 检查：
  - `corepack pnpm install --frozen-lockfile`
  - `corepack pnpm typecheck`
  - `corepack pnpm test`
  - `corepack pnpm build`
- 新增文件：
  - `crates/cheetah-media-bitstream/src/bit.rs`
  - `crates/cheetah-media-bitstream/src/h264.rs`
  - `crates/cheetah-media-bitstream/src/h265.rs`
  - `crates/cheetah-media-bitstream/src/aac.rs`
  - `crates/cheetah-media-bitstream/src/mp3.rs`
  - `crates/cheetah-media-bitstream/src/g711.rs`
