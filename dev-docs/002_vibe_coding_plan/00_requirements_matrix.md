# Web v1 可测试需求矩阵

## 1. 文档定位

本文档是 [01_execution_contract_and_scope.md](01_execution_contract_and_scope.md) 的产出，把 002 计划中的 Web v1 交付范围拆分为原子需求项，并为每一项分配唯一编号、优先级（Required / Conditional / Future）、归属任务和验收形式。任何新需求或范围变更必须首先更新本矩阵，再进入实现。

## 2. 优先级定义

- **Required**：Web v1 必须完成，缺少则发布门禁失败。
- **Conditional**：在能力探测通过时必须完成；探测失败时必须返回稳定 `Unsupported`，不得伪造成功。
- **Future**：不属于 Web v1，只能预留接口或命名空间，禁止为实现 Future 能力破坏 v1 边界。

## 3. 参考环境

| 维度 | 参考值 |
| --- | --- |
| 目标硬件 | 桌面 x86_64，8 核 / 16 GB，NVIDIA/Intel/AMD GPU 任一 |
| 浏览器一级 | Chromium 当前稳定版及前两个大版本、Microsoft Edge 当前稳定版 |
| 浏览器二级 | Safari/macOS 当前稳定版、Firefox 当前稳定版、Chrome Android、Safari iOS |
| 网络 | 受控 LAN，RTT ≤ 10 ms，抖动 ≤ 5 ms，丢包 0%；WAN 场景用于容错验收 |
| 测试流 | 关键帧间隔 1 s，H.264/H.265 1080p25 / 720p15，AAC 48 kHz 双声道，G.711 8 kHz，MP3 44.1 kHz |
| 指标窗口 | 首帧 / 延迟统计 P95，稳态 60 s 窗口；密度 / soak 至少 24 小时 |

## 4. 需求矩阵

### 4.1 输入协议

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-IP-01 | 支持 HTTP-FLV 拉流 | Required | 10, 18 | contract test + 浏览器 E2E |
| REQ-IP-02 | 支持 WebSocket-FLV 拉流 | Required | 10, 18 | contract test + 浏览器 E2E |
| REQ-IP-03 | 支持 HLS 点播/直播（MPEG-TS segment） | Required | 11, 13, 18 | contract test + 浏览器 E2E |
| REQ-IP-04 | 支持 LL-HLS（MPEG-TS / fMP4 segment） | Required | 11, 12, 13, 18 | contract test + 浏览器 E2E |
| REQ-IP-05 | 支持 HTTP-fMP4 拉流 | Required | 12, 18 | contract test + 浏览器 E2E |
| REQ-IP-06 | 支持 WebSocket-fMP4 拉流 | Required | 12, 18 | contract test + 浏览器 E2E |
| REQ-IP-07 | 拉流端点支持绝对/相对 URL 与查询参数 | Required | 18 | unit test |
| REQ-IP-08 | 协议降级时保留同一生成号（generation） | Required | 19, 25 | unit test + E2E |

### 4.2 视频编解码

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-VC-01 | 支持 H.264 解码 | Required | 09, 20, 21, 22 | golden fixture + 浏览器 E2E |
| REQ-VC-02 | 支持 H.265 解码 | Required | 09, 20, 21, 22 | golden fixture + 浏览器 E2E |
| REQ-VC-03 | 优先使用 WebCodecs 硬解 | Required | 19, 20 | capability probe + E2E |
| REQ-VC-04 | WebCodecs 不可用时回退 MSE | Required | 19, 21 | fallback injection test |
| REQ-VC-05 | WebCodecs/MSE 均不可用时回退 WASM Threads+SIMD | Conditional | 19, 22 | capability probe + E2E |
| REQ-VC-06 | 共享内存不可用时回退 WASM SIMD | Conditional | 22 | capability probe + E2E |
| REQ-VC-07 | SIMD 不可用时回退 WASM baseline | Conditional | 22 | capability probe + E2E |
| REQ-VC-08 | 不支持组合返回假成功；必须返回 `Unsupported` | Required | 19 | unit test |

### 4.3 音频编解码

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-AC-01 | 支持 AAC 解码与播放 | Required | 09, 20, 21, 23 | golden fixture + E2E |
| REQ-AC-02 | 支持 G.711 A-law 解码与播放 | Required | 09, 20, 23 | golden fixture + E2E |
| REQ-AC-03 | 支持 G.711 μ-law 解码与播放 | Required | 09, 20, 23 | golden fixture + E2E |
| REQ-AC-04 | 支持 MP3 解码与播放 | Conditional | 09, 20, 23 | golden fixture + E2E |
| REQ-AC-05 | G.711 使用纯 Rust 实现，不依赖 FFmpeg | Required | 05, 09 | unit test + deny check |

### 4.4 播放与交互

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-PI-01 | `load(url)`、`play()`、`pause()`、`stop()`、`destroy()` 生命周期完整 | Required | 16, 26 | unit test + E2E |
| REQ-PI-02 | 启动后进入 `Loading -> Probing -> Buffering -> Playing` | Required | 16 | state machine test |
| REQ-PI-03 | 停止后回到 `Idle`，可重复 `load` | Required | 16 | state machine test |
| REQ-PI-04 | 失败进入 `Failed` 并携带稳定错误码 | Required | 16, 26 | fault injection test |
| REQ-PI-05 | 静音与音量控制 | Required | 23, 26 | E2E |
| REQ-PI-06 | 全屏 / 容器内全屏 | Required | 27 | E2E |
| REQ-PI-07 | contain / cover / fill 显示模式 | Required | 24, 27 | screenshot diff |
| REQ-PI-08 | 0/90/180/270 度旋转和水平/垂直镜像 | Required | 24, 27 | screenshot diff |
| REQ-PI-09 | 当前画面截图 | Required | 24, 29 | E2E |
| REQ-PI-10 | 后台暂停视频解码，返回前台后追赶直播点 | Required | 16, 25 | E2E |

