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

- [ ] core/engine 提交 `rust-toolchain.toml`，包含 `rustfmt`、`clippy`、`wasm32-unknown-unknown`。
- [ ] engine 通过 Corepack 和 `packageManager` 固定 pnpm，依赖精确锁定并提交 lockfile。
- [ ] codec pack manifest 记录 Emscripten、FFmpeg source hash、configure flags 和 patch hash。
- [ ] 完成最小 native、WASM、Vite、Vitest、Playwright smoke；不兼容时先提交最小复现和基线修订。

## BOOT-002：建立 workspace 骨架

- [ ] core 创建 `crates/`、`testing/fixtures`、`testing/property`、`fuzz/` 和 `benches/`。
- [ ] engine 创建 `crates/`、`packages/`、`codec-packs/`、`apps/web-demo`、`tests/browser`、`tests/performance`。
- [ ] 根配置统一 license、repository、edition、MSRV、lint、profile 和依赖版本。
- [ ] JS 开启 TypeScript strict、noUncheckedIndexedAccess、exactOptionalPropertyTypes 和导出边界检查。
- [ ] workspace 默认命令不下载私有流、不依赖外部在线服务，并可在全新环境运行。

## BOOT-003：CI 作业矩阵

- [ ] Rust：fmt、clippy、unit/doc test、feature powerset、`no_std + alloc`、wasm32、deny/advisory。
- [ ] Web：install frozen lockfile、typecheck、lint、unit、build ESM/IIFE、bundle size。
- [ ] Browser：Chrome/Edge/Firefox 常规矩阵；Safari 在 macOS runner；能力缺失验证回退而非强行通过同一路径。
- [ ] Codec pack：三种 WASM variant 构建、导出符号、许可证和可替换性测试。
- [ ] Nightly：fuzz smoke、性能趋势、长时测试；发布候选执行完整 24 小时 soak。

**失败语义**：禁止允许失败的 Required job；flaky test 第一次出现即建 issue，三次重复前不得用重试掩盖。

## BOOT-004：制品和缓存安全

- [ ] cache key 包含 OS、架构、Rust/Node/Emscripten 版本、lock hash 和 feature set。
- [ ] WASM、JS、source map、codec pack、SBOM 作为同一次构建的关联制品保存。
- [ ] 发布作业从 clean checkout 重建，校验 hash，不复用 PR 未验证制品。
- [ ] 日志不得输出流 URL 凭证、Cookie、Authorization、媒体 payload 或用户路径。

