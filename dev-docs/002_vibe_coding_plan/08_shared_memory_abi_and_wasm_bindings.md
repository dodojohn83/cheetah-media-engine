# 08. 共享内存 ABI 与 WASM Bindings

## ABI-001：冻结 ABI 版本和 descriptor

**crate**：`cheetah-media-abi`。ABI 使用固定宽度整数、`#[repr(C)]`、little-endian manifest，不暴露 Rust enum/Vec/String 布局。

- [ ] `AbiVersion` 使用 major/minor；major 不兼容，minor 只允许向后兼容追加。
- [ ] `MemoryDescriptor` 固定 region、offset、length、capacity、generation、flags，不传裸指针给长期 JS 状态。
- [ ] Packet/frame descriptor 通过索引引用 track/plane/side-data 表，并携带 epoch。
- [ ] 提供 layout size/alignment/offset 静态断言和由 Rust 生成的 TypeScript 常量。
- [ ] manifest 包含 core version、ABI、features、SIMD/threads、codec 和 source hash。

## ABI-002：分配、提交和释放协议

- [ ] 控制面导出 create/configure/push/poll/release/stop/destroy；payload 通过 descriptor 传递。
- [ ] JS 请求可写 region，写入后 commit 实际长度；失败或取消必须 release。
- [ ] Rust 输出 descriptor 后，JS 使用完显式 release；批量 poll/release 降低跨边界调用。
- [ ] 所有 handle 使用 slot + generation，拒绝 stale、越界、重复释放和跨实例 handle。
- [ ] 内存增长后重新获取 view；禁止缓存旧 `wasmMemory.buffer`。

## ABI-003：隔离与非隔离内存模式

- [ ] cross-origin isolated 模式启用 SharedArrayBuffer、threads 和原子 ring。
- [ ] 非隔离模式使用单 Worker、SIMD 或 baseline，接口和事件语义保持一致。
- [ ] capability 明确报告 shared memory unavailable 的原因，不将其视为播放失败。
- [ ] 两种模式使用同一 ABI contract suite 和 fixture。

## ABI-004：安全与兼容测试

- [ ] fuzz 所有导出函数的 handle、offset、length、alignment 和调用顺序。
- [ ] 测试 JS 终止、Worker crash、memory growth、codec pack 版本不匹配和 partial batch。
- [ ] ABI golden 在 CI 检测意外 layout/export 变化；变更必须附兼容说明。
- [ ] `unsafe` 仅存在于边界模块，每个块写可检查的 Safety 前置条件。

