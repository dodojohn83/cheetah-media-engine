# 23. 音频 Pipeline、AudioWorklet 与同步

## AUD-001：统一 PCM 格式和转换

- [ ] decoder 输出转换到明确的 planar/interleaved F32/S16 内部格式，保留原 timestamp。
- [ ] 重采样器处理实际输入/AudioContext sample rate，维护延迟和 fractional state。
- [ ] channel mapping 使用显式 layout；未知 layout 不得按 stereo 猜测。
- [ ] 转换 buffer 从有界 pool 获取并记录复制/重采样成本。

## AUD-002：AudioWorklet ring

- [ ] isolated 模式使用共享原子 ring；非隔离模式使用有界 transferable blocks。
- [ ] header 包含 read/write index、capacity、format generation、epoch 和 underrun/overrun counter。
- [ ] process callback 不分配大对象、不记录日志、不阻塞、不调用媒体 parser。
- [ ] stop/reset 清空 ring 并递增 generation，旧 block 不得播放。

## AUD-003：时钟和漂移修正

- [ ] 从实际已渲染 sample 数导出 audio clock，补偿 pipeline/resampler latency。
- [ ] underflow 输出静音并计量；overflow 按策略丢最旧安全块，不能无限增长。
- [ ] 小漂移采用有界 resample/rate correction；超过阈值重建同步基线。
- [ ] mute 只影响输出增益，不停止时钟；volume 不改变 PCM 源数据。

## AUD-004：测试

- [ ] 使用确定性 OfflineAudioContext/仿真 clock 测 frame count、频率、channel、时长和 drift。
- [ ] 真实浏览器覆盖 autoplay 拒绝、suspend/resume、设备变化、后台和 AudioContext close。
- [ ] 组合测试 WebCodecs video + WASM audio、WASM video + WebCodecs audio。
- [ ] 24 小时报告 underrun、overrun、drift 分布、校正次数和 A/V p95。

