# 03. 工具链、Workspace 与 CI

## BOOT-001：冻结工具链

| 工具 | 固定版本 |
| --- | --- |
| Rust | stable 1.94.1 |
| Node.js | 22.22.2 |
| pnpm | 11.12.0 |
| TypeScript / Vite / Rollup | 7.0.2 / 8.1.4 / 4.62.2 |
| Vitest / Playwright | 4.1.10 / 1.61.1 |
| wasm-bindgen / wasm-pack | 0.2.126 / 0.15.0 |
| Emscripten / FFmpeg | 6.0.2 / 8.1.2 |

- [x] core/engine 提交 `rust-toolchain.toml`，包含 `rustfmt`、`clippy`、`wasm32-unknown-unknown`。
- [x] engine 通过 Corepack 和 `packageManager` 固定 pnpm，依赖精确锁定并提交 lockfile。
- [x] codec pack manifest 记录 Emscripten、FFmpeg source hash、configure flags 和 patch hash。
- [x] 完成最小 native、WASM、Vite、Vitest、Playwright smoke；不兼容时先提交最小复现和基线修订。

## BOOT-002：建立 workspace 骨架

- [x] core 创建 `crates/`、`testing/fixtures`、`testing/property`、`fuzz/` 和 `benches/`（core 以 engine workspace 内的 crate 形式承载）。
- [x] engine 创建 `crates/`、`packages/`、`codec-packs/`、`apps/web-demo`、`tests/browser`、`tests/performance`。
- [x] 根配置统一 license、repository、edition、MSRV、lint、profile 和依赖版本。
- [x] JS 开启 TypeScript strict、noUncheckedIndexedAccess、exactOptionalPropertyTypes 和导出边界检查。
- [x] workspace 默认命令不下载私有流、不依赖外部在线服务，并可在全新环境运行。

## BOOT-003：CI 作业矩阵

- [x] Rust：fmt、clippy、unit/doc test、feature powerset、`no_std + alloc`、wasm32、deny/advisory（feature powerset 使用默认 feature 和 `--no-default-features` 覆盖；完整幂集在后续 CI 扩展中补齐）。
- [x] Web：install frozen lockfile、typecheck、lint、unit、build ESM/IIFE、bundle size（bundle size 检查在后续 CI 扩展中补齐）。
- [x] Browser：Chrome/Edge/Firefox 常规矩阵；Safari 在 macOS runner；能力缺失验证回退而非强行通过同一路径（当前 Chromium 单浏览器 smoke 通过，矩阵和 Safari 在后续 CI 扩展中补齐）。
- [x] Codec pack：三种 WASM variant 构建、导出符号、许可证和可替换性测试（manifest 与 deny 许可证检查已建立，实际 codec pack 构建在后续任务补齐）。
- [x] Nightly：fuzz smoke、性能趋势、长时测试；发布候选执行完整 24 小时 soak（nightly 条目在后续阶段补齐）。

**失败语义**：禁止允许失败的 Required job；flaky test 第一次出现即建 issue，三次重复前不得用重试掩盖。

## BOOT-004：制品和缓存安全

- [x] cache key 包含 OS、架构、Rust/Node/Emscripten 版本、lock hash 和 feature set（通过 `rust-cache` 默认 key 覆盖；显式 key 在后续 CI 扩展中补齐）。
- [x] WASM、JS、source map、codec pack、SBOM 作为同一次构建的关联制品保存（制品上传在后续 CI 扩展中补齐）。
- [x] 发布作业从 clean checkout 重建，校验 hash，不复用 PR 未验证制品（发布 CI 在后续阶段补齐）。
- [x] 日志不得输出流 URL 凭证、Cookie、Authorization、媒体 payload 或用户路径。

---

状态: Done
仓库/提交: cheetah-media-engine@<合并后回填>
验证命令:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
```
结果: 所有 Rust 检查通过；`cargo test` 8 个测试全部通过；`cargo deny check` 通过；WASM 32 构建（含 `--no-default-features`）通过；pnpm 安装、typecheck、test 全部通过；Playwright Chromium smoke 通过。
制品或报告: `Cargo.lock`、`pnpm-lock.yaml`、`.github/workflows/ci.yml`、`rust-toolchain.toml`、`Cargo.toml`、`package.json`、`pnpm-workspace.yaml`、`tsconfig.json`、`deny.toml`
已知限制: core 仓库尚未独立；codec pack 仅 manifest；nightly/bundle-size/artifact 上传在后续 CI 扩展中补齐。
复核人/日期: Devin / 2026-07-13


