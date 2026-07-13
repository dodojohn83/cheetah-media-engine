# 11. MPEG-TS Demux 与时钟恢复

## TS-001：增量同步和 PSI

- [ ] 支持任意 chunk、188-byte packet 重同步和有限扫描；持续失步返回 InvalidInput。
- [ ] 解析 PAT/PMT、program/version/CRC、PID 映射和 descriptor，版本变化原子更新 TrackInfo。
- [ ] 检查 continuity counter、duplicate、discontinuity indicator 和 transport error。
- [ ] section/PES assembler 均有长度、PID 数、缓存字节和等待时间上限。

## TS-002：PES、ES 和时间戳

- [ ] 解析 PTS/DTS、payload start、跨 packet PES，并正确处理未知 PES length。
- [ ] H.264/H.265 access unit 组帧必须跨 PES 工作并利用 AUD/slice/参数集边界。
- [ ] AAC ADTS、MP3 和 G.711 映射到统一 packet；不支持 stream type 产生 Unsupported track 诊断。
- [ ] 33-bit wrap 使用每 track unwrap 并在 program discontinuity 时重置。

## TS-003：PCR 和 live 时钟恢复

- [ ] 解析 PCR/OPCR，建立 transport clock，计算 jitter、drift 和 live edge。
- [ ] PCR 缺失时明确降级到 DTS/PTS 时钟并输出 capability/diagnostic。
- [ ] continuity loss 后仅丢受影响 assembler，等待可解码随机访问点恢复。
- [ ] HLS segment 边界不能隐式重置连续时间线；playlist discontinuity 才创建 epoch。

## TS-004：验证

- [ ] golden 覆盖多 program、PMT 更新、PID 切换、wrap、丢包、重复和损坏 CRC。
- [ ] chunk splitter 对每一字节边界产生一致输出。
- [ ] property/fuzz 保证有界内存、无 panic、失败 offset 和恢复进度稳定。
- [ ] 与 HLS 端到端测试验证跨 segment A/V sync、live edge 和追赶。

