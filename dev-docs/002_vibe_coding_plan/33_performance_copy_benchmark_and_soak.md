# 33. 性能、复制 Benchmark 与长稳

## PERF-001：冻结测试环境和方法

- [ ] Windows 11：i5-12400、UHD 730、16GB；macOS：Apple M1、8GB，记录 OS/浏览器/GPU driver。
- [ ] 流使用固定 hash、协议/codec、分辨率、帧率、GOP、bitrate；服务端和客户端时钟校准。
- [ ] 每场景预热后至少运行规定样本数，报告 median/p95/p99、置信区间和原始 JSON。
- [ ] 禁止在不同 stream、缓存、硬件加速或网络条件之间做性能结论。

## PERF-002：首帧与实时延迟

- [ ] 首帧从 `load` 接受时刻到首个正确呈现帧，p95 ≤800ms。
- [ ] FLV/fMP4 稳态 glass-to-glass 或可校准源时间延迟 p95 ≤600ms。
- [ ] LL-HLS p95 ≤1.5s；报告 playlist/下载/demux/decode/render 分解。
- [ ] A/V 偏差 p95 ≤50ms，稳态丢帧率 <0.5%。
- [ ] 网络/backend 故障后报告恢复首帧和重新进入目标延迟的时间。

## PERF-003：复制、分配和内存门禁

- [ ] instrumentation 按边界输出 copy count/bytes/reason、allocation、pool hit/miss 和 peak in-flight。
- [ ] 检查 transport→WASM、parser 拼接、decoder 输入、frame upload、audio 和 recorder。
- [ ] 任一新增热路径复制须有设计批准和基准；无解释回归直接阻断 PR。
- [ ] JS heap、WASM pages、GPU estimate、decoder/frame count 和资源 ledger 同轴采样。

## PERF-004：单窗、软解和多画面

- [ ] H.265 1080p25 分别测试 Threads+SIMD、SIMD、baseline，报告实时能力边界。
- [ ] 硬解验证 9×1080p25 H.265 或 16×720p15；报告每 cell backend/variant/drop。
- [ ] 测试选中/全屏触发主子码流和预算重分配，确保无持续黑屏或资源震荡。
- [ ] 性能不足必须产生可解释降级，禁止通过隐藏帧/缩短测试伪造通过。

## PERF-005：24 小时 soak

- [ ] 包含网络重连、后台恢复、码流切换、截图、录制和 backend fault 周期。
- [ ] 24 小时后内存增长 ≤5%，A/V/时间漂移 ≤100ms，无未释放资源和重启风暴。
- [ ] 报告每分钟时序、首尾 heap/resource snapshot、错误/恢复/drop 汇总和环境。
- [ ] 失败保留原始数据与最小复现；修复后完整重跑，不以短测替代。

