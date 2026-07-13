# Cheetah 下一代跨平台实时音视频引擎设计

## 1. 文档定位

本目录定义 Rust 驱动的下一代跨平台实时音视频引擎目标架构、Web 首版产品边界和长期演进路线。它是实现、评审和验收的规范性输入，不是概念草图。

引擎最终支持 Web、Qt Native、Android、iOS 和鸿蒙，并预留采集、编码、推流和双向通信能力。首个生产版本优先交付 Web 安防实时预览 SDK、参考播放器和多画面组件。

## 2. 已冻结的设计结论

1. Rust 是媒体模型、容器解析、时间线、缓存和管线状态机的唯一核心实现；TypeScript 只负责 Web 平台适配和公共 SDK。
2. 新建独立 `cheetah-media-core-rs` 仓库作为共享媒体核心唯一源码，`cheetah-media-engine` 与 `cheetah-media-server-rs` 通过固定版本依赖使用。
3. Web v1 支持 HTTP/WS-FLV、HLS/LL-HLS、HTTP/WS-fMP4，以及 H.264、H.265、AAC、G.711A/U 和 MP3。
4. 播放后端按输入、codec、设备能力和渲染需求动态规划，不使用全局固定的 WebCodecs/MSE 顺序。
5. 软解回退顺序为 WASM Threads+SIMD、WASM SIMD、WASM baseline；COOP/COEP 不可用时自动禁用共享多线程。
6. 系统第一性能原则是减少复制、分配和跨线程对象传输；热路径禁止 JSON 和 Base64 媒体负载。
7. Web v1 提供无框架 TypeScript SDK、Web Components 单窗播放器以及 1/4/9/16 宫格参考 UI。
8. 多画面由全局资源预算器管理主子码流、硬解实例、CPU/GPU 压力和可见性，不由多个完全独立播放器争抢资源。
9. v1 录制为压缩帧无损重封装，支持 MP4/fMP4 和 FLV 流式写出，不包含 UI、覆盖层和水印。
10. 不兼容 Jessibuca JavaScript API；以功能等价矩阵分阶段覆盖 Jessibuca Pro 能力。
11. 核心采用 MIT OR Apache-2.0；FFmpeg 等 LGPL 组件以独立、可替换、延迟加载的 codec pack 分发。
12. Web 之后按 Qt → Android → iOS → 鸿蒙验证原生平台；最终建设包含采集、编码、推流的双向引擎。

## 3. 规范优先级

发生冲突时依次遵循：

1. 本目录中明确写为“必须”的内部契约；
2. WebCodecs、MSE、WebGPU、WebAssembly 等公开标准；
3. `cheetah-media-core-rs` 的稳定媒体模型和 ABI；
4. 真实浏览器、设备和媒体流 fixture 的兼容结果；
5. `cheetah-media-server-rs` 的协议兼容经验；
6. 参考实现和第三方库的默认行为。

标准原文优先于参考实现。不得为了复用服务端代码把监听、会话编排或具体 runtime 带入浏览器核心。

## 4. 文档索引

| 文档 | 内容 |
| --- | --- |
| [01_goals_and_slo.md](01_goals_and_slo.md) | 产品目标、Web v1 范围、非目标和性能 SLO |
| [02_architecture_and_repositories.md](02_architecture_and_repositories.md) | 总体架构、仓库边界、分层和平台抽象 |
| [03_media_model_memory_and_abi.md](03_media_model_memory_and_abi.md) | 媒体模型、所有权、共享内存、ABI 和复制预算 |
| [04_web_runtime_and_fallback.md](04_web_runtime_and_fallback.md) | Web runtime、网络输入、能力探测和回退状态机 |
| [05_protocol_codec_audio_and_rendering.md](05_protocol_codec_audio_and_rendering.md) | 协议、容器、编解码、音频和渲染设计 |
| [06_web_sdk_ui_multiview_and_recording.md](06_web_sdk_ui_multiview_and_recording.md) | Web SDK、UI、多画面、截图与录制 |
| [07_jessibuca_pro_feature_parity.md](07_jessibuca_pro_feature_parity.md) | Jessibuca Pro 功能等价矩阵和阶段归属 |
| [08_native_platforms_and_bidirectional.md](08_native_platforms_and_bidirectional.md) | Qt、Android、iOS、鸿蒙和双向能力 |
| [09_security_licensing_observability.md](09_security_licensing_observability.md) | 安全、许可证、部署约束和可观测性 |
| [10_testing_and_acceptance.md](10_testing_and_acceptance.md) | 测试分层、设备矩阵、性能和发布门禁 |
| [11_implementation_roadmap.md](11_implementation_roadmap.md) | 阶段顺序、交付物和完成定义 |
| [12_reference_baseline.md](12_reference_baseline.md) | 标准、参考项目和版本冻结规则 |

## 5. 术语

- **共享媒体核心**：独立仓库中的媒体类型、bitstream、容器、时间线和平台无关状态机。
- **Platform Backend**：网络、解码、渲染、音频、时钟、录制或设备生命周期的平台实现。
- **Pipeline Plan**：针对一组输入轨道选择传输、解封装、解码、同步和渲染后端的运行计划。
- **PacketHandle**：指向压缩媒体访问单元的稳定引用句柄。
- **FrameHandle**：指向解码帧或平台媒体资源的稳定引用句柄。
- **显式复制**：由本项目代码执行并可计数的 payload 内存复制。
- **浏览器内部复制**：由 WebCodecs、MSE、WebGPU 或浏览器进程边界产生、项目无法直接控制的复制。
- **主码流/子码流**：同一监控通道的高质量和低资源输入源。
- **实时追赶**：过载或后台恢复时跳过过期数据，从最新可随机访问帧继续播放。
- **codec pack**：独立下载、独立许可并可替换的 WASM 编解码模块。

## 6. 设计基线

- Rust 核心遵循 Sans-I/O、显式输入输出、注入时钟和有界资源原则。
- Web 优化路径允许使用 SharedArrayBuffer、Worker、WASM threads、SIMD、WebCodecs 和 WebGPU，但必须保留兼容回退。
- Rust struct 不是跨模块 ABI；跨 WASM 模块、C ABI 和语言绑定只使用稳定描述符、数值枚举和不透明句柄。
- 所有队列、缓存、录制缓冲、重试、探测和并发都有上界。
- 性能结果必须绑定硬件、系统、浏览器、码流、commit 和配置。
- 默认不发送外部遥测，只提供本地统计、诊断导出和业务可选的观测接口。
- 工具链在实施 Phase 0 固定到实际发布且验证通过的 stable 版本，不采用无法下载的未来版本号。
