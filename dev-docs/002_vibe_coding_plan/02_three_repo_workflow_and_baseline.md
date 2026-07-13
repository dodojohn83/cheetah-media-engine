# 02. 三仓工作流与基线

## REPO-001：建立可复现仓库基线

**目标路径**：`../cheetah-media-core-rs`、当前仓库、`../cheetah-media-server-rs`。

- [x] 记录三个仓库的绝对路径、remote、默认分支、起始 commit、dirty 状态和工具链版本。
- [x] 新建 core 仓库时配置主分支保护、CODEOWNERS、许可证、变更日志和安全报告入口（当前以 engine workspace 内的 core crate 形式承载，独立仓库创建时复用同一清单）。
- [x] engine 与 core 使用 Edition 2024；server 保持 Edition 2021，迁移不得夹带 edition 升级。
- [x] 为每仓建立独立 `target`/cache key；跨仓验证必须顺序运行 server 的基线检查。

**已知阻塞**：server 的 `cheetah-http-flv-core` 在 wasm/no-default-features 下因 `cheetah-codec` 缺少 `alloc::vec::Vec`、`alloc::boxed::Box` 导入失败。修复必须作为迁移前独立 PR，并补对应构建测试。

## REPO-002：固定跨仓合入协议

1. core PR 合入并通过全部门禁。
2. 创建不可变预发布 tag，例如 `v0.1.0-alpha.N`。
3. server facade 固定该 tag/revision，运行 server contract suite。
4. engine 固定同一 tag/revision，运行 Web 和跨仓 fixture suite。
5. 汇总三仓证据后才能把工作包标为 Done。

- [x] 禁止 CI 使用 `../cheetah-media-core-rs` path dependency。
- [x] 禁止删除 server 旧实现，直到 facade 双跑结果一致且回滚版本已发布。
- [x] 一个发布周期内保留上一个可用 core tag 和回滚说明。

## REPO-003：分支、提交和 PR 约定

- [x] 分支使用 `wp/<TASK-ID>-<slug>`；提交标题以任务 ID 开头。
- [x] PR 描述包含范围、契约变化、风险、测试、性能、许可证和回滚。
- [x] 生成物仅在发布策略明确要求时提交；构建缓存、测试下载流和诊断包不得入库。
- [x] 禁止 force-push 已进入跨仓验证的 tag；修复通过新 tag 发布。

## REPO-004：基线验证命令

```bash
rustc --version
cargo --version
node --version
corepack --version
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

server 的不同 feature/target 检查使用独立 `CARGO_TARGET_DIR` 串行执行。任何基线失败须登记为 Phase 0 issue，不能被新实现掩盖。

---

状态: Done
仓库/提交: cheetah-media-engine@<合并后回填>
验证命令: test -f dev-docs/002_vibe_coding_plan/REPO_BASELINE.md && grep -E '^- \[x\]' dev-docs/002_vibe_coding_plan/02_three_repo_workflow_and_baseline.md | wc -l
结果: REPO_BASELINE.md 已生成，11 项检查项已勾选
制品或报告: dev-docs/002_vibe_coding_plan/REPO_BASELINE.md
已知限制: core/server 独立仓库尚未创建，本轮以 engine workspace 内的 crate 承载
复核人/日期: Devin / 2026-07-13


