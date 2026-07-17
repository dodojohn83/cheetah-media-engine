# 33. 性能、复制 Benchmark 与长稳

## PERF-001：冻结测试环境和方法

- [x] 记录 OS/浏览器/GPU driver：Playwright capability snapshot 在每次 `tests/browser/tests/capability-snapshot.spec.ts` 运行时生成 JSON，包含 `browser`、`userAgent`、`platform`、`hardwareConcurrency`、`deviceMemory` 和 GPU vendor/renderer。
- [ ] 固定硬件平台（Windows 11 i5-12400/UHD 730/16GB、macOS M1/8GB）测试和时钟校准。  
  注：VM 环境无法提供指定硬件；该项需在实际目标设备执行并补充环境 JSON。
- [ ] 流固定 hash、协议/codec、分辨率、帧率、GOP、bitrate。  
  注：真实流与校准源待 fixture 和 backend 完全打通后补充。
- [x] 基准输出 median/p95/p99 与原始数据：Criterion benchmark 生成统计报告和 `target/criterion` JSON/HTML。
- [ ] 禁止跨 stream/缓存/硬件/网络做结论：已完成代码层的计数与 benchmark，跨环境结论需按固定环境复测。

## PERF-002：首帧与实时延迟

- [ ] 首帧 p95 ≤800ms、FLV/fMP4 glass-to-glass p95 ≤600ms、LL-HLS p95 ≤1.5s。  
  注：延迟测量依赖真实网络/解码/渲染链路，当前引擎提供了 `LatencyController` 与 `Metrics` 钩子，可在链路完整后采样。
- [ ] A/V 偏差 p95 ≤50ms，丢帧率 <0.5%。
- [ ] 故障后恢复首帧和重新进入目标延迟的时间。

## PERF-003：复制、分配和内存门禁

- [x] instrumentation 按边界输出 copy count/bytes/reason、allocation、pool hit/miss 和 peak in-flight。  
  证据：
  - `cheetah-media-types::buffer::CopyBudget` 现在按 `CopyReason` 同时记录 `bytes` 和 `count`（`CopyCounter`）。
  - `SimpleBufferPool` 的 `PoolStats` 增加 `hits`/`misses` 计数，并在 `acquire` 时按是否命中 free list 累加。
  - `cheetah-media-engine::metrics::Metrics` 聚合 copy、allocation、pool hit/miss、in-flight peak 和 latency drop 毫秒，提供 `snapshot()` 和 `EngineEvent::Metrics` 输出；`EngineCommand::GetMetrics` 可主动拉取。
- [ ] 检查 transport→WASM、parser 拼接、decoder 输入、frame upload、audio 和 recorder。  
  注：各边界已可通过 `Metrics::record_copy(reason, bytes)` 打点，但 transport/decoder/frame upload 的真实热路径尚未接入，待 backend 全链路。
- [x] 新增热路径复制须有设计批准和基准；无解释回归阻断 PR。  
  证据：`CopyBudget` 在测试中检查 `total_limit`；CI `cargo clippy -D warnings` 与 `cargo test` 阻塞回归。
- [ ] JS heap、WASM pages、GPU estimate、decoder/frame count 和资源 ledger 同轴采样。  
  注：Rust 侧 `ResourceLedger` 与 `Metrics` 已就位，Web 侧 ledger 与 memory snapshot 待 SDK 事件接入。

## PERF-004：单窗、软解和多画面

- [ ] H.265 1080p25 软解 Threads+SIMD/SIMD/baseline 能力边界。
- [ ] 硬解 9×1080p25 H.265 或 16×720p15 密度。
- [ ] 主子码流/全屏切换无持续黑屏或资源震荡。
- [ ] 性能不足产生可解释降级。  
  注：以上依赖真实 decoder/codec pack/渲染链路；当前不具备运行条件。

## PERF-005：24 小时 soak

- [ ] 24 小时周期覆盖重连、后台恢复、码流切换、截图、录制和 backend fault。  
  注：soak 无法在单次 Devin 会话完成；已提供 `Metrics` 和资源 ledger 用于长稳采样与收尾对比。
- [ ] 内存增长 ≤5%，A/V/时间漂移 ≤100ms，无未释放资源。
- [ ] 每分钟时序、首尾 heap/resource snapshot、错误/恢复/drop 汇总。
- [ ] 失败保留原始数据与最小复现；修复后完整重跑。

## 本地证据

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo bench -p cheetah-media-types --features std
cargo deny check
source ~/.nvm/nvm.sh && nvm use && corepack pnpm -r typecheck && corepack pnpm -r test && corepack pnpm -r build
```

`cargo bench -p cheetah-media-types` 输出 `copy_budget_record`、`copy_budget_check`、`buffer_pool_acquire_release`、`buffer_pool_hit_rate` 的 median/p95/p99 与 `target/criterion` 报告。
