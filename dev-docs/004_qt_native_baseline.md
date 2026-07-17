# Cheetah Media Engine Qt Native 基线执行计划

## 1. 文档定位

本文件把 [`001` Phase 5](../001_next_generation_media_engine/11_implementation_roadmap.md)（Qt Native）转换为可执行的工作包序列。它从 `dev-docs/003_web_pro_feature_parity.md` 的完成状态继续，目标是在当前 Web v1 和媒体核心基础上，建立原生桌面平台的稳定 C ABI 与 Qt 接入基线。

执行体发现契约缺失时，必须先补充文档和测试，不得自行发明兼容行为或虚假声明 Qt Native 完成。

## 2. 起始基线

- `wp/16-engine-state-machine` 已合并 `dev-docs/003`（Web Pro 功能等价）全部 51 个 WP。
- 共享核心（`cheetah-media-types`、`bitstream`、`container-*`、`timeline`、`engine`）已在 wasm32 和 native 通过。
- Web bindings（`cheetah-media-web-bindings`）已暴露 `WebEngine` 控制面；native 需要等价的 C ABI 层。
- 真实端到端回放、硬件-bound 性能门禁和平台发布演练受外部媒体端点、目标平台 SDK 和签名凭证限制，不进入 Qt 基线阶段的虚假完成声明。

## 3. 三仓边界

| 仓库 | 路径 | 职责 |
| --- | --- | --- |
| 共享核心 | `../cheetah-media-core-rs` | 纯媒体类型、bitstream、容器、HLS、时间线、pipeline 模型、ABI、fixture |
| 播放引擎 | 当前仓库 | engine、Web bindings/runtime/SDK/components、native C ABI、Qt wrapper、codec packs |
| 媒体服务 | `../cheetah-media-server-rs` | 通过 `cheetah-codec` 兼容门面消费共享核心；保留服务端会话和协议驱动 |

依赖只能从产品仓库指向共享核心。共享核心不得依赖 Qt、浏览器、UI、服务端 session、网络 driver 或任一产品仓库。

## 4. 执行规则

1. 严格按 001 Phase 5 依赖顺序实施；阻塞项未关闭时不得提前完成下游任务。
2. 每个任务编号对应一个可独立评审、测试和回滚的 PR 工作包；不得把多个阶段压进不可审查的大 PR。
3. 完成 `[ ]` 后改为 `[x]`，并追加仓库、commit/tag、测试命令、结果和证据路径。
4. 公共 C ABI 和 contract test 先于 Qt widget/QML adapter。三仓变更按 core tag → server facade → engine 的顺序落地。
5. 禁止 `todo!()`、`unimplemented!()`、空 provider、吞错、C ABI 假成功。暂不支持能力返回稳定 `Unsupported`。
6. C ABI 字符串、回调、handle 和生命周期必须显式声明，禁止跨边界传播裸指针所有权。
7. 任何改变 crate 图、ABI、公开 TypeScript API、错误语义、复制预算或 001 边界的实现，先修改 001/004 并经评审。
8. 外部代码和 fixture 必须登记来源 commit、许可证、修改和脱敏方式。
9. CI 不得依赖未提交的本地 path。跨仓开发可临时使用 `[patch]`，提交前必须改为不可变 tag/revision。
10. 性能结论必须有原始数据、环境、命令和对照；不得在无合法可复现实验时宣称优于第三方产品。

## 5. Phase 5 工作包索引

| WP | 文档 | 阶段交付 | 依赖 |
| --- | --- | --- | --- |
| 52 | 本文档 | Phase 5 执行计划与 C ABI 基线 | 003 全部 WP |
| 53 | `004/53_c_abi_bindings.md` | `cheetah-media-c-bindings` crate 骨架：Cargo、header 生成、player handle 创建/释放 | 52 |
| 54 | `004/54_c_abi_control_surface.md` | C ABI 控制面：config、load、play、pause、stop、destroy、async 回调 | 53 |
| 55 | `004/55_qt_widget_surface.md` | Qt QWidget 接入与窗口生命周期 | 54 |
| 56 | `004/56_qt_qml_surface.md` | Qt QML / Qt Quick surface 接入 | 55 |
| 57 | `004/57_native_transport.md` | 原生 HTTP/WS/TCP transport adapter（tokio） | 54、40 |
| 58 | `004/58_native_decoder_backends.md` | 平台硬解探测与回退：Media Foundation / VideoToolbox / VA-API / Vulkan Video | 57 |
| 59 | `004/59_native_renderer.md` | OpenGL / Vulkan / Metal / D3D11 renderer 与零拷贝 surface | 55/56、58 |
| 60 | `004/60_native_audio_sink.md` | 平台 audio sink 与 A/V sync | 59 |
| 61 | `004/61_native_capability_and_diagnostics.md` | 原生能力协商、diagnostics 与生命周期 soak | 60 |

> 未来工作包（Android / iOS / 鸿蒙 / 双向引擎）将在 Phase 5 完成后另建 `dev-docs/005` 计划。

## 5.1 工作包状态

- [x] WP-53 `cheetah-media-c-bindings` crate 骨架（已合并到 `wp/16-engine-state-machine`）
- [x] WP-54 C ABI 控制面：config、load、play、pause、stop、destroy、async 回调（PR #61）
- [x] WP-55 Qt QWidget 接入与窗口生命周期（PR #62）
- [x] WP-56 Qt QML / Qt Quick surface 接入（PR #63）
- [~] WP-57 原生 HTTP/WS/TCP transport adapter（tokio）
- [ ] WP-58 平台硬解探测与回退
- [ ] WP-59 原生 renderer 与零拷贝 surface
- [ ] WP-60 平台 audio sink 与 A/V sync
- [ ] WP-61 原生能力协商、diagnostics 与生命周期 soak

## 6. 全局完成定义

- [ ] 53–61 所有适用项完成，不适用项经产品评审记录原因。
- [ ] 每项有自动化测试、真实设备记录或明确人工验收步骤。
- [ ] 平台限制和硬件能力作为 capability 公示，不冒充标准支持。
- [ ] 新增 native transport/codec 路径在共享核心中通过 contract suite，不重复 parser 逻辑。
- [ ] 性能、内存和长稳测试覆盖启用 native 功能后的增量成本。

## 7. 标准验证命令

与 `002_vibe_coding_plan` / `003_web_pro_feature_parity` 一致，并增加 Qt 相关检查：

```bash
# 共享核心 / engine / C ABI
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build --workspace --target wasm32-unknown-unknown --no-default-features
cargo deny check

# Qt 示例构建（可选，目标平台可用时）
cmake -S apps/qt-demo -B build/qt-demo -DCMAKE_PREFIX_PATH=$QT_PREFIX
cmake --build build/qt-demo

# Web 引擎（保持兼容性）
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm exec playwright test
```

各任务只运行与其仓库和 feature 相关的子集，但完成证据必须记录实际完整命令。发布候选必须运行全集。

## 8. 阶段闸门

| 闸门 | 必须满足后才能进入下一阶段 |
| --- | --- |
| G9 C ABI 稳定闸门 | cbindgen header 与 Rust FFI test 一致；handle 生命周期可重复 create/destroy；跨边界 panic 不传播 |
| G10 Qt 表面闸门 | QWidget 与 QML 示例可在 Linux 桌面运行；resize / hide / DPI / device lost 映射到统一事件 |
| G11 原生播放闸门 | native transport + demux + decoder + renderer + audio sink 覆盖至少一个参考 fixture |
| G12 原生兼容闸门 | 能力探测正确识别硬解/软解/不支持，fallback 不黑屏、不泄漏 |
