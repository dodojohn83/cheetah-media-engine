# 01. 执行契约与范围

## GOV-001：冻结 Web v1 需求基线

**仓库**：engine 文档。**前置**：无。**输出**：可测试的需求清单和唯一需求编号。

- [ ] 将三类输入协议、六类 codec、五级后端路线、SDK/UI/录制能力拆成原子需求并链接到具体任务。
- [ ] 为每项标注 `Required`、`Conditional` 或 `Future`；Web v1 不允许出现含义不明的“尽量支持”。
- [ ] 固定参考硬件、浏览器通道、网络条件、测试流规格和指标统计窗口。
- [ ] 明确“可用”必须包含启动、运行、故障恢复、停止、重复创建销毁五类行为。

**验收**：README 覆盖矩阵无孤立需求；每个 Required 项至少有一个自动化测试和一个验收任务。

## GOV-002：冻结非目标和扩展入口

- [ ] 将 WebRTC/WebTransport、PS/裸流、行业录像回放、PTZ、加密、AI、语音对讲列为 Future。
- [ ] 将 Qt、Android、iOS、鸿蒙和发布链路列为 Future，只允许预留无平台假设的 Rust ports/C ABI。
- [ ] 明确不提供 Jessibuca JS API 兼容层；功能追踪与 API 兼容分开管理。
- [ ] 禁止为了 Future 能力降低 Web v1 的类型安全、内存边界或发布门禁。

## GOV-003：工作包状态与变更控制

- [ ] 状态只使用 `Blocked/In Progress/In Review/Done`；Done 必须附完成证据。
- [ ] 每个 PR 只对应一个主任务 ID；必要的机械修改列为同任务子项。
- [ ] 契约变更需同时更新设计、执行计划、contract test 和版本策略。
- [ ] ABI/公开 API 的破坏性变更在 v1 发布前也必须记录迁移说明，禁止静默变更。
- [ ] 阻塞报告包含复现、预期、实际、已尝试方案和需要的决策，不得只写“环境问题”。

## GOV-004：任务完成证据模板

每个任务完成后在任务末尾追加：

```text
状态: Done
仓库/提交: <repo>@<sha-or-tag>
验证命令: <copy-pasteable command>
结果: <passed counts / metrics>
制品或报告: <relative path or immutable URL>
已知限制: <none or issue id>
复核人/日期: <name> / <ISO-8601>
```

**禁止**：仅附截图、仅写“本地通过”、使用可变 branch 代替提交、遗漏失败/跳过用例。

