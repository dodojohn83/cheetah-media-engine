# 07. Jessibuca Pro 功能等价矩阵

## 1. 目标与规则

本项目以 Jessibuca Pro 公开功能清单作为产品能力参照，但不复制其实现、不兼容其 JavaScript API，也不在缺乏同环境基线时作笼统性能比较。

状态定义：

- **Web v1**：首个生产 Web 版本必须完成；
- **Web parity**：Web Pro 功能等价阶段完成；
- **Extension**：可选扩展模块；
- **Long-term**：原生或双向引擎阶段；
- **N/A**：经产品评审明确不适用，必须记录理由。

公开能力只能声称“已验收”项。实现存在但未通过兼容和稳定性测试仍视为未完成。

## 2. 解码与回退

| 能力 | 阶段 | 验收重点 |
| --- | --- | --- |
| H.264/H.265 WebCodecs | Web v1 | 真实试解、动态配置、运行时熔断 |
| H.264/H.265 MSE | Web v1 | fMP4 重封装、H.265 能力探测、buffer 治理 |
| H.264/H.265 WASM baseline | Web v1 | 单路兼容、错误流不崩溃 |
| WASM SIMD | Web v1 | 1080p25 桌面软解基线 |
| WASM Threads+SIMD | Web v1 | COOP/COEP、共享内存和线程回收 |
| MPEG-4 Part 2 软解 | Web parity | codec pack 与兼容语料 |
| AV1 硬解 | Web parity | WebCodecs/MSE 能力矩阵 |
| VP8/VP9 | Web parity | WebCodecs 与 WebRTC 后续路径 |
| 前后台不累积延迟 | Web v1 | 后台停解码、前台追最新关键帧 |
| 解码失败自动回退 | Web v1 | 故障注入和无重复网络管线 |

## 3. 渲染与画面

| 能力 | 阶段 | 说明 |
| --- | --- | --- |
| WebGPU 渲染 | Web v1 | VideoFrame external texture、软解 YUV shader |
| WebGL2 渲染 | Web v1 | WebGPU fallback |
| Canvas2D 渲染 | Web v1 | 最低兼容或截图路径 |
| `<video>` 渲染 | Web v1 | MSE 主路径 |
| OffscreenCanvas | Web v1 | 实际浏览器支持时启用 |
| 填充/等比/裁剪 | Web v1 | renderer transform |
| 旋转、水平/垂直镜像 | Web v1 | 不复制 Frame |
| 动态分辨率 | Web v1 | decoder/texture 安全重建 |
| 电子放大、区域圈选 | Web parity | 坐标转换和截图 |
| VR/全景 | Extension | 独立 renderer module |

## 4. 输入协议与封装

| 能力 | 阶段 |
| --- | --- |
| HTTP/WS-FLV | Web v1 |
| HLS TS/fMP4、LL-HLS | Web v1 |
| HTTP/WS-fMP4 | Web v1 |
| WebRTC H.264/H.265 | Web parity |
| WebTransport | Web parity |
| HTTP/WS MPEG-TS | Web parity |
| HTTP/WS MPEG-PS | Web parity |
| HTTP/WS 裸 H.264/H.265 | Web parity |
| MP4/HLS 点播 | Web parity |
| 厂商 RTC/私有协议 | Extension |

服务器或浏览器不支持的原生 WebRTC H.265 必须返回能力不足，不以 DataChannel 私有传输冒充标准 WebRTC。

## 5. 音频与通信

| 能力 | 阶段 |
| --- | --- |
| AAC、G.711A/U、MP3 播放 | Web v1 |
| AudioWorklet | Web v1 |
| 纯音频/单音频轨 | Web parity |
| 移动端后台音频 | Web parity，受平台策略约束 |
| 麦克风采集 | Web parity |
| PCM/G.711A/U 编码 | Web parity |
| RTP/JTT 等对讲封装 | Web parity/Extension |
| 完整双向音视频发布 | Long-term |

