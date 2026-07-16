# WP-53: C ABI bindings crate 骨架

## 1. 目标

在当前 Web bindings 之外建立原生 C ABI 层，使 Qt / Android / iOS 等宿主可以通过稳定的 C 接口创建和控制播放器实例。本工作包先交付 crate 骨架、 opaque player handle、创建/释放生命周期和 cbindgen 头文件生成机制，不实现完整播放控制面。

## 2. 交付物

- `crates/cheetah-media-c-bindings/Cargo.toml`：
  - `crate-type = ["cdylib", "staticlib", "rlib"]`。
  - `license.workspace = true`；`edition.workspace = true`；`rust-version.workspace = true`。
  - 依赖 `cheetah-media-abi` 和 `cheetah-media-engine`（std）。
  - `[lints.rust]` 设置 `unsafe_code = "deny"`（C ABI 边界需要 `unsafe`，但禁止其它位置）。
- `crates/cheetah-media-c-bindings/src/lib.rs`：
  - 公开 `CheetahPlayer` opaque 类型（Rust struct 对 C 隐藏，以 `*mut CheetahPlayer` 暴露）。
  - `cheetah_player_create()` / `cheetah_player_destroy()`。
  - `cheetah_player_version()` 返回引擎版本字符串（生命周期归调用方或全局静态，文档说明）。
  - 稳定的 `CheetahResult` 错误码（`Ok = 0`, `NullPtr`, `InvalidState`, `InvalidData`, `NotSupported` 等）。
  - 所有字符串使用 `*const c_char` / `usize` length，显式指定 UTF-8；所有输出字符串指向调用方提供 buffer，避免跨语言所有权争议。
- `crates/cheetah-media-c-bindings/build.rs`（可选）或 `scripts/generate-c-header.sh`：
  - 使用 `cbindgen` 生成 `include/cheetah_media.h`。
  - CI 中校验生成文件与源码一致，防止 header 漂移。
- `crates/cheetah-media-c-bindings/README.md`：说明职责、允许依赖、禁止依赖、feature、公共入口。
- `crates/cheetah-media-c-bindings/tests/`：
  - Rust `#[test]` 验证 FFI create/destroy round-trip、null 输入返回错误、double-destroy 安全、version 字符串非空。

## 3. 非交付物

- 完整 load/play/pause/stop 控制面（WP-54）。
- 视频 surface / OpenGL / platform decoder（WP-55~60）。
- transport / tokio runtime（WP-57）。

## 4. 接口草案

```c
/* include/cheetah_media.h (generated) */
typedef enum CheetahResult {
  CheetahResult_Ok = 0,
  CheetahResult_NullPtr = 1,
  CheetahResult_InvalidState = 2,
  CheetahResult_InvalidData = 3,
  CheetahResult_NotSupported = 4,
} CheetahResult;

typedef struct CheetahPlayer CheetahPlayer;

CheetahResult cheetah_player_create(CheetahPlayer **player);
CheetahResult cheetah_player_destroy(CheetahPlayer *player);
const char *cheetah_player_version(void);
```

## 5. 完成定义

- `cargo fmt --all --check` 通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- `cargo test --workspace --all-features` 通过。
- `cargo build --workspace --all-features` 生成 `.a`/`.so` 或 `.dylib` 产物。
- `cbindgen` 能重新生成 `include/cheetah_media.h` 且与提交文件一致。
- 无 `todo!()`/`unimplemented!()`；对外部输入无 `unwrap()`/`expect()`。
- 测试覆盖创建/释放、null 处理、double-destroy、version。
