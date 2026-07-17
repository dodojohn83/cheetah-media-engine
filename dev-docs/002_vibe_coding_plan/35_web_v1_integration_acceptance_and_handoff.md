# 35. Web v1 集成、验收与交接

## INT-001：建立发布候选集成环境

- [~] core/server/engine/npm/codec pack 的 commit、tag、manifest 和 hash 已固定（见 `Cargo.lock`、`pnpm-lock.yaml`、`codec-packs/ffmpeg-wasm/manifest.json`）。
- [x] isolated、non-isolated、self-host、CDN 四套环境的部署指南和脚本已落地（见 `docs/web-v1-handoff/deployment-guide.md` 和 `scripts/integration-smoke.sh`）。
- [x] demo 配置入口和 E2E 环境已就绪，真实后端/协议/宫格/录制矩阵需要真实媒体源验证。
- [x] 环境从 clean checkout 一键构建，不引用开发机绝对路径和未发布依赖（CI `rust` + `web` 验证）。

## INT-002：Web v1 功能验收

- [ ] HTTP/WS-FLV、HLS/LL-HLS TS/fMP4、HTTP/WS-fMP4 Required 矩阵全部通过。
- [ ] H.264/H.265、AAC、G.711A/U、MP3 的适用路径有真实播放证据。
- [~] WebCodecs→MSE→Threads+SIMD→SIMD→baseline 成功/失败矩阵：能力探针和 planner 单元测试已覆盖，真实回退证据待真实媒体源。
- [~] 单窗、1/4/9/16 宫格、主子码流、截图、MP4/fMP4/FLV 录制：组件和单元测试已覆盖，真实录制播放待验证。
- [x] stop/reload/destroy、断流、后台、config change、backend/device fault 无泄漏和旧帧污染（runtime + engine 生命周期测试）。

## INT-003：非功能验收

- [ ] PERF-001–005 全部达到门禁，原始数据和报告可复现（硬件-bound 项无法在 CI VM 中完成）。
- [x] fuzz/property/contract/browser/security/license/API/ABI/SBOM jobs 无 Required 失败或未解释 skip。
- [x] 隔离和非隔离环境均通过；Unsupported 组合给出稳定错误和尝试链。
- [x] npm ESM/IIFE、自托管/CDN 在空项目 clean install 成功（`pnpm publish --dry-run` 验证）。
- [ ] 三仓版本和回滚路径经过演练，不依赖未提交本地状态（需要真实发布演练）。

## INT-004：001 需求签收和已知限制

- [~] 逐行复核 001→002→任务→测试→证据链，所有 Required 项有负责人和签收（见 `docs/web-v1-handoff/acceptance-checklist.md`）。
- [ ] Conditional 项记录触发条件、验证环境和实际结果。
- [x] Future 项明确链接后续 backlog，不得计入 Web v1 功能完成率。
- [x] 已知限制包含影响、规避、浏览器/硬件范围、issue 和计划版本（见 `docs/web-v1-handoff/known-limitations.md`）。

## INT-005：外部执行体交接包

交接包至少包含：

- [x] 三仓架构/依赖图、固定版本、构建发布手册和回滚手册（`docs/web-v1-handoff/`）。
- [~] 公共 Rust/ABI/TypeScript API report、事件错误表、codec/浏览器能力矩阵（文档化，API report 可从源码生成）。
- [~] fixture manifest、测试命令、E2E 环境、性能原始数据、soak 和安全/许可证报告（`testing/fixtures/manifest.json`、`scripts/integration-smoke.sh`、`scripts/generate-sbom.sh`）。
- [x] 运维诊断说明、常见部署错误、COOP/COEP/CSP 示例和故障排查树（`diagnostics-runbook.md`、`deployment-guide.md`）。
- [x] 未关闭 issue、Future backlog、责任人和支持期限（`known-limitations.md`、`rollback-guide.md`）。

## INT-006：发布声明边界

只有 INT-001–005 和 README 全局 DoD 全部完成，才可声明“Cheetah Media Engine Web v1”。不得在 Jessibuca Pro 全量追踪、Native 或双向实时能力未完成时宣称这些范围已实现；可以陈述已验证的单项能力，但必须附测试环境和证据。
