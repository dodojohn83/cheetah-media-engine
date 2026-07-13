# 07. Packet/Frame 所有权与复制预算

## MEM-001：定义 BufferRef 和池化契约

**目标**：编码 payload 在 transport→parser→decoder 输入间共享，不因跨层转发复制。

- [ ] native 使用不可变引用计数 slice；切片只调整 offset/length，不复制底层数据。
- [ ] WASM 使用线性内存 region + generation descriptor；JS 不持有可能因 memory growth 失效的永久 TypedArray。
- [ ] 明确 owned、borrowed、external-frame 三类生命周期以及跨线程 Send/Sync 规则。
- [ ] buffer pool 有总字节、对象数、单对象和等待时间上限；耗尽返回 ResourceLimit 或背压。
- [ ] debug 构建检测 double release、use-after-release、generation mismatch 和泄漏。

## MEM-002：冻结逐阶段复制预算

| 边界 | 目标 |
| --- | --- |
| Fetch/WS → WASM | 非共享网络 API 允许一次必要复制，并计量 |
| parser 分片/组帧 | 引用切片；跨 chunk 拼接仅在必要时一次 |
| demux → WebCodecs | 允许构造 decoder 输入的一次边界复制或转移，记录字节 |
| demux → MSE | 优先转移完整 segment，禁止逐 sample 重拷贝 |
| WASM decoder → renderer | 共享帧面或一次 upload；禁止 CPU 中间格式链式复制 |

- [ ] 每个不可避免复制点有命名 counter、原因枚举和 benchmark。
- [ ] 超预算在性能 CI 失败，不能用总吞吐量掩盖复制回归。

## MEM-003：背压和释放顺序

- [ ] 所有 pipeline stage 声明最大 in-flight 数、high/low watermark 和 drop policy。
- [ ] live 模式优先丢弃过期非关键视频帧；不得任意丢音频或破坏 decoder reference chain。
- [ ] stop/destroy 顺序固定为停止输入→取消任务→flush/drop→释放 surface/audio→回收 pool。
- [ ] reset 递增 epoch，迟到 callback 只能被丢弃，不能污染新会话。

## MEM-004：验证

- [ ] 单元测试引用计数、切片、池耗尽、epoch 和异常释放。
- [ ] Miri/ASan 覆盖 native；浏览器测试覆盖 memory growth 和 Worker 终止。
- [ ] benchmark 输出每秒复制次数、字节、池峰值、分配次数和 GC pause。
- [ ] 连续创建/销毁 1,000 次无 descriptor、VideoFrame、AudioData 或 GPU resource 泄漏。

