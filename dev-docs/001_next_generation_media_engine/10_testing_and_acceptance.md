# 10. 测试与验收

## 1. 测试分层

```text
Shared core unit / property / fuzz
               ↓
WASM ABI and codec-pack contract
               ↓
Platform backend fault tests
               ↓
Web SDK and component integration
               ↓
Protocol/codec browser end-to-end
               ↓
Real device / density / 24h soak
```

生产问题必须优先沉淀为脱敏媒体 fixture、网络响应序列或状态机输入，再修复。只依赖临时在线流、无法重复的问题不算完成回归闭环。

## 2. 共享核心

必须覆盖：

- FLV、MPEG-TS、ISOBMFF、HLS playlist 增量解析；
- 任意输入切片边界结果一致；
- parse/serialize/parse 语义 round-trip；
- H.264/H.265 参数集、随机访问和格式转换；
- AAC ADTS/ASC、G.711 时间戳；
- PTS/DTS、B 帧、回绕、discontinuity 和 timebase；
- GOP cache、jitter、队列和 drop policy 的硬上限；
- 动态分辨率、codec change 和 timeline epoch；
- malformed input 不 panic、不无限循环、不无界分配。

持续 fuzz target：FLV tag、MPEG-TS/PES、MP4 box、HLS playlist、H.26x NALU/parameter set、AAC header 和公开 ABI descriptor。

## 3. ABI 与 codec pack contract

每个 engine/codec variant 运行相同 contract suite：

- ABI major/minor 和 capability 协商；
- allocator ownership、retain/release 和重复释放防护；
- input PacketHandle 到 output FrameHandle；
- threads-simd、simd、baseline 输出时间线和画面 hash 一致；
- 分辨率变更、flush、reset、destroy；
- 畸形输入、OOM、trap、Worker crash 和取消；
- codec pack 可替换且不访问未授权 memory range；
- stop 后资源回收到基线。

## 4. Web 后端

### 4.1 WebCodecs

- H.264/H.265 支持、拒绝和伪支持场景；
- configure 后无输出、连续 decode error、queue overload；
- VideoFrame close 和旧 generation output；
- 动态配置和 renderer 切换；
- WebGPU/WebGL/Canvas fallback。

### 4.2 MSE

- init segment、codec string 和 append 顺序；
- FLV/TS 重封装；
- QuotaExceeded、append error、timestamp discontinuity；
- buffer eviction、后台追赶和 endOfStream；
- MSE video + G.711 AudioWorklet 同步；
- Worker MediaSource 可用与不可用环境。

### 4.3 WASM

- SIMD/threads 探测失败；
- H.264/H.265 baseline 和软解性能；
- YUV plane/stride、10 bit 和颜色信息；
- GPU upload、texture pool 和 context/device lost；
- 非隔离单 Worker 路径。

## 5. 回退故障矩阵

| 故障 | 必须验证 |
| --- | --- |
| WebCodecs configure 失败 | 选择下一个可行 backend，网络不重复拉取 |
| 关键帧输入后无输出 | deadline 熔断并使用缓存 GOP 回退 |
| MSE codec/addSourceBuffer 失败 | 不遗留 MediaSource/SourceBuffer |
| MSE QuotaExceeded | 先收缩 buffer，无法恢复再回退 |
| WASM Worker crash/trap | 释放 arena 引用，尝试下一 variant 或失败 |
| decoder queue 持续过载 | 实时丢帧/切子码流，不累积延迟 |
| WebGPU device lost | 重建 renderer 或回退 WebGL |
| 页面后台/恢复 | 后台停解码，前台从最新关键帧恢复 |
| 网络断开 | 有界退避重连，旧 generation 回调无效 |
| stop/destroy 竞态 | 所有 Worker、decoder、audio、record 释放 |

## 6. 协议与媒体语料

fixture matrix 至少包含：

- H.264 baseline/main/high，含无 B 帧和有 B 帧；
- H.265 main/main10；
- 720p、1080p、2K、4K 和非标准分辨率；
- 15/25/30/50/60 fps；
- AAC 不同采样率/声道、G.711A/U、MP3；
- 无音频、纯音频、迟到音频；
- 缺失首个 I 帧、参数集 in-band 更新、动态分辨率；
- 时间戳回绕、倒退、跳变、错误 duration；
- 截断、损坏、重复和超大 packet；
- HTTP chunk 边界、WebSocket message 边界、HLS part/segment 边界。

