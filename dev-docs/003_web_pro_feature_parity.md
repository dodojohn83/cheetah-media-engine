# Cheetah Media Engine Web Pro 功能等价执行计划

## 1. 文档定位

本文件把 [`001` Phase 4](../001_next_generation_media_engine/11_implementation_roadmap.md) 和 [`07_jessibuca_pro_feature_parity.md`](../001_next_generation_media_engine/07_jessibuca_pro_feature_parity.md) 转换为外部编程执行体可直接执行的工作包。它从 `dev-docs/002_vibe_coding_plan` 的完成状态继续，目标是在当前 Web v1 骨架上补齐 Jessibuca Pro 公开能力等价项。

执行体发现契约缺失时，必须先补充文档和测试，不得自行发明兼容行为或虚假声明 Web Pro 完成。

## 2. 起始基线

- `wp/16-engine-state-machine` 已合并 002 全部 35 个 WP。
- Web v1 已建立 engine/runtime/bindings/SDK/components 骨架、容器解析（FLV/MPEG-TS/ISOBMFF）、HLS client、timeline/planner、WebCodecs/MSE/WASM 回退和测试套件。
- 真实端到端回放、硬件-bound 性能门禁和三仓发布演练受外部媒体端点、浏览器能力和 npm 凭证限制，已在 002 已知限制中记录，不进入 Phase 4 的虚假完成声明。

## 3. 三仓边界

| 仓库 | 路径 | 职责 |
| --- | --- | --- |
| 共享核心 | `../cheetah-media-core-rs` | 纯媒体类型、bitstream、容器、HLS、时间线、pipeline 模型、ABI、fixture |
| 播放引擎 | 当前仓库 | engine、Web bindings/runtime/SDK/components、codec packs、demo、浏览器和性能测试 |
| 媒体服务 | `../cheetah-media-server-rs` | 通过 `cheetah-codec` 兼容门面消费共享核心；保留服务端会话和协议驱动 |

依赖只能从产品仓库指向共享核心。共享核心不得依赖浏览器、UI、服务端 session、网络 driver 或任一产品仓库。

## 4. 执行规则

1. 严格按 001 Phase 4 依赖顺序实施；阻塞项未关闭时不得提前完成下游任务。
2. 每个任务编号对应一个可独立评审、测试和回滚的 PR 工作包；不得把多个阶段压进不可审查的大 PR。
3. 完成 `[ ]` 后改为 `[x]`，并追加仓库、commit/tag、测试命令、结果和证据路径。
4. 公共契约和 contract test 先于 adapter。三仓变更按 core tag → server facade → engine 的顺序落地。
5. 禁止 `todo!()`、`unimplemented!()`、空 provider、吞错、HTTP/Promise 假成功。暂不支持能力返回稳定 `Unsupported`。
6. 热路径禁止 JSON/Base64 媒体负载、逐帧 wasm-bindgen 对象和无界队列；所有缓存、池、批次、重试和并发必须有上限。
7. 任何改变 crate 图、ABI、公开 TypeScript API、错误语义、复制预算或 001 边界的实现，先修改 001/003 并经评审。
8. 外部代码和 fixture 必须登记来源 commit、许可证、修改和脱敏方式。
9. CI 不得依赖未提交的本地 path。跨仓开发可临时使用 `[patch]`，提交前必须改为不可变 tag/revision。
10. 性能结论必须有原始数据、环境、命令和对照；不得在无合法可复现实验时宣称优于第三方产品。

## 5. Phase 4 工作包索引

| WP | 文档 | 阶段交付 | 依赖 |
| --- | --- | --- | --- |
| 36 | 本文档 | Phase 4 执行计划与范围确认 | 002 全部 WP |
| 37 | `003/37_h264_annexb_demuxer.md` | H.264 Annex-B 裸流解析器 | bitstream 009、types 006 |
| 38 | `003/38_h265_annexb_demuxer.md` | H.265 Annex-B 裸流解析器 | 37 |
| 39 | `003/39_mpegps_demuxer.md` | MPEG-PS 容器解复用 | bitstream 009、types 006 |
| 40 | `003/40_raw_stream_transport.md` | HTTP/WS MPEG-PS/Annex-B transport 与 planner 路由 | 18、37、38、39 |
| 41 | `003/41_webtransport_skeleton.md` | WebTransport transport 骨架与 capability 探测 | 18、19 |
| 42 | `003/42_webrtc_transport.md` | WebRTC H.264/H.265 signaling/transport 骨架 | 19、40 |
| 43 | `003/43_vod_seek_speed.md` | MP4/HLS 点播、seek、倍速 (0.1/0.5/1/2/4/8/16x) | 13、14、21 |
| 44 | `003/44_frame_step.md` | 逐帧 / 逐关键帧 / 暂停显示但保持连接 | 14、43 |
| 45 | `003/45_ptz_panel.md` | PTZ 操作盘与 GB28181 命令生成 | components 027 |
| 46 | `003/46_advanced_wall.md` | 双击局部全屏、拖拽排序、不规则布局 | components 028 |
| 47 | `003/47_watermark.md` | 局部文字/图片/HTML 水印、平铺/动态/幽灵水印 | renderer 024 |
| 48 | `003/48_crypto_transforms.md` | SM4 / XOR / AES-128-CBC 解密 transform | types 006 |
| 49 | `003/49_sei_metadata.md` | SEI 提取、TS PES private data、服务端坐标 overlay | bitstream 009 |
| 50 | `003/50_microphone_intercom.md` | 麦克风采集、G.711/Opus 编码、语音对讲 | audio 023 |
| 51 | `003/51_downloader_recording.md` | 直播/回放下载器、合成录制、VR/AI 扩展入口 | 12、29 |

