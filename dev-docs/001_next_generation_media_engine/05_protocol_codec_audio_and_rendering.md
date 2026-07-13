# 05. 协议、编解码、音频与渲染

## 1. Web v1 输入矩阵

| 输入 | 传输 | 解封装 | WebCodecs | MSE | WASM |
| --- | --- | --- | --- | --- | --- |
| HTTP-FLV | Fetch stream | Rust FLV | 压缩 AU 直送 | 重封装 fMP4 | 压缩 AU 直送 |
| WS-FLV | WebSocket | Rust FLV | 压缩 AU 直送 | 重封装 fMP4 | 压缩 AU 直送 |
| HLS-TS | Fetch | playlist + TS | 压缩 AU 直送 | 重封装或浏览器接受路径 | 压缩 AU 直送 |
| HLS-fMP4/LL-HLS | Fetch | playlist + ISOBMFF | sample 直送 | segment append | sample 直送 |
| HTTP-fMP4 | Fetch stream | Rust ISOBMFF | sample 直送 | segment append | sample 直送 |
| WS-fMP4 | WebSocket | Rust ISOBMFF | sample 直送 | segment append | sample 直送 |

HLS playlist client、segment pacing、FLV/TS/fMP4 demux 和时间线属于共享 Rust 核心。浏览器只提供网络 I/O。

## 2. H.264/H.265

共享核心负责：

- Annex-B 与 length-prefixed 格式识别和转换；
- SPS/PPS/VPS 解析、缓存、变更检测和补发；
- AVCC/HVCC 构建；
- IDR、CRA、BLA 等随机访问判断；
- key/config/corrupt/discontinuity 标记；
- profile、level、bit depth、分辨率和颜色信息提取；
- B 帧 DTS/PTS 保持和错误时间戳修复。

解码 backend 不得各自复制一套参数集或关键帧判断逻辑。

WebCodecs 路径必须：

- 使用完整 codec string 和 description；
- 从随机访问帧开始；
- 监测 decodeQueueSize 和 output deadline；
- 动态配置变化时 flush/reset/reconfigure；
- 每个 VideoFrame 在提交显示后及时 close。

MSE 路径必须生成符合浏览器 byte stream 要求的 init segment、moof/mdat 和时间戳。H.265 的 hvc1/hev1 选择进入兼容 profile，不能全局硬编码。

## 3. WASM codec pack

### 3.1 版本

每个 decoder 至少产生：

- `threads-simd`：共享内存、原子和 SIMD；
- `simd`：单线程 SIMD；
- `baseline`：WebAssembly MVP 兼容路径。

不同变体具有相同 ABI 和功能语义。加载失败时由 planner 选择下一变体。

### 3.2 模块边界

- codec pack 独立 WASM 文件、独立许可证清单和 source offer；
- 使用共享核心提供的 memory/allocator 或显式 caller-owned buffer；
- 输入为 PacketHandle/descriptor，输出为 FrameHandle/plane descriptor；
- decoder 不访问 DOM、网络、业务事件或录制 API；
- decoder 内部线程数、帧池和参考帧内存有硬上限；
- 发生 OOM、畸形 bitstream 或持续超时必须返回错误，不得 panic 宿主。

## 4. 音频

### 4.1 AAC 与 MP3

优先级由 planner 决定：

- MSE 完整音视频路径；
- WebCodecs AudioDecoder；
- LGPL codec pack 软解；
- 平台不支持时返回 UnsupportedAudioCodec。

AAC ADTS、AudioSpecificConfig 和 raw access unit 转换由 Rust 核心完成。

### 4.2 G.711

G.711A/U 使用 Rust 纯实现解码为 PCM：

- 支持 8 kHz 等实际流参数；
- 时间戳以采样数推导，不用视频帧率估算；
- PCM 写入 AudioWorklet 有界环形缓冲；
- 缓冲欠载输出静音并计数，过载丢弃最旧 PCM 追赶实时；
- 不把 G.711 交给不支持的 MSE SourceBuffer。

### 4.3 AudioWorklet

- 隔离档使用 SharedArrayBuffer PCM ring；
- 兼容档使用有界 MessagePort buffer；
- AudioContext 必须遵守浏览器 autoplay/user gesture 规则；
- resume 失败触发 `audio_blocked` 事件，不阻止静音视频播放；
- mute 只影响输出，不停止音频时间线；
- stop/destroy 释放 node、port 和 context ownership。

## 5. 音画同步

- 音频稳定时以音频时钟为主；
- 视频根据 PTS 提前等待、过期丢帧；
- 无音频时使用单调时钟映射媒体时间；
- MSE 音视频都在 video 内时以 HTMLMediaElement 时钟为准；
- MSE video + AudioWorklet G.711 时维护显式共同 timeline epoch；
- 音画差超阈值时优先丢视频或小幅音频补偿，禁止突发播放大量积压帧。

## 6. 视频渲染

### 6.1 WebCodecs

优先级：

1. WebGPU `importExternalTexture(VideoFrame)`；
2. WebGL2 使用 VideoFrame 上传/外部图像路径；
3. Canvas2D；
4. 在能力允许时使用 video/WritableStream 等平台路径。

不需要像素访问时不得调用 VideoFrame.copyTo。

### 6.2 WASM 软解

- YUV420P、NV12、P010 等保持 plane/stride 描述；
- WebGPU/WebGL shader 完成 YUV→RGB、range、matrix 和 transfer 处理；
- 8/10 bit 和 HDR 能力进入 renderer capability；
- texture 池按最大可见窗口数有界复用；
- GPU upload 统计字节和耗时；
- context/device lost 后重建 renderer，保留 pipeline generation 防止旧帧进入新 surface。

### 6.3 MSE

MSE 由 `<video>` 显示。需要截图或覆盖层时允许 drawImage/GPU copy，但必须满足 CORS 并单独计量。MSE 不提供与 WebCodecs 相同的逐帧控制承诺。

## 7. 颜色与画面变换

- 优先使用 bitstream/VUI 或 container 的颜色信息；
- 缺失时按 codec、分辨率和 compat profile 选择明确默认值并发出诊断；
- rotate、mirror、fit 在渲染矩阵完成，不复制 Frame；
- snapshot 使用当前呈现变换；
- 原码流录制保持输入 bitstream，不写入视觉变换。

## 8. 动态变化

必须覆盖：

- 分辨率、profile、bit depth 和颜色空间变化；
- 音频采样率和声道变化；
- 参数集 in-band 更新；
- codec change；
- 时间戳回绕、跳变和 discontinuity；
- 输入在关键帧前开始；
- 视频无音频、音频无视频和迟到轨道。

变化不能导致旧尺寸 texture、旧 decoder output 或旧 timeline epoch 被继续使用。

## 9. 后续协议

Pro 等价阶段按共享模型增加：

- WebRTC、WebTransport；
- MPEG-PS、裸 H.264/H.265、通用 MPEG-TS；
- MP4/HLS 点播和行业录像回放；
- AV1、VP8、VP9、MPEG-4 Part 2；
- Opus 等双向通信 codec。

新增协议或 codec 只能扩展 capability 和 adapter，不改变现有 Packet/Frame 时间线语义。
