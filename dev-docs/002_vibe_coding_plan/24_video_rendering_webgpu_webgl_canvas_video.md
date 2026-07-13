# 24. Video/WebGPU/WebGL/Canvas 渲染

## REN-001：统一渲染输入和 surface

- [ ] Renderer 接收 VideoFrame 或 plane descriptor、visible rect、色彩、timestamp、epoch/generation。
- [ ] surface resize、DPR、fit/fill/stretch、旋转和镜像不修改解码帧数据。
- [ ] 新 surface/device generation 后旧提交被丢弃；渲染完成必释放输入 frame。
- [ ] snapshot 通过 renderer 的受控 readback 接口，不能侵入 decoder。

## REN-002：路径优先级

- [ ] WebCodecs+HTMLVideo/VideoFrame 可直接呈现时优先最少复制路径。
- [ ] WebGPU 支持常见 YUV plane upload/外部纹理、色彩矩阵和 device lost 重建。
- [ ] WebGL2 支持 I420/NV12/RGBA texture、stride、shader conversion 和 context lost。
- [ ] Canvas2D 仅为最终兼容/诊断路径，能力报告其复制和性能限制。

## REN-003：色彩和画面正确性

- [ ] 支持至少 BT.601/709、limited/full range；未知 metadata 使用文档化默认并计量。
- [ ] coded/visible size、奇数尺寸和 stride 不得导致越界或边缘污染。
- [ ] 参数集分辨率变化原子重建 texture/surface，最后一帧策略由配置决定。
- [ ] 多画面各实例 viewport/scissor 隔离，不互相清屏或污染状态。

## REN-004：测试和性能

- [ ] golden pattern 比较色差、裁剪、旋转、镜像和 resize。
- [ ] 注入 WebGPU device lost、WebGL context lost、frame close 和 surface detach。
- [ ] 统计 upload bytes、draw latency、present interval、dropped frame 和 GPU memory estimate。
- [ ] snapshot 在所有 renderer 路径输出正确尺寸/方向且不阻塞实时播放。

