# 08. 共享内存 ABI 与 WASM Bindings

## ABI-001：冻结 ABI 版本和 descriptor

**crate**：`cheetah-media-abi`。ABI 使用固定宽度整数、`#[repr(C)]`、little-endian manifest，不暴露 Rust enum/Vec/String 布局。

- [x] `AbiVersion` 使用 major/minor；major 不兼容，minor 只允许向后兼容追加。
  - 实现：`crates/cheetah-media-abi/src/version.rs`：`AbiVersion { major, minor }`，`CURRENT`、`supports(caller)`、`to_u32()`。
- [x] `MemoryDescriptor` 固定 region、offset、length、capacity、generation、flags，不传裸指针给长期 JS 状态。
  - 实现：`crates/cheetah-media-abi/src/descriptor.rs`：`MemoryDescriptor` 为 `#[repr(C)]` 的 POD，JS 通过 `offset` + `length` 构造 view。
- [x] Packet/frame descriptor 通过索引引用 track/plane/side-data 表，并携带 epoch。
  - 实现：`PacketDescriptor` 和 `FrameDescriptor` 包含 `track_index`、`payload`、`planes`（4 个 plane 槽位）、`side_data` 和 `epoch`。
- [x] 提供 layout size/alignment/offset 静态断言和由 Rust 生成的 TypeScript 常量。
  - 实现：`descriptor.rs` 中使用 `const _: () = assert!(...)` 对 `AbiVersion`、`MemoryDescriptor`、`PacketDescriptor`、`FrameDescriptor` 做编译期布局断言；`crates/cheetah-media-abi/src/bin/gen_abi_ts.rs` 生成 `packages/web/src/abi-constants.ts`；`packages/web/src/abi.ts` 导出常量与 TS 接口。
- [ ] manifest 包含 core version、ABI、features、SIMD/threads、codec 和 source hash。
  - 状态：版本与 descriptor 已冻结，manifest 结构将在 WP-16/WP-19 随 capability 探测一起补齐；当前 `engine_version()` 提供 core version 字符串。

## ABI-002：分配、提交和释放协议

- [x] 控制面导出 create/configure/push/poll/release/stop/destroy；payload 通过 descriptor 传递。
  - 实现：`crates/cheetah-media-web-bindings/src/lib.rs`：`WebEngine` 提供 `new()`（create）、`configure()`、`push_packet()`、`poll_output()`、`release_region()`、`stop()`、`destroy()`；`request_write_region()` 与 `commit_region()` 覆盖请求-写入-提交流程。
- [x] JS 请求可写 region，写入后 commit 实际长度；失败或取消必须 release。
  - 实现：`request_write_region(size)` 返回 `MemoryDescriptor`（含 `offset`、`slot`、`generation`），JS 写入后调用 `commit_region(slot, generation, len)`，不再需要时调用 `release_region(slot, generation)`。
- [ ] Rust 输出 descriptor 后，JS 使用完显式 release；批量 poll/release 降低跨边界调用。
  - 状态：单个 release 已实现；`poll_output` 尚未返回可释放的输出 descriptor（解码器未接入），将在 WP-20/WP-23 后启用。
- [x] 所有 handle 使用 slot + generation，拒绝 stale、越界、重复释放和跨实例 handle。
  - 实现：`cheetah-media-abi/src/handle.rs` 定义 `Handle { instance_id, slot, generation }`；`MemoryArena` 在 `arena.rs` 中通过 generation 与 `instance_id` 拒绝 `StaleHandle`、`OutOfBounds`、`DoubleFree`、`WrongInstance`。
- [ ] 内存增长后重新获取 view；禁止缓存旧 `wasmMemory.buffer`。
  - 状态：通过 descriptor + 每次按 `offset` 构造 view 的协议已确立；JS 侧完整实现随 runtime 一起落地（WP-17）。

## ABI-003：隔离与非隔离内存模式

- [ ] cross-origin isolated 模式启用 SharedArrayBuffer、threads 和原子 ring。
- [ ] 非隔离模式使用单 Worker、SIMD 或 baseline，接口和事件语义保持一致。
- [ ] capability 明确报告 shared memory unavailable 的原因，不将其视为播放失败。
- [ ] 两种模式使用同一 ABI contract suite 和 fixture。

> 说明：ABI 侧已预留 feature flags（`AbiFeatureFlags` TS 枚举），隔离/非隔离 runtime 策略在 WP-17 实现。

## ABI-004：安全与兼容测试

- [ ] fuzz 所有导出函数的 handle、offset、length、alignment 和调用顺序。
- [ ] 测试 JS 终止、Worker crash、memory growth、codec pack 版本不匹配和 partial batch。
- [ ] ABI golden 在 CI 检测意外 layout/export 变化；变更必须附兼容说明。
- [x] `unsafe` 仅存在于边界模块，每个块写可检查的 Safety 前置条件。
  - 实现：`cheetah-media-abi` 与 `cheetah-media-web-bindings` 当前实现无 `unsafe` 块；`cheetah-media-abi` 的 `unsafe_code` lint 从 `forbid` 调整为 `deny` 以符合 WP-04 规则，后续若边界模块需要 `unsafe` 将写 Safety 注释。

---

状态: In Progress
仓库/提交: cheetah-media-engine@`wp/08-shared-memory-abi-and-wasm-bindings`
验证命令:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
source ~/.nvm/nvm.sh && nvm use
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
```
结果: 全部 Rust/JS 检查通过；`cheetah-media-abi` 新增 7 个测试，`cheetah-media-web-bindings` 新增 1 个测试；`wasm32` release/no-default 构建通过；Playwright smoke 通过。
制品或报告:
- `crates/cheetah-media-abi/src/{version,error,descriptor,handle,arena}.rs`
- `crates/cheetah-media-abi/src/bin/gen_abi_ts.rs`
- `crates/cheetah-media-web-bindings/src/lib.rs`（`WebEngine`、`MemoryDescriptor`）
- `packages/web/src/abi-constants.ts`（Rust 生成）
- `packages/web/src/abi.ts`
已知限制: manifest、隔离/非隔离 runtime、decoder `push`/`poll` 输出和 golden ABI CI 将在后续 WP-16/17/19/20 补齐；当前 `push_packet`/`poll_output` 返回 `AbiError::NotSupported` 以明确标识未接入解码器。
复核人/日期: Devin / 2026-07-13
