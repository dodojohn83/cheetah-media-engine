# Cheetah Media Engine Web v1 外部编程执行计划

## 1. 文档定位

本目录把 [001 架构设计](../001_next_generation_media_engine/README.md) 转换为可直接交给外部编程体执行的任务。执行体不得依赖口头约定或未写入本文档集的隐含决策；发现契约缺失时必须先补文档和测试，不得自行发明兼容行为。

本轮只交付 Web v1：HTTP/WS-FLV、HLS/LL-HLS（TS/fMP4）、HTTP/WS-fMP4，H.264/H.265、AAC、G.711A/U、MP3，动态 WebCodecs/MSE/WASM 回退，单窗与 1/4/9/16 宫格、主子码流、截图、流式录制、诊断和发布制品。Jessibuca Pro 后续能力、Qt/Android/iOS/鸿蒙和双向引擎仅保留接口入口，不进入本轮完成声明。

## 2. 三仓边界

| 仓库 | 路径 | 职责 |
| --- | --- | --- |
| 共享核心 | `../cheetah-media-core-rs` | 纯媒体类型、bitstream、容器、HLS、时间线、pipeline 模型、ABI、fixture |
| 播放引擎 | 当前仓库 | engine、Web bindings/runtime/SDK/components、codec packs、demo、浏览器和性能测试 |
| 媒体服务 | `../cheetah-media-server-rs` | 通过 `cheetah-codec` 兼容门面消费共享核心；保留服务端会话和协议驱动 |

依赖只能从产品仓库指向共享核心。共享核心不得依赖浏览器、UI、服务端 session、网络 driver 或任一产品仓库。

## 3. 执行规则

1. 严格按 Phase 和任务依赖实施；阻塞项未关闭时不得提前完成下游任务。
2. 每个任务编号对应一个可独立评审、测试和回滚的 PR 工作包；不得把多个阶段压进不可审查的大 PR。
3. 完成 `[ ]` 后改为 `[x]`，并追加仓库、commit/tag、测试命令、结果和证据路径。
4. 公共契约和 contract test 先于 adapter。三仓变更按 core tag → server facade → engine 的顺序落地。
5. 禁止 `todo!()`、`unimplemented!()`、空 provider、吞错、HTTP/Promise 假成功。暂不支持能力返回稳定 `Unsupported`。
6. 热路径禁止 JSON/Base64 媒体负载、逐帧 wasm-bindgen 对象和无界队列；所有缓存、池、批次、重试和并发必须有上限。
7. 任何改变 crate 图、ABI、公开 TypeScript API、错误语义、复制预算或 001 边界的实现，先修改 001/002 并经评审。
8. 外部代码和 fixture 必须登记来源 commit、许可证、修改和脱敏方式；FFmpeg 只允许 LGPL 配置，GPL/nonfree 必须关闭。
9. CI 不得依赖未提交的本地 path。跨仓开发可临时使用 `[patch]`，提交前必须改为不可变 tag/revision。
10. 性能结论必须有原始数据、环境、命令和对照；不得在无合法可复现实验时宣称优于第三方产品。

## 4. 需求矩阵

Web v1 原子需求清单与验收链接见 [00_requirements_matrix.md](00_requirements_matrix.md)。所有 002 任务的范围变更必须先更新需求矩阵。

## 5. 阶段索引

| Phase | 文档 | 阶段交付 |
| --- | --- | --- |
| 0 | [01](01_execution_contract_and_scope.md)–[05](05_migration_inventory_fixtures_and_licensing.md) | 范围、三仓基线、工具链、依赖图、迁移资产 |
| 1 | [06](06_media_types_time_errors_and_limits.md)–[15](15_media_server_compatibility_facade_migration.md) | 共享媒体核心、ABI、容器、HLS、server facade |
| 2 | [16](16_engine_state_machine_backend_ports_and_scheduler.md)–[25](25_recovery_latency_control_and_resource_cleanup.md) | Web 播放内核和完整回退链 |
| 3 | [26](26_web_sdk_public_api_events_errors.md)–[30](30_observability_diagnostics_security_privacy.md) | SDK、组件、多画面、录制、诊断和安全 |
| 4 | [31](31_testkit_fixtures_property_fuzz_contracts.md)–[35](35_web_v1_integration_acceptance_and_handoff.md) | 跨仓测试、浏览器、性能、发布和交接 |

