# WP-63: Android 播放骨架

## 1. 目标

建立 `crates/cheetah-media-android`，为 Android 平台提供与现有 `cheetah-media-abi` 和 `cheetah-media-native-*` 兼容的播放能力入口。本 WP 是基线：crate 结构、`MediaCodec` 能力探测、`Decoder`/`Renderer`/`AudioSink` 接口实现，以及 JNI 生命周期钩子。由于当前 CI 环境未链接 Android NDK，实际 `MediaCodec` 调用将在 WP-64 及真机环境中实现；本 WP 禁止假支持，未链接平台 SDK 时探测结果必须为空或明确返回 `Unsupported`。

## 2. 交付

- `crates/cheetah-media-android/Cargo.toml`：加入 workspace，依赖 `cheetah-media-abi`、`cheetah-media-types`、`cheetah-media-native-decoder`（probe trait）。
- `src/lib.rs`：crate 入口，暴露 `AndroidMediaCodecProbe`、`AndroidDecoder`、`AndroidRenderer`、`AndroidAudioSink`、JNI 生命周期函数。
- `src/probe.rs`：`MediaCodec` 能力探测，当前 CI 无 NDK 时返回空能力集。
- `src/decoder.rs`：`AndroidDecoder` 实现 `cheetah_media_abi::Decoder`；非 Android 目标返回 `AbiError::NotSupported`。
- `src/renderer.rs`：`AndroidRenderer` 实现 `cheetah_media_abi::Renderer`；非 Android 目标返回 `AbiError::NotSupported`。
- `src/audio.rs`：`AndroidAudioSink` 实现 `cheetah_media_abi::AudioSink`；非 Android 目标返回 `AbiError::NotSupported`。
- `src/jni.rs`：JNI 入口函数声明（`JNI_OnLoad`、`create`、`destroy` 等），当前仅做生命周期计数与错误转发，不调用 `MediaCodec`。
- `cheetah-media-engine` 增加 `android` feature，在 `native` feature 激活时可选择接入 Android 探测。
- 单元测试覆盖：非 Android 目标下探测为空、解码/渲染/音频返回 `NotSupported`、生命周期计数正确。

## 3. 接口草图

```rust
pub struct AndroidMediaCodecProbe;
impl cheetah_media_native_decoder::probe::Probe for AndroidMediaCodecProbe {
    fn name(&self) -> &'static str { "android-mediacodec" }
    fn probe(&self) -> Vec<DecoderCapability> { Vec::new() /* NDK not linked */ }
}

pub struct AndroidDecoder;
impl cheetah_media_abi::Decoder for AndroidDecoder {
    fn decode(&mut self, _input: &Input) -> Result<Output, AbiError> {
        Err(AbiError::NotSupported)
    }
    fn flush(&mut self) -> Result<(), AbiError> { Ok(()) }
}
```

## 4. 验证命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p cheetah-media-android
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo deny check
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
```

## 5. 状态

- [x] 创建 WP-63 计划文档
- [x] 实现 `cheetah-media-android` crate 骨架
- [x] 接入 `cheetah-media-engine` 的 `android` feature
- [x] Rust/JS 验证矩阵通过
- [ ] CI / Devin Review 通过并合并
