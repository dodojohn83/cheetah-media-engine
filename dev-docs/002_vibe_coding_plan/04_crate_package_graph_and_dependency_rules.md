# 04. Crate、Package 图与依赖规则

## ARCH-001：共享核心 crate 图

```text
types
├── bitstream
├── container-flv
├── container-mpegts
├── container-isobmff
├── timeline
└── abi
bitstream + types ──> containers
containers + timeline ──> hls-client / pipeline-core
all public capabilities ──> cheetah-media-core facade
```

- [x] crate 名固定为 `cheetah-media-types`、`cheetah-media-bitstream`、`cheetah-container-flv`、`cheetah-container-mpegts`、`cheetah-container-isobmff`、`cheetah-hls-client`、`cheetah-media-timeline`、`cheetah-media-pipeline-core`、`cheetah-media-abi`、`cheetah-media-core`。
- [x] parser/timeline/pipeline 默认 `no_std + alloc`；网络 HLS adapter 使用单独 `std` feature。
- [x] types 不依赖 parser；容器互不依赖；ABI 不拥有业务策略；禁止循环依赖和 feature 反向泄漏。
- [x] 小 crate 可物理合并，但模块职责、feature 和依赖方向必须保持。

## ARCH-002：engine 与 npm 图

- [x] Rust crate 固定为 `cheetah-media-backend-api`、`cheetah-media-engine`、`cheetah-media-web-bindings`、`cheetah-media-testkit`。
- [x] npm 内部包为 `@cheetah-media/runtime`；公共包为 `@cheetah-media/web`、`@cheetah-media/components`。
- [x] `components` 只能依赖公共 SDK，不直接访问 WASM 内存；SDK 只能经 runtime 控制 worker。
- [x] backend-api 不依赖 Web 类型；engine 不直接调用 DOM/WebCodecs/MSE。
- [x] codec pack 通过 manifest/ABI 装载，不成为 SDK 静态源码依赖。

## ARCH-003：平台端口和 Future 边界

- [x] engine ports 至少覆盖 ByteSource、Demuxer、Decoder、Renderer、AudioSink、Recorder、Clock、MetricsSink。
- [x] ports 使用平台中立 descriptor 和稳定错误，不暴露 DOM、JNI、Qt、ArkTS 类型。
- [x] Future 平台只能新增 adapter，不得复制 container/timeline/planner。
- [x] 发布和采集相关类型不进入 Web v1 实现，仅保留未来命名空间，禁止空 trait 实现。

## ARCH-004：依赖和 unsafe 审计

- [x] parser、timeline、pipeline crate 设置 `unsafe_code = "forbid"`。
- [x] ABI、WASM 和 FFmpeg shim 设置 `unsafe_code = "deny"`，只在审计模块局部允许并写 Safety 注释（当前 WASM 绑定设置 `unsafe_code = "deny"`；FFmpeg shim 在 codec pack 后续实现）。
- [x] 新依赖必须说明必要性、替代方案、维护状态、WASM/no_std 支持、许可证和体积影响。
- [x] 禁止容器 parser 引入异步 runtime、HTTP client、日志实现或全局 allocator 假设。

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
corepack pnpm typecheck
corepack pnpm test
```
结果: 全部 Rust/JS 检查通过；新增核心 crate 14 个测试通过；容器解析器、HLS master 解析、Timeline、Pipeline、ABI ports 实现；无循环依赖；`unsafe_code` 与 lint 配置完整。
制品或报告: `crates/` 下新增 `cheetah-media-bitstream`、`cheetah-container-*`、`cheetah-media-timeline`、`cheetah-media-abi`、`cheetah-media-pipeline-core`、`cheetah-hls-client`、`cheetah-media-core` 及其 README。
已知限制: codec pack 未实际构建；core/server 独立仓库未拆分。
复核人/日期: Devin / 2026-07-13