fixture 必须记录来源类别、codec/container 信息、预期结果、脱敏方式和许可证，不提交未授权真实监控内容。

## 7. SDK 与 UI

- 状态机和 generation/cancel；
- autoplay blocked、mute/unmute 和 AudioContext resume；
- rotate/mirror/fit/fullscreen；
- screenshot 格式和 CORS taint；
- MP4/FLV WritableStream、File System Access、Blob 上限；
- 1/4/9/16 布局、focus、主子切换和失败回滚；
- Shadow DOM theme、slot、自定义按钮和国际化；
- React/Vue 薄示例不产生双重初始化或泄漏；
- diagnostics 不泄漏 token、URL query 和媒体 payload。

## 8. 浏览器矩阵

- Windows：Chrome、Edge 当前稳定版及前两个大版本；
- macOS：Safari、Chrome 当前稳定版及前两个大版本；
- Firefox：当前稳定版及前两个大版本，按实际 capability 走兼容路径；
- Android Chrome：至少一个真实中端设备单路验收；
- iOS Safari：至少一个 A15 级设备单路验收；
- 隔离和非隔离部署各运行一套核心用例。

Playwright/WebKit 不能替代真实 Safari 和真实硬件解码测试。

## 9. 性能验收

### 9.1 参考环境

- Windows 11，Intel Core i5-12400/UHD 730，16 GB，安装合法 HEVC 系统组件；
- Apple M1，8 GB，当前受支持 macOS；
- 浏览器硬件加速开启，无其他显著负载；
- LAN RTT ≤10 ms，稳定服务器，GOP ≤1 秒；
- 1080p H.265 目标码率 4 Mbps，720p H.265 目标码率 2 Mbps；
- 每次报告记录完整版本和偏差。

### 9.2 通过条件

| 指标 | 通过条件 |
| --- | --- |
| FLV/fMP4 首帧 | P95 ≤800 ms |
| FLV/fMP4 稳态直播延迟 | P95 ≤600 ms |
| LL-HLS | P95 ≤1.5 s |
| 音画差绝对值 | P95 ≤50 ms |
| 容量内主动丢帧 | <0.5% |
| 1080p25 H.265 | 9 路硬解稳定播放 |
| 720p15 H.265 | 16 路硬解稳定播放 |
| 软解 | 参考桌面单路 H.265 1080p25 SIMD 达到实时 |

性能报告同时包含 CPU、GPU、RSS/JS/WASM、输入码率、复制字节、queue depth、decoder/backend 和丢帧原因。

## 10. 24 小时 soak

运行内容：

- 9 路 1080p 或 16 路 720p；
- 定时 focus 和主子码流切换；
- 周期性隐藏/恢复页面；
- 网络短断、服务端重连和 decoder fault injection；
- 截图和短录制；
- WebGPU context/device 恢复场景。

验收：

- 无浏览器 tab 崩溃、WASM OOM 或未恢复 Worker；
- 延迟漂移 ≤100 ms；
- 预热后 JS/WASM 内存增长 ≤5%；
- Packet/Frame/texture/VideoFrame 数量回到稳态区间；
- 队列无持续单调增长；
- fallback、重连和录制资源全部回收；
- 日志和诊断包无敏感数据。

## 11. 复制验收

- 输入进入 WASM 最多一次显式完整 payload 复制；
- 格式保持路径 demux/cache/queue 不复制 payload；
- 格式转换每 AU 最多一次合并复制；
- WebCodecs 路径不做常规 VideoFrame CPU readback；
- 软解路径不做 CPU YUV→RGBA；
- 所有例外在 stats 中按原因计数；
- 回归构建的单位输入字节复制量不得无解释增长。

## 12. 发布门禁

- Rust format、clippy、unit/property/fuzz regression；
- TypeScript typecheck、lint、unit、component 和浏览器 E2E；
- ABI/codec pack contract；
- 依赖 advisory、license、SBOM 和可重复构建；
- 浏览器核心矩阵；
- 性能 smoke；
- release candidate 24 小时 soak；
- 公共 API、ABI、asset manifest 和兼容矩阵已更新；
- v1 capability 不存在空实现或虚假成功。