浏览器 autoplay、后台和息屏限制必须如实暴露，不能承诺绕过平台策略。

## 6. 播放控制与 UI

| 能力 | 阶段 |
| --- | --- |
| 播放、暂停、停止、音量、静音、全屏 | Web v1 |
| 自定义控制条样式和按钮 | Web v1 |
| loading、错误、重试和性能面板 | Web v1 |
| 键盘快捷键、右键菜单 | Web parity |
| 国际化 | Web v1 基础，Web parity 完整 |
| 1/4/9/16 宫格 | Web v1 |
| 双击局部全屏、拖拽排序、不规则布局 | Web parity |
| PTZ 操作盘和国标命令生成 | Web parity |
| 流分辨率选择与展示 | Web v1 基础，Web parity 扩展 |

## 7. 直播稳定性

| 能力 | 阶段 |
| --- | --- |
| 最大延迟阈值和主动追赶 | Web v1 |
| 首帧非 I 帧过滤 | Web v1 |
| 加载超时、断流检测、有界重试 | Web v1 |
| 网络延迟检测 | Web v1 |
| 重连期间保留最后一帧 | Web parity |
| pause/play 保留最后一帧 | Web v1 |
| 弱网 WebRTC 策略 | Web parity |
| 可视区域和焦点调度 | Web v1 |
| 播放异常诊断包 | Web v1 |

“绝不累积延迟”通过有界队列和允许丢帧实现，不承诺在保留每一帧的同时维持实时。

## 8. 回放与点播

| 能力 | 阶段 |
| --- | --- |
| MP4/HLS 点播 | Web parity |
| 0.1/0.5/1/2/4/8/16 倍速 | Web parity |
| 逐帧/逐关键帧 | Web parity |
| GB28181 TF 卡录像流 | Web parity |
| JT/T 1078 TF 卡录像流 | Web parity |
| 24 小时/固定时长进度条 | Web parity |
| 暂停显示但保持连接 | Web parity |
| 特殊回放流 | Extension |

## 9. 截图、录制与下载

| 能力 | 阶段 |
| --- | --- |
| PNG/JPEG/WebP 截图 | Web v1 |
| MP4/fMP4 原码流录制 | Web v1 |
| FLV 原码流录制 | Web v1 |
| WritableStream/File System Access | Web v1 |
| 直播/回放下载器 | Web parity |
| 带覆盖层的合成录制 | Extension |
| 区域截图 | Web parity |
| 截图文字/图片/暗水印 | Web parity/Extension |

## 10. 水印与加密

| 能力 | 阶段 |
| --- | --- |
| 局部文字/图片/HTML 水印 | Web parity |
| 平铺、动态、幽灵水印 | Web parity |
| 截图数字暗水印 | Extension |
| M7S/私有加密 | Web parity |
| XOR | Web parity |
| SM4 | Web parity |
| HLS AES-128-CBC | Web parity |

解密在 demux 前的独立有界 transform 完成；密钥不进入日志、stats、URL 回显或诊断包。

## 11. 元数据与 AI

| 能力 | 阶段 |
| --- | --- |
| SEI 提取和事件回调 | Web parity |
| TS PES private data | Web parity |
| 服务端坐标/图形 overlay | Web parity |
| 大疆等厂商 SEI profile | Extension |
| 人脸/物品识别 | Extension |
| 黑屏、绿屏、花屏、马赛克检测 | Extension |
| 遮挡检测 | Extension |

AI 使用可插拔 FrameProcessor，不允许默认强制 CPU readback 或拖慢播放主路径。预算不足时 AI 必须降帧或独立关闭。

## 12. 完成定义

Web parity 完成必须满足：

- 官网公开功能逐项映射，未实现项没有模糊描述；
- 每个已完成项有自动化测试、真实设备记录或明确人工验收步骤；
- 浏览器限制和服务器依赖作为 capability 公示；
- 性能、内存和长稳测试覆盖启用高级功能后的增量成本；
- 不适用项经产品评审记录原因，不通过改名隐藏缺失能力。
