# Cheetah Media Engine 移动平台与双向引擎执行计划

## 1. 文档定位

本文件把 [`001` Phase 6 与 Phase 7](../001_next_generation_media_engine/11_implementation_roadmap.md)（Android / iOS / 鸿蒙 / 双向实时引擎）转换为可执行的工作包序列。它从 `dev-docs/004_qt_native_baseline.md` 的完成状态继续，目标是在稳定 C ABI、Qt Native 和共享核心基础上，建立移动端播放与双向实时能力的平台基线。

执行体发现契约缺失时，必须先补充文档和测试，不得自行发明兼容行为或虚假声明移动平台/双向引擎完成。

## 2. 起始基线

- `main` 已合并 `wp/16-engine-state-machine` 和 Phase 5（WP-53~61）全部内容：稳定 C ABI、Qt QWidget/QML、native transport、decoder、renderer、audio sink、能力协商与 diagnostics。
- 共享核心（`cheetah-media-types`、`bitstream`、`container-*`、`timeline`、`engine`、`abi`、`backend-api`）已在 wasm32、x86_64 native 和无头 CI 通过。
- `NativePlayer` 已能把 `ByteSource → Decoder → Renderer/AudioSink` 串起来并通过 lifecycle soak。
- 真实移动端运行依赖 Android NDK/SDK、iOS/macOS Xcode 工具链、鸿蒙 DevEco/NAPI SDK，以及真机/模拟器；本阶段不进入不具备可复现运行环境的虚假完成声明。

## 3. 三仓边界

| 仓库 | 路径 | 职责 |
| --- | --- | --- |
| 共享核心 | `../cheetah-media-core-rs` | 纯媒体类型、bitstream、容器、HLS、时间线、pipeline 模型、ABI、fixture |
| 播放引擎 | 当前仓库 | engine、Web bindings、native C ABI/Qt、移动端 wrapper、codec packs、双向引擎 |
| 媒体服务 | `../cheetah-media-server-rs` | 通过 `cheetah-codec` 兼容门面消费共享核心；保留服务端会话和协议驱动 |

依赖只能从产品仓库指向共享核心。共享核心不得依赖 Qt、浏览器、移动端 SDK、UI、服务端 session、网络 driver 或任一产品仓库。

## 4. 执行规则

1. 严格按 001 Phase 6 → Phase 7 依赖顺序实施；阻塞项未关闭时不得提前完成下游任务。
2. 每个任务编号对应一个可独立评审、测试和回滚的 PR 工作包；不得把多个阶段压进不可审查的大 PR。
3. 完成 `[ ]` 后改为 `[x]`，并追加仓库、commit/tag、测试命令、结果和证据路径。
4. 移动端公共契约和 contract test 先于 platform adapter。三仓变更按 core tag → server facade → engine 的顺序落地。
5. 禁止 `todo!()`、`unimplemented!()`、空 provider、吞错、假成功。暂不支持能力返回稳定 `Unsupported`。
6. 移动端字符串、回调、handle、生命周期必须显式声明，禁止跨边界传播裸指针所有权。
7. 任何改变 crate 图、ABI、公开 API、错误语义、复制预算或 001 边界的实现，先修改 001/005 并经评审。
8. 外部代码和 fixture 必须登记来源 commit、许可证、修改和脱敏方式。
9. CI 不得依赖未提交的本地 path。跨仓开发可临时使用 `[patch]`，提交前必须改为不可变 tag/revision。
10. 性能结论必须有原始数据、环境、命令和对照；不得在无合法可复现实验时宣称优于第三方产品。

## 5. Phase 6/7 工作包索引

