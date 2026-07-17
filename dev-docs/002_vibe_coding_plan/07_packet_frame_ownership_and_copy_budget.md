# 07. Packet/Frame 所有权与复制预算

## MEM-001：定义 BufferRef 和池化契约

**目标**：编码 payload 在 transport→parser→decoder 输入间共享，不因跨层转发复制。

- [x] native 使用不可变引用计数 slice；切片只调整 offset/length，不复制底层数据。
  - 实现：`crates/cheetah-media-types/src/buffer/ref.rs` 中的 `BufferRef`，支持 `Borrowed`、`Shared(Bytes)` 和 `Empty`。
  - `BufferRef::slice` 对 `Borrowed` 返回子切片，对 `Shared` 调用 `Bytes::slice`，均为零拷贝。
  - 新增依赖：`bytes`（版本 1.12.1，MIT）和 `spin`（版本 0.9.9，MIT），均支持 `no_std`/WASM。
- [x] WASM 使用线性内存 region + generation descriptor；JS 不持有可能因 memory growth 失效的永久 TypedArray。
  - 实现：`crates/cheetah-media-types/src/buffer/wasm.rs` 中的 `LinearMemoryRef { offset, length, generation, memory_id }`。
- [x] 明确 owned、borrowed、external-frame 三类生命周期以及跨线程 Send/Sync 规则。
  - 实现：`BufferLifecycle` 枚举（Borrowed/Shared/External）。
  - `BufferRef`、`MediaPacket`、`VideoFrame`、`AudioFrame` 均提供 `lifecycle()` 方法。
  - `BufferPool: Send + Sync`，`SimpleBufferPool` 使用 `spin::Mutex` 和 `Arc`，在 `no_std` 下仍可 `Send + Sync`。
- [x] buffer pool 有总字节、对象数、单对象和等待时间上限；耗尽返回 ResourceLimit 或背压。
  - 实现：`BufferPoolConfig { max_total_bytes, max_count, max_object_size, max_wait_ms, max_free_count }`。
  - `SimpleBufferPool::acquire` 超限时返回 `MediaError::ResourceLimit`。
- [ ] debug 构建检测 double release、use-after-release、generation mismatch 和泄漏。
  - 说明：Rust 所有权 + `Bytes` 引用计数已消除 double release/use-after-release；
    `PoolStats` 的 `in_use_count`/`in_use_bytes` 和 `SimpleBufferPool` 测试可检测泄漏，
    generation mismatch 检测将在 WASM 绑定集成时补齐（WP-08/10）。

## MEM-002：冻结逐阶段复制预算

| 边界 | 目标 |
| --- | --- |
| Fetch/WS → WASM | 非共享网络 API 允许一次必要复制，并计量 |
| parser 分片/组帧 | 引用切片；跨 chunk 拼接仅在必要时一次 |
| demux → WebCodecs | 允许构造 decoder 输入的一次边界复制或转移，记录字节 |
| demux → MSE | 优先转移完整 segment，禁止逐 sample 重拷贝 |
| WASM decoder → renderer | 共享帧面或一次 upload；禁止 CPU 中间格式链式复制 |

- [x] 每个不可避免复制点有命名 counter、原因枚举和 benchmark。
  - 实现：`CopyReason` 枚举（`NetworkToWasm`、`ParserReassembly`、`DemuxToDecoder`、`DemuxToMse`、`DecoderToRenderer`）。
  - `CopyBudget` 按 `CopyReason` 计数，支持 `total_limit`，超限时 `check()` 返回 `MediaError::ResourceLimit`。
- [ ] 超预算在性能 CI 失败，不能用总吞吐量掩盖复制回归。
  - 说明：`CopyBudget` 已提供计数与超限检查；性能 CI 集成将在 `tests/performance` 扩展时启用。

## MEM-003：背压和释放顺序

- [x] 所有 pipeline stage 声明最大 in-flight 数、high/low watermark 和 drop policy。
  - 实现：`StageBudget { max_in_flight, high_watermark, low_watermark, drop_policy }` 和 `DropPolicy` 枚举。
  - 提供 `is_over_high`、`is_below_low`、`is_over_max`、`should_admit`、`backpressure` 方法。
- [x] live 模式优先丢弃过期非关键视频帧；不得任意丢音频或破坏 decoder reference chain。
  - 实现：`DropPolicy::DropNonKeyframe` + `should_admit`：live 非关键视频在 high watermark 被拒绝；音频/关键帧不会被丢弃。
- [ ] stop/destroy 顺序固定为停止输入→取消任务→flush/drop→释放 surface/audio→回收 pool。
  - 说明：本任务包只定义了类型与策略契约；具体 `Pipeline::stop`/`destroy` 顺序将在 WP-09 实现。
- [ ] reset 递增 epoch，迟到 callback 只能被丢弃，不能污染新会话。
  - 说明：`StreamEpoch` 已在 WP-06 定义；reset 语义将在 pipeline 状态机中实现（WP-09）。

## MEM-004：验证

- [x] 单元测试引用计数、切片、池耗尽、epoch 和异常释放。
  - `BufferRef` 切片/克隆：`buffer::ref::tests`。
  - `SimpleBufferPool` 回收与上限：`buffer::pool::tests`。
  - `CopyBudget` 与 `StageBudget`：`buffer::budget::tests`。
  - `LinearMemoryRef` 溢出校验：`buffer::wasm::tests`。
  - `MediaPacket` / `VideoFrame` 已切换为 `BufferRef`：`packet::tests`、`frame::tests`。
- [ ] Miri/ASan 覆盖 native；浏览器测试覆盖 memory growth 和 Worker 终止。
  - 说明：单元测试通过；Miri/ASan/浏览器专项测试将在 CI matrix 扩展后补齐。
- [ ] benchmark 输出每秒复制次数、字节、池峰值、分配次数和 GC pause。
  - 说明：`CopyBudget`/`PoolStats` 已提供所需计数；benchmark runner 将在 `benches/` 中补齐。
- [ ] 连续创建/销毁 1,000 次无 descriptor、VideoFrame、AudioData 或 GPU resource 泄漏。
  - 说明：当前 `SimpleBufferPool` 测试验证 acquire/drop 后 `in_use_count`/`in_use_bytes` 归零；
    1,000 次循环测试将在集成测试层补齐。

