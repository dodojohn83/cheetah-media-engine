# 19. 能力探测、路由 Planner 与回退

## ROUTE-001：能力探测

- [ ] 探测 WebCodecs API 存在性和 `isConfigSupported`，但把成功 configure/decode 首个关键帧作为最终证据。
- [ ] MSE 探测 API、MIME/codec string、SourceBuffer 成功创建和最小 init append。
- [ ] WASM 探测 SIMD、threads、shared memory、memory limit、codec pack ABI 和 codec availability。
- [ ] renderer 探测 VideoFrame、WebGPU、WebGL2、Canvas2D 和可用 pixel/texture format。
- [ ] capability 结果带环境 fingerprint、时间、可信度和 reason，配置/设备变化后失效。

## ROUTE-002：候选生成和排序

默认视频路线：WebCodecs → MSE（容器/codec 组合允许时）→ WASM Threads+SIMD → WASM SIMD → WASM baseline。音频和 renderer 独立选择但必须组成有效整体计划。

- [ ] planner 根据输入协议、track、延迟目标、隔离状态、用户禁用项和预算生成候选。
- [ ] MSE 需要 remux 时显式计入转换、缓存和延迟成本。
- [ ] baseline 无法实时满足规格时返回 Unsupported/Degraded，不假装成功后无限积压。
- [ ] 每次选择输出完整 reason chain 和被排除候选原因。

## ROUTE-003：运行期回退状态机

- [ ] configure、首关键帧 decode、连续 decode、append、quota、device loss、codec worker crash 分别定义触发条件。
- [ ] 单 epoch 每个候选最多尝试一次；避免 WebCodecs↔MSE 循环和重建风暴。
- [ ] 切换时停止旧 backend、清空旧 generation、从 config + 可解码关键点恢复。
- [ ] 保持公开 player identity 和事件订阅；产生 `backendchange`、原因和恢复耗时。

## ROUTE-004：确定性测试矩阵

- [ ] 对每种协议/codec/隔离组合生成预期候选顺序 golden。
- [ ] 注入每一级 probe/configure/runtime 失败，证明落到下一合法路径。
- [ ] 所有候选失败时聚合主要失败和尝试链，资源完全释放。
- [ ] capability 缓存不得跨浏览器升级、GPU/device change 或 codec pack version 误复用。

