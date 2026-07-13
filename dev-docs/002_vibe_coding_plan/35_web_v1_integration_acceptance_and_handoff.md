# 35. Web v1 集成、验收与交接

## INT-001：建立发布候选集成环境

- [ ] 固定 core/server/engine/npm/codec pack 的 commit、tag、manifest 和 hash。
- [ ] 部署 isolated、non-isolated、self-host、CDN 四套环境和脚本化测试媒体源。
- [ ] demo 覆盖单窗、协议/codec 选择、强制后端、回退注入、宫格、录制和 diagnostics。
- [ ] 环境从 clean checkout 一键构建，禁止引用开发机绝对路径和未发布依赖。

## INT-002：Web v1 功能验收

- [ ] HTTP/WS-FLV、HLS/LL-HLS TS/fMP4、HTTP/WS-fMP4 Required 矩阵全部通过。
- [ ] H.264/H.265、AAC、G.711A/U、MP3 的适用路径有真实播放证据。
- [ ] WebCodecs→MSE→Threads+SIMD→SIMD→baseline 成功/失败矩阵完整。
- [ ] 单窗、1/4/9/16 宫格、主子码流、截图、MP4/fMP4/FLV 录制完成。
- [ ] stop/reload/destroy、断流、后台、config change、backend/device fault 无泄漏和旧帧污染。

## INT-003：非功能验收

- [ ] PERF-001–005 全部达到门禁，原始数据和报告可复现。
- [ ] fuzz/property/contract/browser/security/license/API/ABI/SBOM jobs 无 Required 失败或未解释 skip。
- [ ] 隔离和非隔离环境均通过；Unsupported 组合给出稳定错误和尝试链。
- [ ] npm ESM/IIFE、自托管/CDN 在空项目 clean install 成功。
- [ ] 三仓版本和回滚路径经过演练，不依赖未提交本地状态。

## INT-004：001 需求签收和已知限制

- [ ] 逐行复核 001→002→任务→测试→证据链，所有 Required 项有负责人和签收。
- [ ] Conditional 项记录触发条件、验证环境和实际结果。
- [ ] Future 项明确链接后续 backlog，不得计入 Web v1 功能完成率。
- [ ] 已知限制包含影响、规避、浏览器/硬件范围、issue 和计划版本，禁止模糊措辞。

## INT-005：外部执行体交接包

交接包至少包含：

- [ ] 三仓架构/依赖图、固定版本、构建发布手册和回滚手册。
- [ ] 公共 Rust/ABI/TypeScript API report、事件错误表、codec/浏览器能力矩阵。
- [ ] fixture manifest、测试命令、E2E 环境、性能原始数据、soak 和安全/许可证报告。
- [ ] 运维诊断说明、常见部署错误、COOP/COEP/CSP 示例和故障排查树。
- [ ] 未关闭 issue、Future backlog、责任人和支持期限。

## INT-006：发布声明边界

只有 INT-001–005 和 README 全局 DoD 全部完成，才可声明“Cheetah Media Engine Web v1”。不得在 Jessibuca Pro 全量追踪、Native 或双向实时能力未完成时宣称这些范围已实现；可以陈述已验证的单项能力，但必须附测试环境和证据。

