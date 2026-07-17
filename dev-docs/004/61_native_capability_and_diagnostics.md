# WP-61: 原生能力协商、diagnostics 与生命周期 soak

## 1. 范围

在 `cheetah-media-engine` 中集成已完成的 native transport、decoder、renderer 与 audio sink，形成可运行的原生播放路径，并提供能力协商、diagnostics 与长生命周期 soak 框架：

- `NativePlayerBuilder`：从 `LoadRequest` 出发，协商 transport + decoder + renderer + audio sink 组合。
- `NativePlayer`：持有选定的组件，驱动数据从 transport → decoder → renderer/audio sink；维护 `PlayerState` 与基本 diagnostics。
- `CapabilityNegotiator`：把 decoder/renderer/audio registry 与后端 probe 统一为 `BackendPlan`。
- `Diagnostics`：聚合 metrics、frame drop 计数、backend 选择结果与生命周期事件。
- `LifecycleSoak`：记录 create/load/play/pause/stop/destroy 调用序列，检测泄漏与重复状态转换。
- 至少一个端到端 smoke：使用 `MemoryTransport` 或内置 `http://` 本地 fixture 验证 `NativePlayer` 能完成 load→play→stop→destroy 而不 panic/泄漏。

## 2. 交付物

- `crates/cheetah-media-engine/src/native/` 目录（`mod.rs`、`negotiator.rs`、`player.rs`、`diagnostics.rs`、`lifecycle.rs`）。
- `crates/cheetah-media-engine/src/native/mod.rs` 暴露公共类型。
- `crates/cheetah-media-engine/Cargo.toml` 增加 native 组件依赖与 `native` feature。
- `dev-docs/004/61_native_capability_and_diagnostics.md` 与 baseline 状态更新。

## 3. 接口草案

```rust
pub struct NativePlayerConfig {
    pub url: String,
    pub video: Option<VideoTarget>,
    pub audio: Option<AudioTarget>,
    pub autoplay: bool,
}

pub struct BackendPlan {
    pub transport: TransportKind,
    pub decoder: DecoderBackend,
    pub renderer: RendererBackend,
    pub audio: AudioBackend,
}

pub struct NativePlayer<D: Decoder, R: Renderer, A: AudioSink, T: Transport> {
    transport: T,
    decoder: D,
    renderer: R,
    audio: A,
    diagnostics: Diagnostics,
    lifecycle: LifecycleSoak,
    state: PlayerState,
}

impl NativePlayer<...> {
    pub fn from_plan(plan: BackendPlan, config: NativePlayerConfig) -> Result<Self, EngineError>;
    pub fn tick(&mut self) -> Vec<EngineEvent>;
    pub fn play(&mut self) -> Result<(), EngineError>;
    pub fn pause(&mut self) -> Result<(), EngineError>;
    pub fn stop(&mut self) -> Result<(), EngineError>;
    pub fn destroy(self);
}
```

## 4. 验证

```bash
cargo test -p cheetah-media-engine --features native
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [x] native 模块与能力协商
- [x] `NativePlayer` 生命周期与 diagnostics
- [x] Rust 全矩阵验证通过
- [x] CI / Devin Review 通过并合并
