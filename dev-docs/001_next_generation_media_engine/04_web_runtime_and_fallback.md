# 04. Web Runtime 与回退管线

## 1. 总体数据流

```text
Fetch / WebSocket
        │
        ▼
I/O Worker ──► bounded byte ring
        │
        ▼
Rust/WASM demux + timeline + GOP cache
        │
        ▼
Pipeline Planner
   │                    │                    │
   ▼                    ▼                    ▼
WebCodecs              MSE             WASM codec pack
   │                    │                    │
   ▼                    ▼                    ▼
VideoFrame/AudioData  fMP4 + <video>       YUV/PCM
   │                                         │
   └──────────────► A/V scheduler ◄──────────┘
                        │
                        ▼
              WebGPU/WebGL + AudioWorklet
```

主线程不解析媒体包。主线程只负责 SDK 调用、DOM/UI、MediaSourceHandle 或渲染 surface 接入，以及不能放入 Worker 的浏览器 API。

## 2. 网络输入

### 2.1 Fetch

- HTTP-FLV、HTTP-fMP4 和 HLS 使用 Fetch；
- streaming body 必须增量消费，禁止先完整下载；
- 支持 AbortSignal、deadline、credentials、headers、redirect policy 和 CORS 错误分类；
- HLS playlist、init segment、part 和 segment 分别统计请求延迟与错误；
- Range 用于 MP4/点播阶段，v1 直播不依赖无界随机读取。

### 2.2 WebSocket

- `binaryType` 使用 `arraybuffer`；
- 浏览器不能设置任意 WebSocket header，鉴权只允许 cookie、query token 或 Sec-WebSocket-Protocol 约定；
- message 必须有最大尺寸；
- 收包速度超过消费能力时关闭或重连，不能在 JS 堆积无界 message；
- 重连使用次数、总 deadline、指数退避和 jitter 上限。

## 3. Worker 模式

### 3.1 隔离优化档

满足 cross-origin isolation 时：

- 共享 WebAssembly.Memory；
- I/O/demux Worker 和固定 codec Worker 池；
- Atomics 驱动的有界 descriptor ring；
- WASM threads + SIMD codec pack；
- 可用时在 DedicatedWorker 构造 MediaSource，并通过 MediaSourceHandle 连接主线程 video。

### 3.2 非隔离兼容档

- 不使用 SharedArrayBuffer 和 WASM threads；
- 优先让网络、demux 和软解位于同一 Worker，避免复制；
- 跨线程数据使用 ArrayBuffer transfer；
- SIMD 仍按 WebAssembly.validate/实际实例化探测；
- 多画面密度和软解性能不承诺达到隔离优化档。

## 4. 能力探测

每次 load 按 source tracks 构造能力矩阵：

```text
browser API presence
       ↓
static config/type support
       ↓
configure/create backend
       ↓
parameter sets + first random-access unit probe
       ↓
first decoded/presented frame within deadline
       ↓
runtime health monitoring
```

静态支持结果不能直接等同于可用。以下任一情况可判定当前候选失败：

- configure/addSourceBuffer 抛出异常；
- 参数集或 sample entry 被拒绝；
- 输入关键帧后在 deadline 内无输出；
- 连续 decode error 超过阈值；
- decodeQueueSize、MSE append queue 或 buffered duration 持续超限；
- 实际吞吐低于媒体帧率并持续产生延迟；
- renderer 无法消费该帧格式；
- 动态分辨率或 codec change 后无法重新配置。

## 5. 动态管线规划

候选评分考虑：

- video/audio codec、profile、level、bit depth 和分辨率；
- FLV、TS、fMP4 等输入封装；
- 是否需要截图、AI、像素访问、旋转或自定义 GPU 处理；
- 低延迟、后台播放和多画面资源预算；
- WebCodecs、MSE、WebGPU、WebGL、AudioDecoder 实际能力；
- G.711 等 MSE 不支持的音频组合；
- 用户显式 allow/deny/force 策略。

默认策略：

1. 需要低延迟、逐帧控制或 GPU 处理时优先 WebCodecs；
2. 适合 `<video>` 且 MSE 对完整轨道组合更稳定时可以直接选择 MSE；
3. 硬解路径不可用或运行失败时选择 WASM Threads+SIMD；
4. 无线程条件时选择 WASM SIMD；
5. SIMD 不可用时选择 WASM baseline；
6. 每条音频轨独立选择 MSE、WebCodecs 或 AudioWorklet 软解路径。

这不是固定全局序列。明确不兼容 MSE 的输入不得为了满足顺序而实例化 MSE。

## 6. 回退状态机

```text
CandidateReady
      │ runtime failure
      ▼
Freeze output / keep last frame
      │
      ▼
Stop and drain failed backend
      │
      ▼
Select next viable candidate
      │
      ▼
Feed cached config + latest GOP from keyframe
      │
      ▼
First-frame deadline
  │ success          │ failure
  ▼                  ▼
Playing         next candidate / Failed
```

- fallback 保持同一 Transport 和 demux generation；
- 参数集、最新有界 GOP 和音频同步锚点由 Rust 核心持有；
- 失败 backend 必须完全释放后再计入可用硬解实例；
- 成功降级后默认不在本次播放中自动升级，避免震荡；
- source reload、设备恢复或用户显式 retry 才重新评估高优先级后端；
- 每次切换发出包含 from、to、reason、duration 和是否丢帧的事件。

## 7. MSE 特殊规则

- MSE 输入必须是浏览器接受的 byte stream format，FLV/TS 输入由 Rust 重封装为 fMP4；
- 使用准确的 avc1/avc3/hvc1/hev1 codec string 和初始化片段；
- G.711 组合采用 video-only MSE 加 AudioWorklet，不把 G.711 声称为 MSE 支持；
- appendBuffer 串行化，任何时刻每个 SourceBuffer 最多一个 updating 操作；
- 定期移除播放点之后不需要的历史 buffer；
- QuotaExceededError 先收缩 buffer，再失败回退；
- timestampOffset、appendWindow 和 discontinuity 必须由 timeline epoch 管理；
- 后台恢复时移除过期范围并 seek 到直播点。

## 8. 实时控制

默认 `latencyMode=realtime`：

- target latency 用于正常调度；
- max latency 是硬边界；
- 超出 max latency 时跳过过期非关键帧并追到最新关键帧；
- 轻微网络抖动只使用短小有界 jitter buffer；
- 不通过持续扩大 MSE、Packet 或 Frame 队列换取表面连续；
- 丢帧原因区分 late、queue_full、decoder_overload、visibility、quality_switch 和 corruption。

## 9. 页面生命周期

- `visibilitychange` 后默认暂停视频 decode/render；
- 网络、demux 和最新 GOP 保活，受浏览器节流时允许有界重连；
- 音频默认随视频暂停，业务可显式指定允许后台音频；
- 回前台后清理旧 Packet/Frame，从最新关键帧恢复；
- page freeze、BFCache、device lost 和 context lost 进入显式恢复流程；
- destroy 后不得遗留 Worker、MediaSource、AudioContext、GPU resource 或 object URL。

## 10. 手动策略

公共配置允许 `auto`、`webcodecs`、`mse`、`wasm` 等测试策略。强制模式仍必须验证安全与基本兼容；不支持时返回稳定 `UnsupportedBackend`，不能静默切换，除非调用方同时启用 fallback。
