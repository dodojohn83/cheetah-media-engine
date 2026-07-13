# 28. 多画面、资源预算与主子码流

## WALL-001：`<cheetah-wall>` 模型

- [ ] 支持 1/4/9/16 固定布局和显式 cell id；增删/重排不错误复用 player epoch。
- [ ] 每 cell 通过公共 SDK 控制，不绕过 API 访问 runtime/WASM。
- [ ] active、selected、visible、fullscreen 状态分离；焦点切换不隐式重连全部流。
- [ ] wall 销毁必须递归销毁其拥有的 player；外部注入实例的所有权由配置声明。

## WALL-002：全局资源预算器

- [ ] 预算输入包括硬解实例、软解线程、CPU、GPU、总像素率、网络、WASM/JS 内存和音频输出数。
- [ ] 每个 player 上报需求和实际使用；预算器分配 backend/variant/render fps，不直接解析媒体。
- [ ] 优先级固定为 fullscreen/selected/visible/background，可由用户在安全范围调整。
- [ ] 超预算时依次降低非焦点码流/帧率、暂停不可见渲染，最后明确拒绝，不允许系统抖动。

## WALL-003：主子码流自动切换

- [ ] HLS 使用 VariantInfo；FLV/fMP4 使用用户提供的关联 source group，SDK 不猜 URL。
- [ ] 放大/选中且预算允许时切主码流；缩小/后台时切子码流，带滞回和最小驻留时间。
- [ ] 切换等待新流 config+关键帧后原子呈现，可保留旧帧但不长时间双解。
- [ ] 失败回退原流并输出 variantchange reason，不进入无限切换循环。

## WALL-004：密度测试

- [ ] 参考平台验证 9×1080p25 H.265 或 16×720p15 硬解目标。
- [ ] 报告各 cell 首帧、延迟、丢帧、码流、backend、CPU/GPU/内存和降级动作。
- [ ] 反复布局切换、全屏、滚动不可见、主子码流失败无黑帧污染和资源泄漏。
- [ ] 测试预算器在能力误报、decoder quota 和 GPU device loss 下重新收敛。

