# WP-71: 采集源与权限模型

## 1. 目标

在 `cheetah-media-engine` 的 `broadcast` 模块中补充采集源种类、平台权限模型，并把权限检查接入 `BroadcastEngine` 生命周期。

- 定义 `CaptureSourceKind`（Camera / Microphone / Screen / Application / Custom）。
- 定义 `PermissionState`（Unknown / Prompt / Granted / Denied / Restricted）和 `PermissionModel` trait。
- 提供 `HostPermissionModel`（主机无采集设备，返回 `Denied`），以及测试用的 `AlwaysGrantPermissionModel` / `AlwaysDenyPermissionModel`。
- 提供 `CameraCaptureSource`、`MicrophoneCaptureSource`、`ScreenCaptureSource` 主机占位实现；未链接平台 SDK 时返回 `MediaError::Unsupported`。
- 在 `CaptureSource` trait 增加 `required_permission()` 默认返回 `None`。
- `BroadcastPipeline` 暴露 `required_permission()` 委托给 source。
- `BroadcastEngine` 增加 `PermissionModel` 注入点；`BroadcastCommand::Start` 在启动前先查询/请求 source 所需权限，未 `Granted` 时返回 `BroadcastError::PermissionDenied`。
- 新增 `BroadcastCommand::RequestPermission(CaptureSourceKind)` 和 `BroadcastEvent::PermissionChanged`。

## 2. 交付

- `crates/cheetah-media-engine/src/broadcast/permission.rs`：kind、state、trait、host/test 实现。
- `crates/cheetah-media-engine/src/broadcast/capture_sources.rs`：Camera / Microphone / Screen 占位，`MockCaptureSource` 用于测试。
- `crates/cheetah-media-engine/src/broadcast/source.rs`：扩展 `required_permission` 方法。
- `crates/cheetah-media-engine/src/broadcast/pipeline.rs`：暴露 `required_permission()`。
- `crates/cheetah-media-engine/src/broadcast/engine.rs`：权限检查与事件。
- `crates/cheetah-media-engine/src/broadcast/mod.rs`：重导出。
- `dev-docs/005_mobile_and_bidirectional.md`：状态更新。

## 3. 接口草图

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSourceKind {
    Camera,
    Microphone,
    Screen,
    Application,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionState {
    #[default]
    Unknown,
    Prompt,
    Granted,
    Denied,
    Restricted,
}

pub trait PermissionModel: Send {
    fn query(&self, kind: CaptureSourceKind) -> PermissionState;
    fn request(&mut self, kind: CaptureSourceKind) -> PermissionState;
}

pub trait CaptureSource: Send {
    fn start(&mut self) -> Result<(), MediaError>;
    fn stop(&mut self) -> Result<(), MediaError>;
    fn poll(&mut self) -> Result<Option<MediaFrame<'static>>, MediaError>;
    fn kind(&self) -> &'static str;
    fn required_permission(&self) -> Option<CaptureSourceKind> { None }
}
```

`BroadcastEngine` 新增：

```rust
pub fn with_permission_model(self, model: Box<dyn PermissionModel>) -> Self
pub enum BroadcastCommand {
    ...
    RequestPermission(CaptureSourceKind),
}
pub enum BroadcastError {
    ...
    PermissionDenied { kind: CaptureSourceKind },
}
pub enum BroadcastEvent {
    ...
    PermissionChanged { kind: CaptureSourceKind, state: PermissionState },
}
```

## 4. 验证命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p cheetah-media-engine --features bidirectional
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo deny check
( cd crates/cheetah-media-c-bindings && cbindgen --config cbindgen.toml --crate cheetah-media-c-bindings --output /tmp/cheetah_media.h && diff -u include/cheetah_media.h /tmp/cheetah_media.h )
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
```

## 5. 状态

- [x] 创建 WP-71 计划文档
- [x] 实现 `CaptureSourceKind` / `PermissionModel` / 权限状态
- [x] 实现 Camera / Microphone / Screen 主机占位 source
- [x] 接入 `BroadcastEngine` 启动前权限检查
- [x] Rust/JS 验证矩阵通过
- [x] CI / Devin Review 通过并合并（PR #72）
