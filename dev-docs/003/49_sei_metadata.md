# WP-49: SEI 提取、TS PES Private Data 与服务端坐标 Overlay

## 1. 目标

在 bitstream 层提取 H.264/H.265 SEI 消息，在 MPEG-TS 解复用层暴露 PES private data，并通过 `CheetahPlayer` 的 metadata 事件把服务端坐标/图形 overlay 投递到 Web 组件，由组件渲染为 SVG overlay。

## 2. 交付物

- `cheetah-media-bitstream` 新增 `sei` 模块：
  - `SeiMessage { payload_type, payload }`。
  - 解析 SEI NAL payload（`payload_type` / `payload_size` 的 `0xFF` 延续）。
  - 支持 H.264 NAL type 6 和 H.265 NAL type 39/40。
  - 对未知 SEI payload type 仍保留原始字节，不 panic。
- `cheetah-container-mpegts` 在 PES header 解析时保留 `PES_private_data`（16 bytes）和 private stream `stream_id`（0xBD/0xBF-0xDF）payload，作为 `MediaPacket` 的 `metadata` 扩展或独立 `PrivateData` track。
- `cheetah-media-types` 或 `cheetah-media-engine` 新增 `MetadataEvent`：
  - 来源 enum：`Sei`、`PesPrivate`、`External`。
  - 字段：`source`, `timestamp_ms`（可选），`payload`/`items`。
- `packages/components`：
  - `CheetahPlayerElement` 新增 `metadata` 事件（`CustomEvent<MetadataEvent>`）。
  - 当 metadata 包含坐标/图形 overlay 时，渲染到 `.overlay-svg` 层；支持 `line`、`rect`、`circle`、`polygon` 和 `text` 基本图形。
  - overlay 清空/更新与视频帧同步，避免残留。

## 3. 完成定义

- `cargo fmt/clippy/test --workspace --all-features` 通过。
- `no_std + alloc` 编译通过（bitstream crate）。
- 无 `todo!()`/`unimplemented()`；对外部输入无 `unwrap()`/`expect()`。
- 测试覆盖 H.264/H.265 SEI 单消息/多消息/空消息/延续字节、畸形输入。
- 测试覆盖 TS PES private data 提取与边界（无 private flag、private stream payload）。
- 测试覆盖 Web 组件 overlay 渲染与清空。

## 4. 边界

- 本 WP 只提取和投递 metadata；AI 分析/检测在后续 WP 作为扩展点。
- overlay 图形能力限定为基本 SVG 形状；复杂样式由调用方提供 style 字符串，组件只负责安全渲染。
- 坐标系约定为归一化 [0,1] 区间，调用方负责缩放；组件在视频显示区域内等比映射。
