# 01. 目标、范围与 SLO

## 1. 产品目标

建设 Rust 驱动的跨平台实时音视频引擎，使 Web、Qt、Android、iOS 和鸿蒙共享同一媒体模型、协议解析、时间线、缓存和管线控制能力。平台新增不应迫使媒体核心、公共 API 语义或测试语料重写。

首个生产版本必须完成：

- 面向安防实时预览的 Web 播放 SDK；
- H.264/H.265 硬解、MSE 和 WASM 软解的自动选择与运行时回退；
- HTTP/WS-FLV、HLS/LL-HLS、HTTP/WS-fMP4 输入；
- AAC、G.711A/U、MP3 播放和音画同步；
- 单窗与 1/4/9/16 宫格参考 UI；
- 主子码流自动切换、实时追赶、截图、原码流录制和性能面板；
- 可复现的桌面密度、延迟和 24 小时稳定性验收。

## 2. 产品原则

优先级从高到低为：

1. 延迟、队列和资源始终有界；
2. 避免应用可控的 payload 复制和临时分配；
3. 慢流、坏流或慢消费者不能拖垮其他播放实例；
4. 硬件能力不可假定，必须探测、试运行并可熔断；
5. Web 特性不能污染跨平台媒体核心；
6. 功能等价和性能结论必须有明确测试口径。

## 3. Web v1 功能范围

### 3.1 输入与编码

- HTTP-FLV、WebSocket-FLV；
- HLS 与 LL-HLS，支持 MPEG-TS 和 fMP4 segment；
- HTTP-fMP4、WebSocket-fMP4；
- H.264、H.265；
- AAC、G.711 A-law、G.711 μ-law、MP3。

### 3.2 播放与交互

- load、play、pause、stop、destroy；
- 静音、音量、全屏、容器内全屏；
- contain、cover、fill 显示模式；
- 0/90/180/270 度旋转和水平/垂直镜像；
- 当前画面截图；
- MP4/fMP4、FLV 原码流录制；
- 后端切换、延迟、队列、丢帧和复制统计；
- 页面后台暂停视频解码，返回前台后追赶直播点。

### 3.3 多画面

- 1、4、9、16 宫格；
- 每个通道配置主、子输入源；
- 宫格默认子码流，焦点或全屏切换主码流；
- 全局硬解实例、CPU、GPU、内存和带宽预算；
- 能力不足时按优先级降帧、切子码流或暂停不可见窗口。

## 4. 非目标

以下能力不属于 Web v1，但必须进入后续功能矩阵：

- Jessibuca JavaScript API 兼容；
- WebRTC、WebTransport 和厂商私有 RTC；
- MPEG-PS、裸 H.264/H.265、通用本地文件播放；
- 行业录像回放、倍速、逐帧和逐关键帧；
- PTZ、电子放大、复杂水印、加密流和 AI；
- 把 UI 或覆盖层烧录进 v1 录像；
- Qt、Android、iOS、鸿蒙的首版可执行产品；
- 通用采集、视频编码和推流。

非目标必须通过 capability 返回 `Unsupported`，不得以空实现或虚假成功占位。

## 5. 性能口径

性能测试必须记录：

- CPU、GPU、内存和硬件解码器；
- 操作系统、浏览器及版本；
- 视频 codec、profile、level、分辨率、帧率、码率、GOP 和 B 帧；
- 音频 codec、采样率和声道；
- 协议、服务端缓存行为、网络 RTT、抖动和丢包；
- 是否启用 COOP/COEP、SharedArrayBuffer、Worker、SIMD 和 WebGPU；
- commit、构建 profile、codec pack 版本和播放器配置。

没有相同环境的合法 Jessibuca Pro 基线前，只发布绝对指标，不发布“全部场景均更快”的比较结论。

## 6. Web v1 SLO

受控局域网 RTT 不高于 10 ms、关键帧间隔不高于 1 秒且服务端提供起播关键帧时：

| 指标 | 目标 |
| --- | --- |
| HTTP/WS-FLV、HTTP/WS-fMP4 首帧 | P95 ≤ 800 ms |
| HTTP/WS-FLV、HTTP/WS-fMP4 稳态直播延迟 | P95 ≤ 600 ms |
| LL-HLS 稳态直播延迟 | P95 ≤ 1.5 s |
| 音画时间差绝对值 | P95 ≤ 50 ms |
| 支持容量内播放器主动丢帧 | < 0.5% |
| 桌面 H.265 1080p25 硬解密度 | 稳定 9 路 |
| 桌面 H.265 720p15 硬解密度 | 稳定 16 路 |
| 连续稳定性 | 24 小时无崩溃、无无界队列 |
| 24 小时延迟漂移 | ≤ 100 ms |
| 预热后 JS/WASM 内存增长 | ≤ 5% |

普通 HLS 的延迟受 segment target duration 约束，单独报告首帧和稳态延迟，不套用 LL-HLS 指标。

## 7. 兼容目标

- Windows 和 macOS 是 Web v1 性能硬门槛平台；
- Chromium/Edge 为一级性能目标；
- Safari/macOS 进入桌面性能与兼容矩阵；
- Firefox、Android 和 iOS 首版至少通过单路兼容验收；
- 浏览器版本窗口为当前稳定版和前两个稳定大版本；
- H.265 能力必须按实际硬件、操作系统组件和浏览器试解结果判定。

## 8. 长期目标

- 按 Jessibuca Pro 功能矩阵逐项实现功能等价并形成可追踪验收状态；
- 按 Qt → Android → iOS → 鸿蒙复用 Rust 核心；
- 增加 CaptureSource、EncoderBackend 和 PublisherBackend；
- 最终形成播放、采集、编码、推流和双向通信统一引擎。