## 6. 全局完成定义

- [ ] 01–35 所有任务完成，无未登记 TODO、跳过测试或临时 path 依赖。
- [ ] 共享核心 native、`no_std + alloc`、`wasm32-unknown-unknown` 构建及全部 contract suite 通过。
- [ ] media server 与 engine 对相同 fixture 产生一致 Track、Packet、时间线和错误分类。
- [ ] WebCodecs、MSE、Threads+SIMD、SIMD、baseline 路径均有成功和强制失败证据。
- [ ] 隔离与非隔离部署均可播放；不支持组合稳定回退或返回 `Unsupported`。
- [ ] 单窗、1/4/9/16 宫格、主子码流、截图、MP4/fMP4/FLV 录制完成。
- [ ] 首帧、延迟、A/V sync、丢帧、复制、密度、24 小时 soak 达到 001 门禁。
- [ ] npm ESM/IIFE、自托管 WASM、CDN、ABI manifest、SBOM、许可证和安全门禁通过。

## 7. 001→002 覆盖矩阵

| 001 要求 | 002 归属 |
| --- | --- |
| 三仓架构与共享核心 | 02、04、05、15 |
| 统一媒体模型与零拷贝 | 06–08、14、33 |
| FLV/TS/fMP4/HLS | 09–14 |
| WebCodecs/MSE/WASM 回退 | 17、19–25、32 |
| Web SDK 与组件 | 26–27 |
| 1/4/9/16 宫格、主子码流 | 28、33 |
| 截图和流式录制 | 10、12、29 |
| 可观测、安全、许可 | 05、30、34 |
| 浏览器、性能和长稳 | 31–35 |
| Jessibuca Pro 后续追踪 | 01、35 |
| Native 与双向扩展入口 | 04、16、35 |

## 8. 标准验证命令

各任务只运行与其仓库和 feature 相关的子集，但完成证据必须记录实际完整命令。发布候选必须运行全集。

### 共享核心

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build --workspace --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

### Web 引擎

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm exec playwright test
```

### Media Server 兼容验证

```bash
CARGO_TARGET_DIR=target/check-codec cargo check -p cheetah-codec --no-default-features
CARGO_TARGET_DIR=target/check-http-flv-wasm cargo check -p cheetah-http-flv-core --target wasm32-unknown-unknown --no-default-features
CARGO_TARGET_DIR=target/test-server cargo test --workspace --all-features
```

server 命令必须串行运行。若实际 package/feature 名在迁移前发生变化，先更新本文档，不得在完成证据中静默替换。

## 9. 阶段闸门

| 闸门 | 必须满足后才能进入下一阶段 |
| --- | --- |
| G0 工程闸门 | 三仓基线可复现；工具链 smoke、no_std/wasm 基线、许可证清单通过 |
| G1 核心闸门 | ABI golden、容器/HLS/timeline contract、server facade 双跑全部通过 |
| G2 播放闸门 | 每级后端成功与失败注入通过；隔离/非隔离播放；资源 ledger 归零 |
| G3 产品闸门 | SDK/API contract、组件、多画面、主子码流、截图和三类录制通过 |
| G4 发布闸门 | 浏览器矩阵、性能、复制、24 小时 soak、SBOM、安装和回滚演练通过 |

任一闸门失败时只允许修复当前或上游任务；不得以 UI 演示、单一浏览器成功或人工观察代替门禁证据。
