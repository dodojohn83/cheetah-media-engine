# WP-59: 原生 renderer 与零拷贝 surface

## 1. 范围

建立 `cheetah-media-native-renderer` crate，为原生平台提供视频渲染后端选择、零拷贝 surface 模型与 CPU fallback：

- 统一 `PlatformRenderer` 与 `RendererCapability` 能力描述（API、像素格式、最大分辨率、zero-copy 支持）。
- `RendererProbe` trait 与平台探测实现：OpenGL / Vulkan / Metal / D3D11 / Software。
- `RendererRegistry`：合并探测结果，按格式/分辨率选择最佳渲染后端。
- `NativeRenderer`：实现 `cheetah_media_abi::Renderer`，内部维护后端列表并回退到 `CpuRenderer`。
- `Surface` / `SurfaceHandle`：描述解码帧在 native memory 中的布局，CPU backend 拥有 `Vec<u8>`；GPU backends 以 stub 形式存在。
- 平台 GPU renderer 实现以 stub 形式存在，避免在未链接平台 SDK 时虚假声明 GPU 支持。

## 2. 交付物

- `crates/cheetah-media-native-renderer/Cargo.toml`
- `crates/cheetah-media-native-renderer/src/lib.rs`
- `crates/cheetah-media-native-renderer/src/capability.rs`
- `crates/cheetah-media-native-renderer/src/probe.rs`
- `crates/cheetah-media-native-renderer/src/registry.rs`
- `crates/cheetah-media-native-renderer/src/surface.rs`
- `crates/cheetah-media-native-renderer/src/renderer.rs`
- `dev-docs/004/59_native_renderer.md` 与 baseline 状态更新

## 3. 接口草案

```rust
use cheetah_media_abi::{AbiError, Output, Renderer};

pub enum PlatformRenderer { OpenGl, Vulkan, Metal, D3D11, Software }

pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

pub trait SurfaceAccess {
    fn surface(&self) -> Option<&Surface>;
}

pub struct NativeRenderer { ... }
impl Renderer for NativeRenderer { ... }
impl SurfaceAccess for NativeRenderer { ... }
```

## 4. 验证

```bash
cargo test -p cheetah-media-native-renderer
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [x] crate 创建与 surface 模型
- [x] 平台 renderer 探测 stub 与注册表选择
- [x] `NativeRenderer` 回退与 `CpuRenderer`
- [x] Rust 全矩阵验证通过
- [x] CI / Devin Review 通过并合并