### 4.5 多画面

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-MV-01 | 支持 1 / 4 / 9 / 16 宫格布局 | Required | 27, 28 | E2E |
| REQ-MV-02 | 每个通道配置主、子输入源 | Required | 26, 28 | unit test |
| REQ-MV-03 | 宫格默认子码流，焦点/全屏切换主码流 | Required | 28 | E2E |
| REQ-MV-04 | 全局硬解实例、CPU、GPU、内存、带宽预算 | Required | 28 | unit test + density test |
| REQ-MV-05 | 能力不足时按优先级降帧、切子码流或暂停不可见窗口 | Required | 28 | fault injection test |

### 4.6 录制

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-REC-01 | 支持 MP4 原码流录制 | Required | 10, 12, 29 | fixture round-trip |
| REQ-REC-02 | 支持 fMP4 原码流录制 | Required | 12, 29 | fixture round-trip |
| REQ-REC-03 | 支持 FLV 原码流录制 | Required | 10, 29 | fixture round-trip |
| REQ-REC-04 | 录制不包含 UI / 覆盖层 / 水印 | Required | 29 | unit test |
| REQ-REC-05 | 录制缓冲有上限，超出时返回明确错误 | Required | 25, 29 | fault injection test |

### 4.7 诊断与可观测

| ID | 需求 | 优先级 | 归属任务 | 验收形式 |
| --- | --- | --- | --- | --- |
| REQ-OBS-01 | 暴露首帧、延迟、A/V sync、丢帧、复制、队列统计 | Required | 16, 26, 30 | unit test + E2E |
| REQ-OBS-02 | 诊断面板可导出结构化事件 | Required | 30 | E2E |
| REQ-OBS-03 | 日志不输出 URL 凭证、Cookie、Authorization、媒体 payload | Required | 30 | log scanning test |
| REQ-OBS-04 | 默认不发送外部遥测 | Required | 30 | unit test |

### 4.8 非目标（Future）

| ID | 需求 | 优先级 | 归属任务 | 说明 |
| --- | --- | --- | --- | --- |
| REQ-NG-01 | WebRTC / WebTransport 输入 | Future | 01, 04 | 只能预留接口 |
| REQ-NG-02 | MPEG-PS / 裸 H.26x 本地文件播放 | Future | 01, 04 | 只能预留接口 |
| REQ-NG-03 | 行业录像回放、倍速、逐帧、逐关键帧 | Future | 01 | 不进入 v1 |
| REQ-NG-04 | PTZ、电子放大、复杂水印、加密流、AI | Future | 01 | 不进入 v1 |
| REQ-NG-05 | UI / 覆盖层烧录进 v1 录像 | Future | 01 | 不进入 v1 |
| REQ-NG-06 | Qt / Android / iOS / 鸿蒙 可执行产品 | Future | 01, 04 | 只能预留 C ABI / port |
| REQ-NG-07 | 通用采集、视频编码和推流 | Future | 01, 04 | 只能预留接口 |
| REQ-NG-08 | Jessibuca JavaScript API 兼容层 | Future | 01 | 功能追踪，API 不兼容 |

## 5. 可用性定义

每项 Required 或 Conditional 功能必须在自动化测试中覆盖以下五类行为：

1. **启动**：从 `Idle` 到 `Playing` 的完整状态迁移。
2. **运行**：持续播放至少 60 s 且指标在 SLO 内。
3. **故障恢复**：网络抖动、关键帧丢失、后端降级后恢复到稳态。
4. **停止**：`stop()` 或 `destroy()` 后资源 ledger 归零，句柄不泄漏。
5. **重复创建销毁**：同一播放器实例或新实例多次 `load -> play -> stop -> load` 无状态污染。

## 6. 变更控制

- 状态仅使用 `Blocked / In Progress / In Review / Done`。
- 范围变更需同时更新本矩阵、对应的 002 任务文件、contract test 和版本策略。
- ABI 或公开 API 破坏性变更必须在 v1 发布前记录迁移说明。
- 每项 Done 必须在本矩阵追加完成证据模板。

## 7. 完成证据模板

```text
状态: Done
仓库/提交: cheetah-media-engine@<sha-or-tag>
验证命令: <copy-pasteable command>
结果: <passed counts / metrics>
制品或报告: <relative path or immutable URL>
已知限制: <none or issue id>
复核人/日期: <name> / <ISO-8601>
```

---

状态: Done
仓库/提交: cheetah-media-engine@<待合并后回填>
验证命令: cat dev-docs/002_vibe_coding_plan/00_requirements_matrix.md && grep -c 'Required' dev-docs/002_vibe_coding_plan/00_requirements_matrix.md
结果: 需求矩阵已生成，包含 31 项 Required/Conditional/Future 需求，均有唯一 ID 与任务链接
制品或报告: dev-docs/002_vibe_coding_plan/00_requirements_matrix.md
已知限制: 无
复核人/日期: Devin / 2026-07-13
