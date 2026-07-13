# 06. Web SDK、UI、多画面与录制

## 1. 交付形态

Web v1 发布：

- 无框架 TypeScript ESM SDK；
- script-tag 可使用的 IIFE/UMD bundle；
- 单窗播放器 Web Component；
- 多画面电视墙 Web Component；
- Worker、engine WASM 和可选 codec pack；
- 参考 demo、诊断页和性能基准页。

SDK 不依赖 React/Vue。框架适配只包装生命周期和 props/events，不复制播放器逻辑。

## 2. Source 与配置

核心配置模型：

```text
PlayerOptions
  source
    main: SourceSpec
    sub?: SourceSpec
  decoder_policy: auto | webcodecs | mse | wasm
  renderer_policy: auto | webgpu | webgl2 | canvas2d | video
  fallback_enabled: bool
  latency
    mode: realtime | balanced
    target_ms
    max_ms
  audio / display / worker / memory / retry / recording
```

`SourceSpec` 至少包含 URL、协议提示、credentials、HTTP headers、WebSocket subprotocol、是否直播和业务 metadata。协议提示为 `auto` 时按 URL、Content-Type 和首段探测，不只依赖扩展名。

敏感 token 不得进入 stats、日志、错误或可下载诊断包。

## 3. CheetahPlayer API

异步方法：

```text
load(options or source)
play()
pause()
stop()
destroy()
switchQuality(main | sub | auto)
snapshot(options)
startRecording(options)
stopRecording()
```

同步或轻量控制：

```text
setMuted(bool)
setVolume(0..1)
setFit(contain | cover | fill)
setRotation(0 | 90 | 180 | 270)
setMirror(horizontal, vertical)
getState()
getCapabilities()
getStats()
```

- 同一实例的控制命令串行化；
- destroy 幂等，destroy 后其他方法返回稳定错误；
- load 新 source 自动取消旧 generation；
- autoplay 被浏览器阻止时返回可恢复状态，不伪造 Playing。

## 4. 事件

公开类型化事件：

- `statechange`；
- `tracks`、`firstframe`、`playing`；
- `backendchange`、`qualitychange`；
- `latency`、`drop`、`buffering`；
- `audio_blocked`、`visibilitychange`；
- `reconnecting`、`reconnected`；
- `recordingstart`、`recordingdata`、`recordingstop`；
- `warning`、`error`；
- `stats`。

高频 stats 默认按固定间隔聚合，不为每帧派发 DOM event。

错误包含稳定 code、stage、retryable、backend 和安全 message。内部异常栈只进入显式 debug 诊断。

## 5. 参考播放器 UI

v1 控件包含：

- 播放/暂停/停止；
- 静音、音量；
- 全屏和容器内全屏；
- 截图、开始/停止录制；
- 主/子/自动码流；
- contain/cover/fill；
- 旋转和镜像；
- loading、错误和重试状态；
- 可折叠性能面板。

Web Component 使用 Shadow DOM 隔离默认样式，同时提供 CSS custom properties、slots 和自定义按钮注册点。UI 文案通过可注入字典国际化。

## 6. 多画面 CheetahWall

### 6.1 API

至少提供：

```text
setLayout(1 | 4 | 9 | 16)
attach(index, player/source)
detach(index)
focus(index)
clear()
setBudget(policy)
getStats()
```

### 6.2 全局资源预算

预算器统一管理：

- 可用硬解实例和当前 backend；
- 总输入码率、CPU、GPU 和 engine 内存；
- 可见面积、焦点和用户固定优先级；
- 主子码流状态和切换冷却时间；
- 每路目标帧率和是否暂停 renderer/decoder。

默认优先级：焦点/全屏 > 可见大窗 > 可见小窗 > 不可见窗。

### 6.3 主子码流切换

- 宫格使用子码流；
- focus/fullscreen 请求主码流；
- 切换前保持旧画面，等待新源参数集和关键帧；
- 新源首帧成功后原子替换；
- 失败保留旧源并发出 qualitychange error；
- 频繁 focus 使用冷却时间避免连接和 decoder 震荡。

## 7. 截图

- 输出 Blob、ArrayBuffer 或业务提供的 WritableStream；
- 支持 PNG/JPEG/WebP，具体格式通过 capability 返回；
- 默认包含当前 rotate/mirror/fit 后的可见画面，不包含播放器控件；
- MSE/video 截图受 CORS taint 限制，失败返回明确错误；
- 暗水印、区域截图和 AI overlay 属于后续阶段。

## 8. 原码流录制

### 8.1 格式

- MP4/fMP4：默认选择，支持 H.264/H.265 与可封装音频；
- FLV：用于保持原始 FLV 兼容和直播下载；
- 不重新编码，不包含 UI、旋转、镜像、水印或 AI overlay。

### 8.2 写出

优先级：

1. 调用方提供的 WritableStream；
2. File System Access writable；
3. 分片回调；
4. 有明确最大时长/字节上限的 Blob fallback。

writer 过慢时录制独立失败并停止，不能增加播放延迟。stopRecording 必须完成必要的容器尾部/finalization，取消或磁盘错误返回可诊断的部分文件状态。

### 8.3 时间线

- 录像从可随机访问视频帧开始；
- 音频早于首个视频关键帧的部分按配置丢弃或对齐；
- discontinuity、codec change 和质量切换默认切分新文件/fragment epoch；
- 时间戳保持单调并记录修复诊断；
- 无音频和纯音频必须分别有明确 capability。

## 9. 性能面板

显示：

- 当前协议、轨道、codec/profile、分辨率、帧率和码率；
- decoder、renderer、audio backend 和 fallback 历史；
- 首帧、直播延迟、A/V 差；
- 接收、解码、显示、丢弃帧数；
- Packet/Frame/MSE/audio/record queue；
- JS、WASM arena 和估算 GPU 资源；
- 显式复制次数、字节和原因；
- Worker、COOP/COEP、SIMD、threads、WebGPU 能力。

debug 日志默认关闭且有环形上限，不能因长时间打开 DevTools 导致无界日志保留。

## 10. 发布与资源加载

- npm ESM 包支持 tree-shaking；
- IIFE/UMD 提供全局 `CheetahMedia`；
- Worker/WASM/codec pack 文件使用内容哈希和 manifest；
- SDK 允许配置 assetBaseUrl 和逐资源 URL；
- 所有资源支持自托管、CSP nonce/hash 和 SRI 使用说明；
- engine 与 codec pack 执行 ABI 版本协商，不兼容时拒绝加载；
- 不在运行时从未配置的第三方域名自动下载代码。
