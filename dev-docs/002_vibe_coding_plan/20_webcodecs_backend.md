# 20. WebCodecs Backend

## WC-001：视频配置和输入

- [ ] 从 TrackInfo 构造 codec、description、coded size、color space、hardwareAcceleration preference。
- [ ] description/bitstream 格式与 codec string 一致；必要 Annex-B/length-prefix 转换集中且计量。
- [ ] 只在 config generation 或 backend 重建时 configure，不逐帧重复配置。
- [ ] decoder queue 达到上限时停止上游或执行引用安全的实时 drop。

## WC-002：输出和生命周期

- [ ] output callback 立即登记 VideoFrame 所有权；渲染完成或丢弃后始终 `close()`。
- [ ] 以 epoch/generation 过滤迟到 output/error，旧帧不得显示在新流。
- [ ] flush 只用于 drain/stop/切换，不作为日常低延迟机制；reset/close 顺序固定。
- [ ] config change 等待合法关键帧重建 decoder。

## WC-003：音频 WebCodecs 路径

- [ ] 支持浏览器实际可用的 AAC/MP3 config；G.711 走 Rust 解码。
- [ ] AudioData 立即转换/传递到有界 PCM ring 并 close，不在主线程长期持有。
- [ ] sample rate/channel change 触发 AudioWorklet pipeline 原子重建。
- [ ] 音频 decoder 缺失时可与 WASM audio 组合，而非强制视频一起回退。

## WC-004：错误与测试

- [ ] 覆盖 unsupported config、configure throw、decode throw、async error、queue stall、flush/reset 失败。
- [ ] 使用真实 H.264/H.265/AAC/MP3 fixture 验证首帧、帧数、timestamp、color 和 close count。
- [ ] mock 只验证状态机；发布门禁必须用真实浏览器 decoder。
- [ ] 输出 decode queue、decode latency、frame lifetime、drop、reconfigure 和 failure reason 指标。

