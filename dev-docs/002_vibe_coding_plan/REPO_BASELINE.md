# 三仓基线记录

## 1. 说明

本文件是 [02_three_repo_workflow_and_baseline.md](02_three_repo_workflow_and_baseline.md) 的产出。当前工作区仅包含 `dodojohn83/cheetah-media-engine`，因此先记录 engine 基线；`cheetah-media-core-rs` 与 `cheetah-media-server-rs` 在本轮按 monorepo 方式置于 engine 的 workspace 中，待后续需要时再拆分为独立仓库。

## 2. 仓库基线

### 2.1 engine（当前仓库）

| 字段 | 值 |
| --- | --- |
| 仓库路径 | `/home/ubuntu/repos/cheetah-media-engine` |
| remote | `https://github.com/dodojohn83/cheetah-media-engine.git` |
| 默认分支 | `main` |
| 起始 commit | `9bcdf69 add dev-docs/002_vibe_coding_plan` |
| dirty 状态 | 干净 |
| 当前 PR | `wp/01-execution-contract` |

### 2.2 core / server

| 字段 | 值 |
| --- | --- |
| 计划路径 | `../cheetah-media-core-rs` / `../cheetah-media-server-rs` |
| 当前状态 | 尚未创建独立仓库 |
| 处理方式 | 在 `cheetah-media-engine` workspace 内以 `crates/cheetah-media-*` 形式落地；模块职责、feature 和依赖方向保持与三仓设计一致 |

## 3. 工具链版本

| 工具 | 固定版本 | 来源 |
| --- | --- | --- |
| Rust | `1.94.1` | 002 计划 / `rust-toolchain.toml`（WP-03） |
| Node.js | `22.22.2` | 002 计划 / `.nvmrc`（WP-03） |
| pnpm | `11.12.0` | 002 计划 / `package.json` `packageManager`（WP-03） |
| TypeScript | `7.0.2` | 002 计划 |
| Vite | `8.1.4` | 002 计划 |
| Rollup | `4.62.2` | 002 计划 |
| Vitest | `4.1.10` | 002 计划 |
| Playwright | `1.61.1` | 002 计划 |
| wasm-bindgen | `0.2.126` | 002 计划 |
| wasm-pack | `0.15.0` | 002 计划 |
| Emscripten | `6.0.2` | 002 计划（codec pack，后续任务） |
| FFmpeg | `8.1.2` | 002 计划（codec pack，后续任务） |

## 4. 分支与提交约定

- 分支命名：`wp/<TASK-ID>-<slug>`
- 提交标题：以任务 ID 开头，例如 `[03] add workspace skeleton`
- PR 描述包含：范围、契约变化、风险、测试、性能、许可证、回滚

## 5. 基线验证命令

```bash
rustc --version
cargo --version
node --version
corepack --version
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

server 的不同 feature/target 检查将在独立 core/server 仓库建成后使用独立 `CARGO_TARGET_DIR` 串行执行；当前 monorepo 方案下，上述命令由 engine workspace 统一运行。
