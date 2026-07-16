# WP-40b WASM demuxer bindings

## 1. 目标

在 `cheetah-media-web-bindings` 中暴露 `AnnexBDemuxer` 和 `MpegPsDemuxer`，让 JS runtime 可以把 HTTP/WS 收到的字节流推进 WASM，并拉出 `MediaPacket` 描述符，为 40c 的 `RawStreamBackend` 提供输入。

## 2. 依赖

- WP-37 `crates/cheetah-container-annexb`
- WP-38 `crates/cheetah-container-annexb`（H.265 在 `AnnexBConfig` 中区分）
- WP-39 `crates/cheetah-container-mpegps`
- WP-32 `cheetah-media-web-bindings` / `MemoryArena` / `MemoryDescriptor`

## 3. 交付物

### 3.1 新增 WASM 绑定

在 `crates/cheetah-media-web-bindings/src/demux.rs` 中实现：

- `AnnexBDemuxer`：
  - `new(video_codec: u8, max_nal_size: u32, max_buffer: u32) -> Self`
  - `push(data: &[u8])`
  - `end()`
  - `reset()`
  - `next_event() -> Option<DemuxEvent>`
- `MpegPsDemuxer`：
  - `new(video_codec: u8, max_packet_size: u32, max_buffer: u32, max_nal_size: u32) -> Self`
  - 同样 `push`/`end`/`reset`/`next_event`。

`video_codec` 编码：
- `0` -> `CodecId::H264`
- `1` -> `CodecId::H265`

### 3.2 `DemuxEvent` 描述符

在 `crates/cheetah-media-web-bindings/src/demux.rs` 中定义 `#[wasm_bindgen]` 结构体：

```rust
pub enum DemuxEventKind {
    Track = 0,
    Packet = 1,
    Eof = 2,
    Error = 3,
}
```

`DemuxEvent` 字段：
- `kind: u8`
- `track_id: u32`
- `codec: u8`（仅 Track；0=H264,1=H265,2=AAC,255=unknown）
- `width`, `height`: u32（Track video）
- `sample_rate`, `channels`: u32（Track audio）
- `config_slot`, `config_generation`, `config_len`: u32（Track codec config 在 demuxer 自有 arena 中的描述符）
- `data_slot`, `data_generation`, `data_offset`, `data_len`: u32（Packet payload 描述符）
- `pts_ms`, `dts_ms`, `duration_ms`: i64
- `flags`: u32（`is_keyframe` bit 0）
- `error_code`: u32（Error 事件）
- `error_message`: String（Error 事件，简短静态字符串）

每个 demuxer 实例内部持有独立的 `MemoryArena`，用于存放：
- 最近一次 `Track` 的 codec config 字节；
- 最近一次 `Packet` 的 payload 字节。

`next_event` 在返回 `Packet`/`Track` 之前把数据写入 arena 并填充描述符；JS 通过 `read_region(slot, generation)` 读取（复用 `WebEngine` 的 arena 读取接口或新增 `read_demuxer_region`）。

### 3.3 与 `WebEngine` 隔离

Demuxer 绑定独立成 `DemuxInstance`，不耦合 `WebEngine` 的生命周期。JS runtime 在 `RawStreamBackend` 中创建并管理 demuxer，40c 再把 packet 描述符转换为 `EncodedVideoChunk` / `EncodedAudioChunk`。

### 3.4 测试

- Rust unit tests in `crates/cheetah-media-web-bindings` (using `wasm-bindgen-test` is optional; we run with `cargo test --target wasm32-unknown-unknown` if available, otherwise `cargo test` under `cfg(not(target_arch = "wasm32"))` with simulated binding types).
- For host tests, use a small `MockJs` pattern or export a non-wasm `DemuxSession` under `#[cfg(test)]` that returns the same descriptors without `wasm_bindgen` wrappers.
- JS planner tests（已存在）不需要变化；40c 再补充 E2E。

## 4. 完成定义

- `cargo fmt/clippy --workspace --all-features` 通过。
- `cargo build --workspace --target wasm32-unknown-unknown --no-default-features` 通过。
- `cheetah-media-web-bindings` 新增文件 <=500 行；超过则拆分为 `demux.rs` / `demux_event.rs` / `demux_arena.rs`。
- 无 `todo!()` / `unimplemented!()` / 对外部输入 `unwrap()`/`expect()`。
- `DemuxEvent` 不通过 JSON/Base64 序列化媒体负载；payload 通过 `MemoryDescriptor` 描述。

## 5. 后续

- 40c：在 `packages/runtime/src/raw.ts` 实现 `RawStreamBackend`，把 `DemuxEvent` 转换成 `EncodedVideoChunk` / `EncodedAudioChunk`，并接入 `FallbackController` 与 `transport.ts`。

PR: `wp/40b-wasm-demuxer-bindings` → `wp/16-engine-state-machine`。
