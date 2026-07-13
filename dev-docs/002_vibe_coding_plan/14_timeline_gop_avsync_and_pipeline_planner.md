# 14. 时间线、GOP、A/V Sync 与 Pipeline 模型

## TIME-001：统一时间线

- [ ] 将输入 DTS/PTS 转换为内部高精度整数时钟，保存原 timebase 用于输出。
- [ ] 按 epoch 处理 wrap、reset、discontinuity；同 epoch 内保证调度时间单调但不篡改原始时间。
- [ ] 定义 preroll、playing、catch-up、rebuffering、ended 的时间线输入输出。
- [ ] 统计 jitter、buffer level、live latency、drift 和 dropped duration。

## TIME-002：GOP cache 和随机访问

- [ ] cache 从最新可用 config + 独立解码点开始，按字节、帧数和时长三重限制。
- [ ] config generation 改变时旧 GOP 不得用于新 decoder；等待新的有效随机访问点。
- [ ] H.265 CRA/IDR 的开放 GOP 限制写入 bitstream→timeline contract。
- [ ] 多消费者共享不可变 packet，不为每个 backend/视图复制 GOP。

## TIME-003：A/V 同步策略

- [ ] 音频可用时以实际 audio render clock 为主；无音频时使用单调 wall clock。
- [ ] 小漂移通过调度/有限音频校正处理，大跳变创建 discontinuity 并重建基线。
- [ ] 视频迟到 drop policy 不破坏参考链；音频 underflow/overflow 有明确静音/丢样策略。
- [ ] 目标 p95 A/V 偏差 ≤50ms，统计不得排除恢复窗口而不说明。

## PLAN-001：平台无关 pipeline 请求/结果

- [ ] `PipelineRequest` 包含输入协议、tracks、延迟模式、隔离状态、用户约束和资源预算。
- [ ] `CapabilitySnapshot` 保存探测结果、可信度、失败原因和有效期。
- [ ] `PipelinePlan` 明确 transport/demux/decode/render/audio/record 路径、所需转换和预估复制。
- [ ] planner 为纯函数或可重放决策；相同输入产生相同候选顺序和 reason codes。
- [ ] 禁止在共享 core 直接调用浏览器 capability API。

