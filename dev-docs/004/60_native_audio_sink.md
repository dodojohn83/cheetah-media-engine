# WP-60: 平台 audio sink 与 A/V sync

## 1. 范围

建立 `cheetah-media-native-audio` crate，为原生平台提供音频输出 sink 抽象、A/V 同步控制与平台探测：

- 统一 `PlatformAudioSink` 与 `AudioSinkCapability` 能力描述。
- `AudioSinkProbe` 与平台探测实现：ALSA / PulseAudio / CoreAudio / WASAPI / WASAPI / Null。
- `NullAudioSink`：不依赖任何平台音频 API，可累积提交的 PCM 帧并支持 pause/volume/flush，用于 CI 与 headless 测试。
- `AvSync`：接收音频/视频 `Output` 并通过 `Clock` 接口计算呈现时机与 drift；在音频主导模式下以音频时钟为基准，视频按 pts 延迟或追赶。
- 平台真实音频 sink 以 stub 形式存在，避免在未链接平台 SDK 时虚假声明输出可用。

## 2. 交付物

- `crates/cheetah-media-native-audio/Cargo.toml`
- `crates/cheetah-media-native-audio/src/lib.rs`
- `crates/cheetah-media-native-audio/src/capability.rs`
- `crates/cheetah-media-native-audio/src/probe.rs`
- `crates/cheetah-media-native-audio/src/registry.rs`
- `crates/cheetah-media-native-audio/src/sink.rs`
- `crates/cheetah-media-native-audio/src/sync.rs`
- `dev-docs/004/60_native_audio_sink.md` 与 baseline 状态更新

## 3. 接口草案

```rust
use cheetah_media_abi::{AudioSink, Output};

pub enum PlatformAudioSink { Alsa, PulseAudio, CoreAudio, Wasapi, Null }

pub struct AudioSinkCapability { ... }

pub struct NullAudioSink { ... }
impl AudioSink for NullAudioSink { ... }

pub struct AvSync<C: Clock> {
    audio_latency_ms: i64,
    video_presented_ms: i64,
    clock: C,
}
impl<C: Clock> AvSync<C> {
    pub fn submit_audio(&mut self, output: &Output) -> SyncAction;
    pub fn submit_video(&mut self, output: &Output) -> SyncAction;
}
```

## 4. 验证

```bash
cargo test -p cheetah-media-native-audio
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [~] crate 创建与 audio sink 模型
- [~] 平台 audio sink 探测 stub 与注册表选择
- [~] `NullAudioSink` 与 `AvSync`
- [~] Rust 全矩阵验证通过
- [ ] CI / Devin Review 通过并合并
