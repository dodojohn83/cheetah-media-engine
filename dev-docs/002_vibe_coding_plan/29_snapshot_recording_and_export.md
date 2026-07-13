# 29. 截图、录制与导出

## CAP-001：截图

- [ ] `snapshot` 定义来源为最后已呈现帧，参数包括格式、质量、目标尺寸和是否包含覆盖层。
- [ ] 支持 PNG/JPEG/WebP 中浏览器实际可编码格式；不支持时返回 Unsupported。
- [ ] 色彩、方向、visible rect 与画面一致；无首帧、surface 丢失、跨域受限返回明确错误。
- [ ] readback/编码在可行时移出主线程，期间不阻塞实时 render queue。

## RECFILE-001：录制会话 API

- [ ] `startRecording` 返回独立 session，支持 stop/cancel/stats；同 player 并发数有上限。
- [ ] 配置固定 container、目标 WritableStream/File System adapter、分片、大小/时长上限。
- [ ] raw remux 只写原 codec packet，不承诺转码；不兼容 container/codec 在开始前拒绝。
- [ ] recorder 订阅共享 packet，不能为了录制复制整条解码 pipeline。

## RECFILE-002：流式 MP4/fMP4/FLV

- [ ] seekable 目标可完成普通 MP4 finalize；通用流目标默认 fMP4 或 FLV。
- [ ] 写入服从目标 backpressure，超时/磁盘错误停止该录制但不默认停止播放。
- [ ] config/track/epoch 变化按 container 规则切文件或终止，禁止静默写损坏内容。
- [ ] 文件名、MIME、extension、partial 标记和完成 metadata 可由调用方获取。

## RECFILE-003：验收

- [ ] 独立工具验证容器结构、codec、sample count、payload hash、关键帧、时长和 A/V sync。
- [ ] 覆盖正常 stop、用户 cancel、页面关闭、目标报错、空间上限、断流重连和 config change。
- [ ] 2 小时录制不在 JS/WASM 内存保存完整文件，内存保持有界。
- [ ] 录制指标包括写入字节、时长、队列、backpressure、分片和最终状态。

