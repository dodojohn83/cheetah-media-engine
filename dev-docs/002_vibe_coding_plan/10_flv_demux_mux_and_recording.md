# 10. FLV Demux、Mux 与录制

## FLV-001：增量 FLV demux

- [x] 状态机解析 header、previous tag size、audio/video/script tag，可接受任意 chunk 边界。
- [x] 校验 tag size、stream id、timestamp extension 和限制；截断返回 NeedMoreData。
- [x] H.264/H.265 video tag 输出 config/media packet、DTS 和带符号 CTS 计算的 PTS。
- [x] AAC/MP3/G.711 输出 TrackInfo 和 packet；未知 codec 返回 Unsupported。
- [x] metadata 只解析受限必要字段，AMF 深度、字符串、数组和总字节均有限制。

## FLV-002：HTTP/WS-FLV live 语义

- [ ] 连接建立后在 config + 可解码关键帧满足前保持 Preroll，不向 decoder 投递残缺 GOP。（后续 pipeline 层实现）
- [x] timestamp wrap/reset 产生明确 discontinuity 和新 epoch，不用简单 clamp 隐藏错误。
- [ ] 网络重连后丢弃旧 session 迟到数据，并重新等待 metadata/config/keyframe。（后续 pipeline 层实现）
- [x] WS message 边界不被当作 FLV tag 边界；demuxer 按字节流解析。

## FLV-003：流式 FLV mux/record

- [x] mux 从 TrackInfo 生成 header/config tag，按 DTS 排序并写正确 CTS。
- [x] recorder 只保留有界 reorder queue；目标 writer 背压通过 flush threshold 传播。
- [x] stop 正常写完尾部；取消返回明确 partial 状态，不在内存累积完整录像。
- [x] codec/config 不兼容时返回 Unsupported，不静默生成损坏文件。

## FLV-004：验证

- [x] demux→mux→demux contract 验证关键信息、payload 和时间戳（H.264 + AAC）。
- [x] 时间戳 wrap 用 32-bit 边界值回归验证。
- [ ] golden 覆盖 H.264/H.265 + AAC/G.711/MP3、负 CTS、损坏 tag。（ golden fixture 后续 WP-31 补充）
- [ ] 与 server 固定 fixture 比较 Track、Packet、时间线和错误 offset。
- [ ] 浏览器录制文件用独立播放器/探针验证可打开、时长和 A/V sync。
