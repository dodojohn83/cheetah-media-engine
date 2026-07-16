# WP-53: C ABI bindings crate 骨架

## 1. 目标

在当前 Web bindings 之外建立原生 C ABI 层，使 Qt / Android / iOS 等宿主可以通过稳定的 C 接口创建和控制播放器实例。本工作包先交付 crate 骨架、 opaque player handle、创建/释放生命周期和 cbindgen 头文件生成机制，不实现完整播放控制面。

## 2. 交付物

- `crates/cheetah-media-c-bindings/Cargo.toml`：
  - `crate-type = ["cdylib", "staticlib", "rlib"]`。
  - `license.workspace = true`；`edition.workspace = true`；`rust-version.workspace = true`。
  - 依赖 `cheetah-media-engine`（std）。
  - `[lints.rust]` 设置 `unsafe_code = "deny"`；FFI 函数局部使用 `#[allow(unsafe_code)]` 并写 `SAFETY` 注释。
- `crates/cheetah-media-c-bindings/src/lib.rs`：
  - 公开 `CheetahPlayer` opaque 类型（Rust struct 对 C 隐藏，以 `*mut CheetahPlayer` 暴露）。
  - `cheetah_player_create()` / `cheetah_player_destroy()`（destroy 接收 `*mut *mut` 并将句柄置空）。
  - `cheetah_player_version()` / `cheetah_player_version_length()`。
  - `cheetah_player_state()` 返回当前状态字符串。
  - 稳定的 `CheetahResult` 错误码（`Ok = 0`, `NullPtr`, `InvalidState`, `InvalidData`, `NotSupported`, `InternalError`）。
  - 所有字符串使用 `*const c_char` 指向静态/引擎字符串；跨语言所有权不转移。
- `crates/cheetah-media-c-bindings/cbindgen.toml` + `include/cheetah_media.h`：
  - 使用 `cbindgen` 生成 C 头文件；CI 中校验生成文件与源码一致，防止 header 漂移。
- `crates/cheetah-media-c-bindings/README.md`：说明职责、允许依赖、禁止依赖、feature、公共入口。
- `src/lib.rs` 内 `#[cfg(test)]`：
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
  CheetahResult_InternalError = 5,
} CheetahResult;

typedef struct CheetahPlayer CheetahPlayer;

int cheetah_player_create(CheetahPlayer **player);
int cheetah_player_destroy(CheetahPlayer **player);
const char *cheetah_player_version(void);
uintptr_t cheetah_player_version_length(void);
const char *cheetah_player_state(const CheetahPlayer *player);
```

说明：
- `cheetah_player_destroy` 接收 `CheetahPlayer**` 并在内部置为 `NULL`，使同址重复调用安全。
- `cheetah_player_state` 返回当前播放器状态字符串（`idle`/`loading`/`playing` 等），生命周期随下次状态变更调用失效。

## 5. 完成定义

- `cargo fmt --all --check` 通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- `cargo test --workspace --all-features` 通过。
- `cargo build --workspace --all-features` 生成 `.a`/`.so` 或 `.dylib` 产物。
- `cbindgen` 能重新生成 `include/cheetah_media.h` 且与提交文件一致。
- 无 `todo!()`/`unimplemented!()`；对外部输入无 `unwrap()`/`expect()`。
- 测试覆盖创建/释放、null 处理、double-destroy、version。
