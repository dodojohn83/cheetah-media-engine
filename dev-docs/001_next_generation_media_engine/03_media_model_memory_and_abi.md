# 03. 媒体模型、内存与 ABI

## 1. 统一媒体模型

所有输入协议在进入通用播放管线前统一为：

```text
TrackInfo
  track_id / media_kind / codec
  timebase / profile / level
  width / height / fps / bitrate
  sample_rate / channels
  codec extradata / readiness

PacketHandle
  track_id / codec / format
  pts / dts / duration / timebase
  key/config/discontinuity/corrupt flags
  payload range / side-data range

FrameHandle
  track_id / media kind / format
  pts / duration / dimensions
  platform resource or plane descriptors
  color space / HDR / rotation metadata
```

协议原生时间戳保存在 side data，标准调度时间线使用明确 timebase 和规范化后的 PTS/DTS。不得把 RTMP 毫秒、RTP ticks、MPEG 90 kHz 和微秒裸整数混用。

## 2. 所有权规则

- Packet/Frame payload 通过不透明句柄和引用计数共享；
- clone 句柄只增加引用，不复制 payload；
- 所有后端必须在消费完成后显式 release；
- VideoFrame、GPU texture、native decoder buffer 等平台资源通过外部资源 variant 表示；
- 资源最终释放必须回到创建它的平台后端和正确线程；
- stop/destroy 后仍到达的旧 generation callback 只允许释放资源，不得修改新状态。

任何跨线程、跨 WASM 模块或跨 FFI 的资源都必须明确：所有者、引用计数位置、可访问线程、释放函数和超时回收策略。

## 3. 稳定 ABI

Rust struct 布局不是 ABI。公共边界使用：

- `#[repr(C)]` 的固定宽度标量描述符；
- 不透明的 32/64 位 handle；
- 稳定数值 enum；
- `ptr + len + capacity/arena_id` 范围；
- 显式 ABI major/minor 和 capability bits；
- 调用方分配或引擎分配的明确规则。

公共 ABI 禁止：

- Rust `Vec`、`String`、trait object、enum 内存布局；
- JS object 作为媒体帧；
- JSON/Base64 payload；
- 让一个 allocator 释放另一个 allocator 的地址；
- 未校验的裸指针长期保存在宿主。

## 4. Web 共享内存

隔离环境的优化路径使用一块可共享 `WebAssembly.Memory`，划分为：

```text
control region       ABI version / queues / atomics
packet arena         compressed input and access units
frame arena          software-decoded planes
descriptor arena     Track/Packet/Frame descriptors
scratch arena        bounded transform and mux scratch
```

- 内存上限在初始化时配置；
- 不依赖频繁 memory.grow，发生 grow 后所有 JS typed-array view 必须刷新；
- codec pack 使用宿主提供的共享内存和引擎 allocator API，不能维护冲突的全局 allocator；
- 环形队列使用单生产者/单消费者或经过证明的原子协议；
- 队列满执行明确的 backpressure/drop，不覆盖仍被引用的数据。

非隔离环境不共享线性内存，使用单 Worker 或 ArrayBuffer transfer。功能保持一致，但不承诺多线程软解密度。

## 5. 复制预算

### 5.1 压缩数据

正常路径允许：

1. 浏览器网络缓冲写入 WASM packet arena 一次；
2. WebCodecs/MSE 提交时由浏览器内部持有或复制一次，该过程不计为应用可控零复制；
3. 输入格式与后端格式不一致时，每个 access unit 最多一次可计量的合并/改写复制。

禁止在 demux、时间线、GOP cache、后端队列和事件分发之间重复复制完整 payload。

### 5.2 解码数据

- WebCodecs 输出保持 VideoFrame 资源，不执行 CPU `copyTo`，除非截图、AI 或显式像素访问；
- WebGPU 优先使用 external texture；
- WASM 软解输出保持 YUV/NV12/P010 plane，不先转 RGBA；
- WebGL/WebGPU 上传属于必要的 GPU 传输，必须计量提交字节；
- 截图和合成录制是显式复制场景，单独统计，不污染播放基线。

## 6. 有界资源

每个播放器必须配置或继承：

- 网络积压字节上限；
- demux 未完成单元上限；
- Packet 数量和总字节上限；
- GOP 数量、时长和字节上限；
- decoder 输入/输出队列上限；
- 等待显示 Frame 数量上限；
- MSE append queue 和 buffered duration 上限；
- AudioWorklet PCM 时长上限；
- 录制写出队列和 Blob fallback 上限。

默认实时模式下，超过 max latency 或资源上限时按以下顺序处理：

1. 丢弃已过显示期限的非关键视频帧；
2. 清空到最新关键帧并重建解码依赖；
3. 降低非焦点窗口帧率或切子码流；
4. recorder 慢时停止录制并报错，不能拖慢播放；
5. 无法恢复时触发后端 fallback 或有界重连。

## 7. 时间线与同步

- 单调时钟用于 deadline、队列年龄和调度；
- 媒体 PTS/DTS 用于解码和呈现；
- wall clock 只用于端到端延迟和外部时间映射；
- 音频存在且稳定时通常作为播放主时钟；
- MSE video 与外部 G.711 AudioWorklet 组合时使用显式 media-time 映射；
- discontinuity、seek、codec change 和主子码流切换必须建立新 timeline epoch；
- 不通过不断调整单帧时间戳掩盖持续漂移。

## 8. 可观测复制指标

每个实例至少暴露：

- ingress copied bytes；
- access-unit transform copied bytes；
- mux copied bytes；
- software frame upload bytes；
- screenshot/readback bytes；
- Packet/Frame arena 当前值和峰值；
- allocation/reuse 计数；
- 各队列当前深度、峰值和 drop 原因。

性能报告必须同时给出复制字节和媒体输入字节，避免只报告无法比较的绝对复制量。
