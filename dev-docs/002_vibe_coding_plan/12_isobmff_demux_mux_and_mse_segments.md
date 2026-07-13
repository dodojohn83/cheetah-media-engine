# 12. ISOBMFF、fMP4 与 MSE Segment

## MP4-001：受限增量 box parser

- [ ] 解析 32/64-bit size、嵌套 box 和跨 chunk 数据；深度、box size、table count 有上限。
- [ ] v1 必需 box：ftyp/moov/mvhd/trak/mdia/minf/stbl、mvex/trex、moof/traf/tfhd/tfdt/trun、mdat。
- [ ] 解析 avcC/hvcC/esds 和 sample entry，生成 TrackInfo/codec string/config。
- [ ] 未知 box 可安全跳过；结构冲突、越界 offset、整数溢出返回稳定错误。

## MP4-002：fragment sample 提取

- [ ] 组合 default/sample duration、size、flags、composition offset 和 base data offset。
- [ ] 正确输出 DTS/PTS、keyframe、duration 和 payload slice，避免逐 sample 复制。
- [ ] sequence gap、tfdt 回退和 init segment 变化产生 discontinuity/generation。
- [ ] HTTP/WS 流式输入在 init 完成前有界缓存 media fragment。

## MP4-003：MSE/CMAF segmenter

- [ ] 从 packet 生成确定性 init segment 和短 media segment；关键帧边界优先。
- [ ] segment 时间范围、sequence 和 codec generation 显式传给 MSE backend。
- [ ] 不支持组合在 planner 阶段排除，不能生成浏览器必然拒绝的 MIME/config。
- [ ] mux 不在内存保存完整直播，fragment 和写出队列都有上限。

## MP4-004：MP4/fMP4 录制和验证

- [ ] seekable 目标可生成标准 MP4；流式目标默认 fragmented MP4。
- [ ] 中断文件的可恢复行为和 metadata finalize 限制写入 API 文档。
- [ ] 使用独立解析器验证 box、sample count、payload hash、duration、关键帧和 A/V sync。
- [ ] golden/fuzz 覆盖 malicious size/offset/count、负 composition offset 和 config 切换。

