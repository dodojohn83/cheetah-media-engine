# 02. 系统架构与仓库边界

## 1. 总体结构

```text
                        cheetah-media-core-rs
              media types / bitstream / containers
              timeline / jitter / cache / pipeline core
                     ▲                       ▲
                     │ versioned crates      │ versioned crates
                     │                       │
        cheetah-media-engine          cheetah-media-server-rs
   engine orchestration / backends    protocol server / distribution
       │            │           │
       ▼            ▼           ▼
      Web          Native     Future publishers
 TS + WASM      Qt/Android/
 Workers/UI      iOS/Harmony
```

## 2. 仓库职责

### 2.1 `cheetah-media-core-rs`

共享核心是唯一权威实现，负责：

- Track、Packet、Frame、时间基和 side data；
- H.26x、AAC、G.711 等 bitstream 工具；
- FLV、MPEG-TS、ISOBMFF/fMP4/MP4；
- HLS playlist、LL-HLS 状态和拉流 pacing；
- 参数集缓存、时间戳归一化、抖动和 A/V 同步基础；
- 有界 GOP、Packet/Frame 生命周期和平台无关 pipeline planner；
- 稳定 ABI 描述符、fixture、property test 和 fuzz corpus。

共享核心禁止依赖浏览器 DOM、Tokio socket、Qt、JNI、Objective-C、鸿蒙 SDK、具体 GPU API或媒体服务器 engine。

### 2.2 `cheetah-media-engine`

本仓库负责：

- 播放、录制和未来发布管线编排；
- Transport、Decoder、Renderer、AudioSink、Clock 等平台后端接口；
- Web WASM binding、Worker runtime、TypeScript SDK；
- Web Components、参考应用、多画面预算器；
- 原生 C ABI 和各平台绑定；
- 浏览器、设备、性能和端到端测试。

### 2.3 `cheetah-media-server-rs`

媒体服务器继续负责：

- RTMP、RTSP、HTTP-FLV、HLS、WebRTC 等服务端监听和会话；
- 发布订阅、流路由、录制和服务端分发；
- 协议 driver/module、socket、鉴权和服务端资源治理。

可迁移进共享核心的是媒体和容器基础能力。服务端请求解析、监听、连接会话和 engine 编排不得迁入播放器。

## 3. 依赖方向

从上到下分为：

1. **product/application**：参考 UI、业务适配和应用生命周期；
2. **public SDK**：播放器、电视墙、录制和事件 API；
3. **engine orchestration**：pipeline、资源预算、状态机和错误恢复；
4. **platform backends**：网络、解码、渲染、音频、存储和时钟；
5. **shared media core**：媒体模型、容器、bitstream 和 Sans-I/O 状态机。

依赖只能向下或指向下层定义的接口。禁止：

- shared core 依赖 TypeScript、DOM 或具体 runtime；
- Web SDK 复制一套容器和时间线实现；
- 平台后端反向决定公共 API 语义；
- UI 直接操作 WASM 内存或 decoder 对象；
- media server 与 engine 使用语义不同的 AVFrame 模型。

## 4. 平台端口

核心端口至少包括：

```text
TransportSource  -> compressed byte stream
VideoDecoder     -> decoded video frame/resource
AudioDecoder     -> decoded PCM/resource
VideoRenderer    -> present video frame
AudioSink        -> schedule PCM
Clock            -> monotonic/media/wall clock mapping
RecorderSink     -> bounded streaming output
CapabilityProbe  -> static probe + active validation
DiagnosticsSink  -> local metrics and bounded diagnostic events
```

为长期双向能力预留：

```text
CaptureSource
VideoEncoder
AudioEncoder
PublisherBackend
DuplexTransport
```

端口公共类型必须 runtime-neutral，不能暴露 Tokio channel、WebCodecs 对象、JNI handle 或平台线程类型。

## 5. 播放器状态

播放器使用显式状态机：

```text
Idle -> Loading -> Probing -> Buffering -> Playing
                    │             │          │
                    └-> Fallback <-┴----------┘
Playing <-> Paused
any state -> Stopping -> Idle
any state -> Failed
any terminal state -> Destroyed
```

- 每次 load 生成新的 generation，旧 generation 的异步回调必须丢弃；
- stop 和 destroy 必须向 Transport、Worker、Decoder、Renderer、AudioSink、Recorder 传播取消；
- fallback 是同一次 load 的后端重建，不创建第二条不受控网络管线；
- Failed 必须携带稳定错误码、失败阶段和最后一个可诊断原因。

## 6. 并发模型

### 6.1 Web

- 主线程只负责 API、DOM 和轻量协调；
- I/O、解封装、时间线和大部分调度位于 Worker；
- 隔离环境使用共享 WASM 内存和固定 Worker 池；
- 非隔离环境使用单 Worker 或可转移 ArrayBuffer；
- 不为每一帧创建 Promise、闭包或跨线程普通对象图。

### 6.2 Native

- pipeline 由固定执行器或线程池驱动；
- decoder callback 只写入有界队列；
- 平台 UI 线程只提交显示和交互操作；
- 慢 renderer、recorder 或业务 callback 不能阻塞输入和其他播放器。

## 7. 发布与版本关系

- 共享核心采用语义化版本和固定 Git tag；
- engine 和 server 在 lockfile 中固定共享核心 revision；
- 共享 ABI、Frame/Track 语义和容器行为变更必须同步两仓 contract tests；
- 破坏性 ABI 使用新 major，不复用已经发布的 enum 数值或字段含义；
- 初期不要求发布 crates.io，可在稳定后迁移到公共或私有 registry。

## 8. 配置与能力

配置只表达偏好和限制，不能伪造平台能力。至少支持：

- decoder/renderer policy；
- target/max latency；
- 网络、GOP、Packet、Frame 和录制缓冲上限；
- Worker 数量与总内存上限；
- 主子码流切换策略；
- 自动 fallback、重试和诊断级别；
- 每类后端的 allow/deny 策略。

实际能力由 CapabilityProbe 和试运行结果产生，并通过公共 stats/events 暴露。
