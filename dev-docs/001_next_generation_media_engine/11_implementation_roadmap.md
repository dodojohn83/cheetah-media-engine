# 11. 实施路线图

## 1. 建议仓库结构

共享核心独立仓库建议：

```text
cheetah-media-core-rs/
  crates/
    types
    bitstream
    container-flv
    container-mpegts
    container-isobmff
    hls-client
    timeline
    pipeline-core
    abi
  testing/
    fixtures
    property
    fuzz
```

本仓库建议：

```text
cheetah-media-engine/
  crates/
    engine
    backend-api
    bindings-c
    bindings-wasm
  packages/
    web-sdk
    web-components
    web-runtime
  codec-packs/
    build
    manifests
  apps/
    web-demo
    benchmark
  tests/
    browser
    performance
```

初始实现可合并过小 crate，但必须保持职责和依赖方向，不以 crate 数量制造循环依赖。

## 2. Phase 0：可行性闸门

交付：

- 固定实际可用 Rust stable、WASM 和前端工具链；
- 从 `cheetah-media-server-rs` 提取可复用媒体能力和 fixture；
- 修复 `no_std`/WASM 编译路径；
- 定义 Packet/Frame/Track 和共享内存 ABI；
- HTTP-FLV H.265/AAC 最小播放器；
- WebCodecs、MSE、threads-simd、simd、baseline 原型；
- WebGPU/WebGL YUV 渲染；
- 显式复制、内存和首帧测量；
- LGPL codec pack 构建与替换验证。

完成条件：

- HLS、FLV、TS、fMP4 共享解析能力在 wasm32 和 native 构建；
- H.265 1080p25 SIMD 软解在参考桌面达到实时；
- 四类播放路径可由故障注入触发切换；
- 热路径无 JSON/Base64 payload；
- 输入到 WASM 复制预算和内部 payload 共享得到实测证明；
- codec pack 许可证和替换机制通过审查。

任一核心条件失败时暂停产品 UI 开发，先调整 ABI、codec 选型或内存模型。

## 3. Phase 1：共享核心基线

交付：

- 独立 `cheetah-media-core-rs` 仓库；
- Track/Packet/Frame、时间线、参数集和 GOP cache；
- FLV、MPEG-TS、ISOBMFF 和 HLS client；
- pipeline planner 的平台无关输入/输出模型；
- C ABI descriptor 和 WASM memory ABI；
- fixture、property、fuzz 和 benchmark；
- media server 通过固定 tag 消费共享 crates 的迁移方案。

完成条件：engine 与 media server 对相同 fixture 产生一致 Track/Packet/时间线结果，服务端协议会话未进入共享核心。

## 4. Phase 2：Web v1 播放内核

按顺序实现：

1. Fetch/WebSocket transport 和取消；
2. Worker runtime、隔离/非隔离模式；
3. WebCodecs video/audio backend；
4. MSE fMP4 backend；
5. WASM codec pack loader 和软解；
6. WebGPU/WebGL/Canvas/video renderer；
7. AudioWorklet 和 A/V sync；
8. 动态 planner、probe、fallback 和实时追赶；
9. stats、diagnostics 和资源回收。

完成条件：全部 v1 协议/codec 组合通过浏览器核心矩阵，后台恢复和 backend fault 不累积延迟或泄漏资源。

## 5. Phase 3：Web SDK 与安防 UI

交付：

- TypeScript SDK 和稳定事件/错误模型；
- 单窗 Web Component；
- CheetahWall 1/4/9/16 宫格；
- 主子码流无黑屏切换和全局资源预算；
- 截图、MP4/fMP4 与 FLV 流式录制；
- 性能面板、诊断包和 demo；
- npm ESM、IIFE/UMD 和自托管 assets。

完成条件：满足 [10_testing_and_acceptance.md](10_testing_and_acceptance.md) 的桌面密度、延迟、复制和 24 小时 soak 门禁。

## 6. Phase 4：Web Pro 功能等价

按依赖顺序交付：

1. WebRTC、WebTransport、TS/PS/裸流；
2. MP4/HLS 点播和行业录像回放；
3. 倍速、逐帧、逐关键帧和回放 UI；
4. PTZ、电子放大、高级多画面和国际化；
5. 水印、SM4/XOR/AES 和 SEI/私有数据；
6. 麦克风、G.711/Opus 和语音对讲；
7. 下载器、合成录制、VR 和可插拔 AI。

完成条件：[07_jessibuca_pro_feature_parity.md](07_jessibuca_pro_feature_parity.md) 的所有目标项均有实现状态和验收证据，未完成项不进入功能等价声明。

## 7. Phase 5：Qt Native

交付：

- 稳定 C ABI；
- Windows/Linux/macOS transport、hardware decoder、renderer 和 audio backend；
- QWidget/QML wrapper；
- 单窗、多画面、录制和 diagnostics；
- Web/Qt 共用 fixture 和 timeline contract。

完成条件：Qt 不依赖 Web SDK，不复制容器实现，surface/device 生命周期测试无泄漏和旧帧污染。

## 8. Phase 6：Android、iOS、鸿蒙

依次交付：

- Android JNI/Kotlin、MediaCodec、Surface、AudioTrack；
- iOS Swift、VideoToolbox、Metal、AudioUnit；
- 鸿蒙 NAPI/ArkTS wrapper、平台 codec、surface 和 audio；
- 每个平台真机兼容矩阵和生命周期 soak。

完成条件：相同共享 fixture 和 C ABI contract 全部通过，平台限制通过 capability 如实暴露。

## 9. Phase 7：双向实时引擎

交付：

- CaptureSource、Processor、Encoder、PublisherBackend；
- 麦克风/摄像头和屏幕采集；
- H.264/H.265/Opus/AAC/G.711 平台能力；
- WebRTC/RTMP 等发布路径；
- 拥塞反馈、动态码率、关键帧和双工音频；
- 播放与发布统一资源预算和 diagnostics。

完成条件：采集到远端播放的端到端测试覆盖网络拥塞、设备切换、后台、权限拒绝和编码器故障。

## 10. 迁移规则

从 media server 提取共享能力采用：

1. 固定原始 commit 和 fixture；
2. 先迁移纯媒体/容器代码，不搬 driver/module；
3. 两边并行运行 golden/contract；
4. media server 改用固定共享 tag；
5. 删除原仓库重复实现；
6. 一个发布周期内保留可回退依赖版本；
7. 后续修复只进入共享核心，不双写。

## 11. Web v1 完成定义

Web v1 只有同时满足以下条件才可发布：

- 三类协议和首批 codec 全部完成；
- 动态 WebCodecs/MSE/WASM 回退通过故障矩阵；
- 隔离与非隔离模式均可用；
- 单窗、1/4/9/16 宫格、主子码流完成；
- 截图和 MP4/FLV 流式录制完成；
- 桌面密度、延迟、A/V sync、复制和 24 小时 soak 通过；
- npm/CDN/self-host 制品和 ABI manifest 完成；
- 许可证、SBOM、安全和敏感数据门禁通过；
- 不支持能力返回明确 Unsupported，不存在空实现。
