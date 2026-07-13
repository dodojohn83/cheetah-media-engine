# 10. FLV Demux、Mux 与录制

## FLV-001：增量 FLV demux

- [ ] 状态机解析 header、previous tag size、audio/video/script tag，可接受任意 chunk 边界。
- [ ] 校验 tag size、stream id、timestamp extension 和限制；截断返回 NeedMoreData。
- [ ] H.264/H.265 video tag 输出 config/media packet、DTS 和带符号 CTS 计算的 PTS。
- [ ] AAC/MP3/G.711 输出 TrackInfo 和 packet；未知 codec 可跳过但产生诊断。
- [ ] metadata 只解析受限必要字段，AMF 深度、字符串、数组和总字节均有限制。

## FLV-002：HTTP/WS-FLV live 语义

- [ ] 连接建立后在 config + 可解码关键帧满足前保持 Preroll，不向 decoder 投递残缺 GOP。
- [ ] timestamp wrap/reset 产生明确 discontinuity 和新 epoch，不用简单 clamp 隐藏错误。
- [ ] 网络重连后丢弃旧 session 迟到数据，并重新等待 metadata/config/keyframe。
- [ ] WS message 边界不得被当作 FLV tag 边界。

## FLV-003：流式 FLV mux/record

- [ ] mux 从 TrackInfo 生成 header/config tag，按 DTS 排序并写正确 CTS。
- [ ] recorder 只保留有界 reorder queue；目标 WritableStream 背压向引擎传播。
- [ ] stop 正常写完可恢复尾部；取消保留明确 partial 状态，不在内存累积完整录像。
- [ ] codec/config 不兼容时结束当前文件或返回 Unsupported，不静默生成损坏文件。

## FLV-004：验证

- [ ] golden 覆盖 H.264/H.265 + AAC/G.711/MP3、负 CTS、时间戳 wrap 和损坏 tag。
- [ ] 与 server 固定 fixture 比较 Track、Packet、时间线和错误 offset。
- [ ] demux→mux→demux contract 验证关键信息、payload hash 和时间戳。
- [ ] 浏览器录制文件用独立播放器/探针验证可打开、时长和 A/V sync。

