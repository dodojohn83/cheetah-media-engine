# 21. MSE Backend

## MSE-001：MediaSource/SourceBuffer 生命周期

- [ ] 创建、sourceopen、addSourceBuffer、append、updateend、endOfStream、close 使用串行状态机。
- [ ] 所有 append/remove/changeType 操作进入有界队列，同一 SourceBuffer 不并发调用。
- [ ] stop/destroy 解除 listener、abort 更新、撤销 object URL，并忽略旧 epoch 事件。
- [ ] HTMLVideoElement 错误和 MediaSource 错误映射到稳定 backend code。

## MSE-002：append 与 buffer window

- [ ] 先 append 匹配 generation 的 init segment，再 append media segment。
- [ ] 使用实际 buffered ranges 维护 live window；只在 SourceBuffer 空闲时 remove。
- [ ] QuotaExceeded 先有界清理旧区间并重试一次，仍失败触发回退。
- [ ] timestampOffset/appendWindow 只在明确 discontinuity 下调整并留下诊断。

## MSE-003：低延迟控制

- [ ] live edge、currentTime、buffer ahead/behind 和 playbackRate 定期采样。
- [ ] 小偏差可有限提高 playbackRate；大偏差 seek 到安全 live point，并记录原因。
- [ ] 禁止无限累积 buffer 或频繁 seek 抖动；所有阈值配置化且有默认边界。
- [ ] 浏览器后台恢复重新评估 live edge，不播放长时间积压内容。

## MSE-004：兼容与故障测试

- [ ] 覆盖 fMP4 直入、FLV/TS remux、init change、timestamp discontinuity 和音视频组合。
- [ ] 注入 addSourceBuffer/append/Quota/HTMLMediaElement failure，验证切换到 WASM。
- [ ] 验证 Safari/Chrome/Edge 的 MIME/sequence 差异；Firefox 不支持组合必须明确排除。
- [ ] 指标包括 append duration、queue depth、buffer ranges、quota cleanup、seek 和 stall。

