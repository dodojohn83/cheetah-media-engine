# WP-54: C ABI 控制面

## 1. 范围

在 WP-53 的 `cheetah-media-c-bindings` crate 骨架基础上，增加原生宿主可调用的一组控制函数：

- 配置（`cheetah_player_configure`）
- 加载（`cheetah_player_load`）
- 播放/暂停/停止（`cheetah_player_play/pause/stop`）
- 异步事件回调注册（`cheetah_player_set_event_callback`）
- 事件结构 `CheetahEvent` 与回调类型 `CheetahEventCallback`

引擎仍然保持平台无关；C ABI 层负责把 `EngineCommand` 路由到状态机，并把 `EngineEvent` 同步转发给 C 回调。

## 2. 交付物

- `crates/cheetah-media-c-bindings/src/lib.rs`：
  - `CheetahEvent`（`#[repr(C)]`）与 `CheetahEventCallback` 类型。
  - `cheetah_player_set_event_callback`。
  - `cheetah_player_configure` / `load` / `play` / `pause` / `stop`。
  - 事件到 `CheetahEvent` 的映射与同步回调分发。
- `crates/cheetah-media-engine/src/state.rs`：
  - `LoadRequest.url` 改为 `String`，以便从 C 接收动态 URL。
- `crates/cheetah-media-c-bindings/include/cheetah_media.h`：重新生成。
- `crates/cheetah-media-c-bindings/cbindgen.toml`：更新 `include` 列表。
- 测试：
  - null/invalid URL/UTF-8 返回对应错误码。
  - 未加载时 `play` 返回 `InvalidState`。
  - 事件回调在 `load`/`play` 路径上被调用并收到 `state_changed`。

## 3. 接口草案

```c
/* include/cheetah_media.h (generated) */
typedef void (*CheetahEventCallback)(const CheetahPlayer *player,
                                     const CheetahEvent *event,
                                     void *userdata);

typedef struct {
  const char *event_type;  /* "state_changed", "error", "eof", ... */
  const char *track_id;    /* 可选，NULL 表示无 */
  const char *message;     /* 可读上下文，NULL 表示无 */
  uint32_t error_code;     /* error 事件的引擎错误码 */
} CheetahEvent;

int cheetah_player_set_event_callback(CheetahPlayer *player,
                                      CheetahEventCallback callback,
                                      void *userdata);
int cheetah_player_configure(CheetahPlayer *player, const char *config);
int cheetah_player_load(CheetahPlayer *player, const char *url, bool is_live);
int cheetah_player_play(CheetahPlayer *player);
int cheetah_player_pause(CheetahPlayer *player);
int cheetah_player_stop(CheetahPlayer *player);
```

说明：

- 回调在调用控制函数的线程上同步触发；`CheetahEvent` 及其字符串字段仅在回调存续期内有效。
- `cheetah_player_set_event_callback` 传入 `NULL` 回调可禁用事件投递。
- `cheetah_player_load` 要求 URL 包含 `://`，否则返回 `CHEETAH_RESULT_INVALID_DATA`。
- 若控制命令在当前引擎状态下非法，函数仍返回事件给回调，同时函数返回 `CHEETAH_RESULT_INVALID_STATE`。

## 4. 验证

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
cd crates/cheetah-media-c-bindings && cbindgen --config cbindgen.toml --crate cheetah-media-c-bindings --output /tmp/cheetah_media.h && diff -u include/cheetah_media.h /tmp/cheetah_media.h
```

## 5. 状态

- [x] `CheetahEvent` / `CheetahEventCallback` 类型与头文件生成
- [x] `cheetah_player_set_event_callback`
- [x] `cheetah_player_configure`
- [x] `cheetah_player_load` / `play` / `pause` / `stop`
- [x] 事件映射与同步回调分发
- [x] Rust FFI 测试
- [x] 完整验证矩阵通过
