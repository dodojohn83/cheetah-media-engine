# 09. H.264/H.265 与音频 Bitstream

## BIT-001：H.264 参数集和格式转换

- [ ] 增量解析 Annex-B、AVCC、SPS/PPS、slice header 最小字段和 IDR 判定。
- [ ] AVCC↔Annex-B 转换保留 NAL 边界，支持 1/2/4-byte length size，拒绝越界长度。
- [ ] 生成稳定 codec string 和 decoder config；配置实质变化递增 generation。
- [ ] 处理 AUD/SEI/重复参数集/带内参数集，不把任意非 IDR 标为随机访问点。

## BIT-002：H.265 参数集和格式转换

- [ ] 增量解析 Annex-B、HVCC、VPS/SPS/PPS、NAL type 和 IRAP 类型。
- [ ] 正确区分 IDR、CRA、BLA 及其随机访问限制，生成 RFC 兼容 codec string。
- [ ] 支持 FLV/fMP4 中 length-prefixed 输入和 decoder 所需格式转换。
- [ ] 参数集缺失、引用不完整或超限返回结构化错误并等待可恢复关键点。

## BIT-003：AAC、MP3、G.711

- [ ] AAC 支持 AudioSpecificConfig、ADTS、sample rate index、channel config 和 frame duration。
- [ ] MP3 解析 header、采样率、channel、bitrate/frame length，支持跨 chunk frame。
- [ ] G.711A/U 提供 Rust table/SIMD 可选实现，输出明确的 PCM 格式和每 sample 时间。
- [ ] 音频配置改变触发 sink reconfigure；不支持的 profile/channel layout 返回 Unsupported。

## BIT-004：测试和基准

- [ ] 使用官方/自有合法最小向量覆盖每种格式、参数集变化和截断位置。
- [ ] property：任意输入不 panic、不越界、消费进度单调；转换往返保持 NAL payload。
- [ ] fuzz H.264/H.265 config record、ADTS/MP3 header 和 chunk splitter。
- [ ] benchmark 输出 MB/s、分配和复制，禁止为探测关键帧复制完整 access unit。

