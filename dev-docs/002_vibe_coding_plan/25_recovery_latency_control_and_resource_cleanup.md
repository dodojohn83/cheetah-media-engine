# 25. 恢复、延迟控制与资源清理

## REC-001：统一恢复策略

- [ ] 错误按 retry-same-stage、rebuild-stage、fallback-backend、reconnect-source、fatal 分类。
- [ ] 每类错误固定最大次数、窗口、退避、需要的关键点和状态事件。
- [ ] 同一根因在短时间聚合，防止日志、事件和重建风暴。
- [ ] 用户 stop/destroy 优先于任何自动恢复，Cancelled 不显示为播放错误。

## REC-002：实时延迟控制

- [ ] 用输入 live edge、demux latest DTS、decode/render clock 分解总延迟。
- [ ] soft target 通过调度/有限 playbackRate 修正；hard target 通过丢到安全关键点恢复。
- [ ] 追赶同时考虑音频 ring、decoder reference、MSE buffer 和 recorder，不产生 A/V 分裂。
- [ ] 输出 latency action reason、丢弃时长和重新稳定时间。

## REC-003：切换无旧帧污染

- [ ] backend/stream/config 切换创建 generation barrier。
- [ ] 停止旧输入和 decoder 后从新 config+随机访问点启动；迟到帧只 close/release。
- [ ] UI 可保留最后一帧但不得把它计为新流首帧；首帧事件携带 epoch/backend。
- [ ] 主子码流切换允许短暂重叠但受全局资源预算限制。

## REC-004：资源清理验收

- [ ] 建立资源 ledger：fetch/ws、timer、worker、WASM handle、decoder、frame、audio、GPU、URL/listener。
- [ ] 正常 stop、每阶段失败、fallback、页面隐藏和 destroy 后 ledger 归零。
- [ ] 1,000 次创建销毁与 10,000 次 backend fault 无线性 heap/GPU/handle 增长。
- [ ] 24 小时 soak 内存增长 ≤5%，漂移 ≤100ms；失败必须提供时间序列和 heap/resource diff。