| WP | 文档 | 阶段交付 | 依赖 |
| --- | --- | --- | --- |
| 62 | 本文档 | Phase 6/7 执行计划与移动/双向基线 | 004 全部 WP |
| 63 | `005/63_android_baseline.md` | Android 播放骨架：crate、`MediaCodec` probe、JNI 入口、生命周期 | 62 |
| 64 | `005/64_android_surface_audio.md` | Android `Surface` 渲染与 `AudioTrack` sink | 63 |
| 65 | `005/65_android_smoke.md` | Android 模拟器/真机 smoke 与 lifecycle soak | 64 |
| 66 | `005/66_ios_baseline.md` | iOS 播放骨架：Swift/C 桥接、`VideoToolbox` probe、生命周期 | 62 |
| 67 | `005/67_ios_surface_audio.md` | iOS `Metal`/`CAMetalLayer` 渲染与 `AudioUnit` sink | 66 |
| 68 | `005/68_harmonyos_baseline.md` | 鸿蒙 NAPI/ArkTS wrapper、平台 codec/surface/audio 生命周期 | 62 |
| 69 | `005/69_mobile_capability_matrix.md` | 跨平台 capability 统一、探测注册表与真机兼容矩阵 | 65、67、68 |
| 70 | `005/70_bidirectional_engine.md` | 双向引擎抽象：`CaptureSource`、`Processor`、`Encoder`、`PublisherBackend` | 63 |
| 71 | `005/71_capture_sources.md` | 麦克风/摄像头/屏幕采集与平台权限模型 | 70 |
| 72 | `005/72_encoders.md` | H.264/H.265/Opus/AAC/G.711 平台编码器能力 | 71 |
| 73 | `005/73_publish_backends.md` | WebRTC/RTMP 发布路径、拥塞反馈与动态码率 | 72 |
| 74 | `005/74_bidirectional_integration.md` | 播放与发布统一资源预算、A/V sync、diagnostics 与端到端 soak | 71、73 |

> 后续 WP 的范围、编号和验收标准在进入该 WP 前补充为 `005/XX_name.md`，避免一次性编造无法验证的细节。

## 5.1 工作包状态

- [x] WP-62 Phase 6/7 执行计划与移动/双向基线（PR #69）
- [x] WP-63 Android 播放骨架（PR #70）
- [~] WP-64 Android Surface 渲染与 AudioTrack sink（跳过，无 Android SDK）
- [~] WP-65 Android 模拟器/真机 smoke（跳过）
- [~] WP-66 iOS 播放骨架（跳过，无 macOS/Xcode）
- [~] WP-67 iOS Metal 渲染与 AudioUnit sink（跳过）
- [~] WP-68 鸿蒙 NAPI/ArkTS wrapper（跳过，无 HarmonyOS SDK）
- [~] WP-69 跨平台 capability 统一与真机兼容矩阵（跳过）
- [x] WP-70 双向引擎抽象（PR #71）
- [x] WP-71 采集源与权限模型（PR #72）
- [x] WP-72 平台编码器能力（PR #73）
- [~] WP-73 发布路径与拥塞控制（本 PR）
- [ ] WP-74 双向引擎集成与端到端 soak

## 6. 全局完成定义

- [ ] 62–74 所有适用项完成，不适用项经产品评审记录原因。
- [ ] 每项有自动化测试、真机/模拟器记录或明确人工验收步骤。
- [ ] 平台限制和硬件能力作为 capability 公示，不冒充标准支持。
- [ ] 新增移动端/双向路径在共享核心中通过 contract suite，不重复 parser 逻辑。
- [ ] 性能、内存和长稳测试覆盖启用移动端/双向功能后的增量成本。

## 7. 标准验证命令

与 `002_vibe_coding_plan` / `003_web_pro_feature_parity` / `004_qt_native_baseline` 一致，并增加移动端/双向相关检查：

```bash
# 共享核心 / engine / C ABI / Qt / Web
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo deny check

# 移动端 cross-check（目标平台 SDK/NDK 可用时）
# cargo build -p cheetah-media-android --target aarch64-linux-android
cargo build -p cheetah-media-engine --features native

# Web 引擎（保持兼容性）
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
```

各任务只运行与其仓库和 feature 相关的子集，但完成证据必须记录实际完整命令。发布候选必须运行全集。

## 8. 阶段闸门

| 闸门 | 必须满足后才能进入下一阶段 |
| --- | --- |
| G13 移动基线闸门 | 至少一个移动平台（Android/iOS/鸿蒙）在模拟器或真机上能完成 load → play → render → stop → destroy，生命周期无泄漏 |
| G14 移动兼容闸门 | 硬解/软解/不支持能力如实通过 capability 探测暴露，fallback 不黑屏、不崩溃 |
| G15 双向抽象闸门 | `CaptureSource`/`Processor`/`Encoder`/`PublisherBackend` trait 与播放端共享 timeline、budget 和 diagnostics |
| G16 端到端双向闸门 | 采集 → 编码 → 发布 → 回环播放在实验室网络下通过故障注入和 15 分钟 soak |