> 未来工作包（WebTransport/MP4/PTZ/水印等）的编号、范围和验收标准将在进入该 WP 前补充为 `003/XX_name.md`，避免在本文件中一次性编造无法验证的细节。

## 6. 全局完成定义

- [ ] 36–51 所有适用项完成，不适用项经产品评审记录原因。
- [ ] 每项有自动化测试、真实设备记录或明确人工验收步骤。
- [ ] 浏览器限制和服务器依赖作为 capability 公示，不冒充标准支持。
- [ ] 新增容器/transport 路径在共享核心中通过 contract suite，不重复 parser 逻辑。
- [ ] 性能、内存和长稳测试覆盖启用高级功能后的增量成本。

## 7. 标准验证命令

与 `002_vibe_coding_plan` 一致：

```bash
# 共享核心 / engine
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build --workspace --target wasm32-unknown-unknown --no-default-features
cargo deny check

# Web 引擎
corepack pnpm install --frozen-lockfile
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm exec playwright test
```

各任务只运行与其仓库和 feature 相关的子集，但完成证据必须记录实际完整命令。发布候选必须运行全集。

## 8. 阶段闸门

| 闸门 | 必须满足后才能进入下一阶段 |
| --- | --- |
| G5 Web Pro 核心闸门 | Annex-B/MPEG-PS/transport 新增路径 contract suite 通过；无 parser 重复 |
| G6 高级协议闸门 | WebTransport/WebRTC/VOD/seek 路径有成功和强制失败证据 |
| G7 UI/功能闸门 | PTZ/水印/倍速/逐帧/键盘/右键/区域截图通过浏览器矩阵 |
| G8 安全与扩展闸门 | SM4/XOR/AES-128/HLS AES-128/SEI/麦克风通过安全审计和门禁 |

任一闸门失败时只允许修复当前或上游任务；不得以 UI 演示、单一浏览器成功或人工观察代替门禁证据。

## 9. WP-36 Annex-B 裸流解析器范围（首个实施包）

### 9.1 目标

交付 `crates/cheetah-container-annexb`，将 Annex-B H.264/H.265 字节流解析为 `MediaPacket`，并提供参数集追踪、随机访问帧识别和 AVCC/HVCC 配置记录生成，作为后续 HTTP/WS 裸流和 WebRTC/WebTransport 视频轨的共同输入。

### 9.2 交付物

- `cheetah-container-annexb` crate，README 说明职责、允许依赖和 feature。
- `AnnexBDemuxer`：增量 `push` 接口，返回 `AnnexbEvent`（`Track`、`Packet`、`NeedMore`、`Eof` 等）。
- 支持 3 字节和 4 字节 start code 混合。
- 支持 H.264 (SPS/PPS) 和 H.265 (VPS/SPS/PPS) 参数集缓存与变更检测。
- 使用 `cheetah-media-bitstream` 现有 SPS/HEVC 解析生成 `AvcC`/`HvcC` 配置。
- 关键帧识别（H.264 IDR、H.265 IDR/CRA/BLA/IRAP）。
- 可配置的单包 NAL 大小上限和缓冲区上限，超限时返回稳定错误。

### 9.3 完成定义

- `cargo fmt/clippy/test --workspace --all-features` 通过。
- `no_std + alloc` 编译通过（`default-features = false`）。
- 无 `todo!()`/`unimplemented()`；对外部输入无 `unwrap()`/`expect()`。
- 测试覆盖 golden 正常流、参数集变更、start code 边界、畸形输入、空包、大 NAL 切片和 fuzz regression。

## 10. Phase 4 执行记录

| WP | 状态 | 分支/PR | 备注 |
| --- | --- | --- | --- |
| 36 | 已完成 | `wp/16-engine-state-machine` | Phase 4 计划文档与基线 |
| 37 | 已完成 | `wp/37-h264-annexb-demuxer` → #38 | H.264 Annex-B 解复用与 AvcC 配置 |
| 38 | 已完成 | `wp/38-h265-annexb-demuxer` → #39 | H.265 Annex-B 解复用与 HvcC 配置 |
| 39 | 已完成 | `wp/39-mpegps-demuxer` → #40 | MPEG-PS 容器解复用 |
| 40 | 已完成（40a + 40b） | `wp/40b-wasm-demuxer-bindings` → #42 | 40a planner 路由；40b WASM demuxer 绑定 |
| 41 | 已完成 | `wp/41-webtransport-skeleton` → #43 | WebTransport transport 骨架与 capability 探测 |
| 42 | 已完成 | `wp/42-webrtc-transport` → #44 | WebRTC H.264/H.265 signaling/transport 骨架 |
| 43 | 已完成 | `wp/43-vod-seek-speed` → #45 | MP4/HLS 点播、seek、倍速；43a/43b/43c 全部通过完整矩阵 |
| 44 | 已完成 | `wp/44-frame-step` -> #46 | 逐帧/暂停显示 API；MSE time-step、WebCodecs 前向步进/队列保持连接、FallbackController/Player 转发 |
| 45 | 已完成 | `wp/45-ptz-panel` -> #47 | PTZ 操作盘组件、GB28181 PtzCmd 编码、CheetahPlayer.ptz 事件；信号传输由调用方转发到 signaling 服务 |
| 46 | 已完成 | `wp/46-advanced-wall` -> #48 | 双击局部全屏、拖拽排序、不规则布局 |
| 47 | 进行中 | `wp/47-watermark` | 局部文字/图片/HTML 水印、平铺/动态/幽灵水印 |
