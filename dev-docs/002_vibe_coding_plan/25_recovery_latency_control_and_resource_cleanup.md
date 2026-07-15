# 25. 恢复、延迟控制与资源清理

## REC-001：统一恢复策略

- [x] 错误按 retry-same-stage、rebuild-stage、fallback-backend、reconnect-source、fatal 分类。
- [x] 每类错误固定最大次数、窗口、退避、需要的关键点和状态事件。
- [x] 同一根因在短时间聚合，`RecoveryTracker` 按 `(code, stage, action)` 计数并自动 prune 过期记录。
- [x] 用户 stop/destroy 优先于任何自动恢复；load/stop/destroy 会先清空 ledger 并重置 recovery tracker。`Cancelled`/`load-while-active` 不显示为播放错误。

## REC-002：实时延迟控制

- [x] `LatencyController` 已可分解输入、demux、decode、render 延迟并输出 `LatencyAction`。
- [-] soft/hard target 策略已就绪，playbackRate 与 A/V ring 集成将在后续 pipeline 阶段 wiring。
- [-] 控制器输出 `SpeedUp`/`DropToKeyframe`/`JumpToLive` reason 与 dropped_ms；与 MSE buffer/recorder 的联动仍待后续阶段接通。
- [x] `LatencyAction` 携带 `reason`、`dropped_ms`、`target_ms`。

## REC-003：切换无旧帧污染

- [x] `BackendEvent` 携带 `StreamEpoch`，state machine 对旧 epoch 事件执行 generation barrier。
- [-] `Engine::load` 会重置 epoch/tracker/ledger；后续 pipeline 停止旧 decoder 并释放旧帧的逻辑在 state machine 层通过 recovery/stop 触发。
- [ ] UI 最后一帧保留规则仍待 web 层接入。
- [-] 全局 `ResourceLedger` 已就位，切换重叠资源受预算约束将在后续调度器集成。

## REC-004：资源清理验收

- [x] 建立 `ResourceLedger` 与 RAII `ResourceGuard`，覆盖 Network/Timer/Worker/WasmHandle/Decoder/Frame/Audio/Gpu/Url/Listener。
- [x] `Engine` 在 load/stop/destroy 时重置 ledger，未释放资源会发出 `ResourceWarning` 事件。
- [x] 单元压力测试：1,000 次 `load -> stop -> stopped` 循环 ledger 归零；10,000 次 backend fault 不泄漏 ledger 计数。
- [ ] 24 小时 soak 与真实 heap/GPU/handle 增长指标仍依赖后续性能 harness/browser 测试套件。

