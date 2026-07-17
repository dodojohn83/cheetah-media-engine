# WP-74: 双向引擎集成与端到端 soak

## 1. 目标

完成 `cheetah-media-engine` Phase 7 双向引擎的集成层，把资源预算、A/V 时间线和 diagnostics 接到 `BroadcastEngine` 上，并通过 soak 测试验证稳态行为。

- 统一资源预算：
  - `resource.rs` 新增 `ResourceLimits`（总量/按类上限）。
  - `BroadcastEngine` 可配置 `MediaLimits` 与 `ResourceLimits`。
  - `BroadcastPipeline::start` 前校验分辨率/资源占用，超限返回 `MediaError::ResourceLimit`。
- A/V 时间线：
  - `BroadcastEngine` 内嵌 `MediaClock`。
  - `BroadcastPipeline::tick` 在 `BroadcastPacketSummary` 中携带 `timestamp`、`stream_epoch`、`is_keyframe`、`is_audio`。
  - `BroadcastEngine::tick` 用发布包的 `MediaTime` 更新 `MediaClock`，暴露 jitter/discontinuity 统计。
- Diagnostics：
  - 新增 `BroadcastDiagnostics`（state + metrics snapshot + resources + clock stats）。
  - `BroadcastEngine::diagnostics()` 返回当前瞬态。
- 端到端 soak：
  - 用 `MockCaptureSource` + `MockEncoder` + `MockPublisher` 跑 100+ tick。
  - 验证无资源泄漏、metrics 增长、序列号递增、diagnostics 非空、停止后资源归零。

## 2. 交付

- `crates/cheetah-media-engine/src/resource.rs`：`ResourceLimits`。
- `crates/cheetah-media-engine/src/broadcast/pipeline.rs`：`BroadcastPacketSummary` 扩展，新增 `config()` 访问器。
- `crates/cheetah-media-engine/src/broadcast/engine.rs`：`BroadcastDiagnostics`、`MediaClock`、`MediaLimits`、`ResourceLimits`、soak 测试。
- `crates/cheetah-media-engine/src/broadcast/mod.rs`：重导出 `BroadcastDiagnostics`。
- `dev-docs/005/74_bidirectional_integration.md`、状态更新。

## 3. 验证命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
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

## 4. 状态

- [x] 创建 WP-74 计划文档
- [ ] 实现 `ResourceLimits` 与统一资源预算校验
- [ ] 集成 `MediaClock` 与 A/V 时间线元数据
- [ ] 实现 `BroadcastDiagnostics` 接口
- [ ] 添加端到端 soak 测试
- [ ] Rust/JS 验证矩阵通过
- [ ] CI / Devin Review 通过并合并
