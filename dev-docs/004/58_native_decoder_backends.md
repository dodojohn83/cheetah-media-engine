# WP-58: 平台硬解探测与回退

## 1. 范围

建立 `cheetah-media-native-decoder` crate，为原生平台提供硬解能力探测、注册表与回退选择：

- 统一 `PlatformApi` 与 `DecoderCapability` 能力描述（codec、profile/level、分辨率/帧率/位深、并发实例、zero-copy surface 类型、优先级）。
- `Probe` trait 与平台探测实现：Media Foundation、VideoToolbox、VA-API、Vulkan Video、Software。
- `CapabilityRegistry`：合并各平台探测结果，按 codec 与约束选择最佳后端。
- `NativeDecoder`：实现 `cheetah_media_abi::Decoder`，内部维护多个后端并按优先级回退；失败时返回 `AbiError::NotSupported`。
- 真实软件解码器：`G711Decoder`（A-law / μ-law -> 16-bit PCM），证明 `Decoder` 接口可用并覆盖对讲音频路径。
- 平台硬解实现以 stub 形式存在：当前 Linux/Windows/macOS 真机 SDK 未在 CI 中链接，探测返回空，避免虚假声明硬解可用。

## 2. 交付物

- `crates/cheetah-media-native-decoder/Cargo.toml`
- `crates/cheetah-media-native-decoder/src/lib.rs`
- `crates/cheetah-media-native-decoder/src/capability.rs`
- `crates/cheetah-media-native-decoder/src/probe.rs`
- `crates/cheetah-media-native-decoder/src/registry.rs`
- `crates/cheetah-media-native-decoder/src/decoder.rs`
- `crates/cheetah-media-native-decoder/src/g711.rs`
- `dev-docs/004/58_native_decoder_backends.md` 与 baseline 状态更新

## 3. 接口草案

```rust
use cheetah_media_abi::{Decoder, Input, Output};
use cheetah_media_types::CodecId;

pub enum PlatformApi { MediaFoundation, VideoToolbox, VaApi, VulkanVideo, Software }
pub struct DecoderCapability { ... }

pub struct CapabilityRegistry { ... }
impl CapabilityRegistry {
    pub fn new() -> Self;
    pub fn add(&mut self, cap: DecoderCapability);
    pub fn select(&self, codec: CodecId, width: u32, height: u32, fps: u32) -> Option<PlatformApi>;
    pub fn select_audio(&self, codec: CodecId) -> Option<PlatformApi>;
}

pub struct NativeDecoder { ... }
impl NativeDecoder {
    pub fn from_registry(registry: &CapabilityRegistry, codec: CodecId) -> Result<Self, AbiError>;
    pub fn with_backends(backends: Vec<Box<dyn Decoder + Send>>) -> Self;
}
impl Decoder for NativeDecoder { ... }
```

## 4. 验证

```bash
cargo test -p cheetah-media-native-decoder
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [x] crate 创建与能力模型
- [x] 平台探测 stub 与注册表选择
- [x] `NativeDecoder` 回退与 `G711Decoder`
- [x] Rust 全矩阵验证通过
- [x] CI / Devin Review 通过并合并
