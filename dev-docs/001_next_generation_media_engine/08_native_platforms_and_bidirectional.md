# 08. 原生平台与双向引擎

## 1. 平台顺序

Web v1 稳定后按以下顺序验证：

1. Qt Native；
2. Android；
3. iOS；
4. 鸿蒙。

顺序只表示产品交付优先级。共享核心和 ABI 从 Phase 0 起必须保持平台中立，不能在 Qt 阶段固化桌面专用假设。

## 2. 原生总体结构

```text
Application UI
      │ platform wrapper
      ▼
Stable C ABI / language binding
      │
      ▼
Rust engine orchestration
      │
      ├── shared media core
      ├── platform transport
      ├── hardware decoder/encoder
      ├── renderer/audio sink
      └── lifecycle/diagnostics
```

Rust 公共核心不暴露 Qt object、JNI object、Objective-C object 或鸿蒙 Native API 类型。平台对象只由对应 backend 持有，并以不透明 handle 进入 engine。

## 3. 稳定 C ABI

C ABI 负责：

- engine/player 创建和释放；
- source、policy 和资源上限配置；
- 异步命令提交；
- 类型化事件和统计回调；
- platform surface、audio device 和 credential provider 接入；
- buffer/FrameHandle retain/release；
- ABI/capability 协商。

约束：

- 所有字符串显式 UTF-8 pointer/length；
- callback 携带 generation 和 user_data；
- 回调线程在 API 中固定说明；
- callback 内禁止阻塞 engine；
- ABI 不跨边界传播 panic/exception；
- stop/destroy 可重复调用且有明确完成通知。

## 4. Qt Native

### 4.1 目标

- Windows、Linux、macOS；
- QWidget 与 QML/Qt Quick surface 接入；
- 复用 Rust 网络、demux、同步和录制；
- 使用平台硬解和 GPU renderer，不依赖 Qt Multimedia 作为媒体核心。

### 4.2 后端

- Windows：Media Foundation/D3D11 或经过验证的 FFmpeg 硬件路径；
- Linux：VA-API/Vulkan/OpenGL；
- macOS：VideoToolbox/Metal；
- 音频通过平台 API 或受控 Qt audio adapter；
- surface resize、DPI、窗口隐藏和 GPU device lost 映射到统一生命周期。

Qt 阶段必须证明 C ABI、FrameHandle、线程回调和平台 surface 模型不依赖 Web。

## 5. Android

- Kotlin API 由 JNI 薄绑定生成或维护；
- MediaCodec 负责硬解/硬编，Surface/SurfaceTexture 负责零拷贝显示主路径；
- OpenGL ES/Vulkan 用于自定义渲染、覆盖层和软解帧；
- AudioTrack/AAudio 作为 AudioSink；
- Activity/Fragment/Service 生命周期映射为 suspend/resume/stop；
- 前后台、音频焦点、网络切换和 Surface 重建必须可恢复；
- 不在 JNI 每帧创建 Java byte[] 或普通对象图。

## 6. iOS

- Swift API 包装稳定 C ABI；
- VideoToolbox 负责硬解/硬编；
- CVPixelBuffer/IOSurface 与 Metal renderer 尽量保持平台资源引用；
- AVAudioEngine/AudioUnit 作为 AudioSink/CaptureSource；
- app inactive/background、音频 session、route change 和 memory warning 进入统一事件；
- FrameHandle 释放必须尊重 CoreVideo/CoreMedia 引用规则；
- App Store 分发下 codec pack 和动态代码策略必须单独审查。

## 7. 鸿蒙

- 首先验证目标系统版本、ArkTS/NAPI/Native API 和媒体编解码能力；
- Rust 通过稳定 C ABI/NAPI wrapper 接入，不在共享核心依赖 ArkTS；
- 使用平台硬解、surface、图形和音频 API；
- 平台缺失能力通过 capability 返回，不复制 Android backend 假定兼容；
- 真机工具链、签名、包格式和应用市场规则进入独立发布矩阵。

## 8. 平台能力协商

每个平台统一返回：

- codec/profile/level/bit depth；
- 最大分辨率、帧率和并发实例；
- hardware/software 属性和是否可强制；
- zero-copy surface/resource 类型；
- renderer format、HDR 和颜色能力；
- audio format、采集和回声处理能力；
- 后台、录制、文件写出和安全存储能力。

能力结果是提示和测试输入，运行时仍必须监测失败、吞吐和资源压力。

## 9. 双向能力

### 9.1 统一端口

```text
CaptureSource -> raw Frame/PCM
Processor     -> optional transform
Encoder       -> compressed Packet
Packetizer    -> protocol packet/segment
Publisher     -> RTMP/WebRTC/WebTransport/etc.
```

播放和发布共享 TrackInfo、Packet/Frame、timebase、资源预算和诊断模型。

### 9.2 音频对讲

- 麦克风采集、AEC/NS/AGC 由平台 capability 控制；
- 支持 PCM、G.711A/U 和后续 Opus/AAC 编码；
- half-duplex/full-duplex 是显式 session policy；
- RTP、JT/T 等封装由独立 packetizer 完成；
- 权限拒绝、设备占用和 route change 返回可恢复错误。

### 9.3 视频发布

- 首批考虑 WebRTC 和 RTMP 发布；
- 编码器输入优先使用平台 GPU/zero-copy frame；
- 编码码率、关键帧、分辨率和拥塞控制由 typed config 表达；
- 网络拥塞不得导致 CaptureSource 无界堆积；
- 动态降码率、降帧和关键帧请求进入统一 publisher feedback。

## 10. 原生验收原则

- 相同 fixture 在 Web 和 Native 解析结果一致；
- 相同 Packet timeline 在不同平台的呈现顺序一致；
- 平台层不复制 FLV/TS/MP4/parser；
- 硬解失败可切换平台软解或 LGPL codec backend；
- surface/app lifecycle 压力下无 use-after-free、旧帧污染或线程泄漏；
- 每个平台建立真机兼容矩阵，不因 API 存在即宣称支持全部设备。
