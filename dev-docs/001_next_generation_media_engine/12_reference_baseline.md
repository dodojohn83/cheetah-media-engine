# 12. 标准与技术参考基线

## 1. 使用规则

本文件记录设计阶段使用的公开基线。实现每个 Phase 前必须复核标准、浏览器实现、crate、工具链和参考项目的实际版本，并把结果写入 release/compatibility matrix。

标准原文优先于开源实现。参考实现用于理解工程方案、兼容场景和测试维度，不能在未检查许可证时复制代码、二进制或 fixture。

本基线核对日期：2026-07-13。

## 2. Web 媒体标准

- [W3C WebCodecs](https://www.w3.org/TR/webcodecs/)：VideoDecoder、AudioDecoder、EncodedVideoChunk、VideoFrame、资源引用和能力提示。
- [W3C Media Source Extensions 2](https://www.w3.org/TR/media-source-2/)：MediaSource、SourceBuffer、DedicatedWorker MediaSource 和 buffer 模型。
- [W3C MSE Byte Stream Format Registry](https://www.w3.org/TR/mse-byte-stream-format-registry/)：MSE 可接受封装格式入口。
- [WebAssembly Core Specification](https://webassembly.github.io/spec/core/)：WASM 基础语义。
- [WebAssembly Threads](https://github.com/WebAssembly/threads)：共享内存和原子扩展基线。
- [WebAssembly SIMD](https://github.com/WebAssembly/simd)：SIMD 扩展基线。
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)：GPU device、texture 和 external image 处理。
- [WHATWG HTML](https://html.spec.whatwg.org/)：HTMLMediaElement、页面生命周期和媒体 autoplay 行为。

浏览器 API 存在、`isConfigSupported()` 或 `isTypeSupported()` 返回 true 都不能替代真实参数集/关键帧试运行。

## 3. 媒体格式与协议

- [ISO Base Media File Format overview](https://www.iso.org/standard/83102.html)：MP4/fMP4 基础标准入口，具体采用版本在实现阶段记录。
- [RFC 8216: HTTP Live Streaming](https://www.rfc-editor.org/rfc/rfc8216)
- [Apple HLS Authoring Specification](https://developer.apple.com/documentation/http-live-streaming/hls-authoring-specification-for-apple-devices)
- [Adobe Flash Video File Format Specification](https://rtmp.veriskope.com/pdf/video_file_format_spec_v10.pdf)：FLV 历史格式参考。
- [ISO/IEC 13818-1 MPEG-2 Systems](https://www.iso.org/standard/87619.html)：MPEG-TS/PS 标准入口。
- [ITU-T H.264](https://www.itu.int/rec/T-REC-H.264)
- [ITU-T H.265](https://www.itu.int/rec/T-REC-H.265)
- [RFC 6381: MIME Codecs Parameter](https://www.rfc-editor.org/rfc/rfc6381)

受版权限制的标准文本只按合法获取版本实施，不将未授权标准全文提交仓库。

## 4. 浏览器与平台资料

- [Chrome WebCodecs](https://developer.chrome.com/docs/web-platform/best-practices/webcodecs)
- [Chrome WebGPU](https://developer.chrome.com/docs/web-platform/webgpu/)
- [WebKit Blog](https://webkit.org/blog/)：Safari/WebKit 媒体能力发布记录。
- [MDN Web APIs](https://developer.mozilla.org/docs/Web/API)：仅用于开发者兼容资料，规范冲突时以标准和实测为准。
- [Microsoft Media Foundation](https://learn.microsoft.com/windows/win32/medfound/microsoft-media-foundation-sdk)
- [Android MediaCodec](https://developer.android.com/reference/android/media/MediaCodec)
- [Apple VideoToolbox](https://developer.apple.com/documentation/videotoolbox)
- [Qt Platform Abstraction](https://doc.qt.io/qt-6/qpa.html)

鸿蒙 API、工具链和应用市场规则在进入对应 Phase 时按目标系统官方资料单独冻结。

## 5. 产品与参考实现

- [Jessibuca Pro 功能说明](https://jessibuca.com/pro.html)：功能等价矩阵的公开产品基线。
- [Jessibuca 文档](https://jessibuca.com/document)：兼容、延迟、录制和常见故障场景参考。
- [zhaohappy/libmedia](https://github.com/zhaohappy/libmedia)：共享内存、FFmpeg 风格媒体模型、多 Worker、WebCodecs/WASM 和 WebGPU 设计参考；许可证为 LGPL-3.0。
- [ChungTak/cheetah-media-server-rs-dev](https://github.com/ChungTak/cheetah-media-server-rs-dev)：共享媒体、容器和协议兼容能力的迁移来源。
- [FFmpeg](https://ffmpeg.org/)：软解和媒体兼容参考；实际启用组件与许可证必须由构建清单冻结。

参考 libmedia 时只吸收架构思想和公开行为，不直接依赖其播放器核心，也不把 TypeScript 媒体模型作为跨平台权威实现。

## 6. Rust 与构建基线

- [Rust 官方发布记录](https://blog.rust-lang.org/releases/)；
- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)；
- [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen)；
- [wasm-pack](https://github.com/rustwasm/wasm-pack)；
- [Binaryen](https://github.com/WebAssembly/binaryen)；
- [Emscripten](https://emscripten.org/)：FFmpeg/codec pack 候选构建工具链。

设计审计时 `cheetah-media-server-rs-dev` 固定了无法从当前镜像下载的 Rust `1.96.1`。本项目不得复制该版本号；Phase 0 必须选择实际发布、能构建所有 target 且通过 CI 的 stable，并同步 rust-toolchain、CI、容器和 SBOM。

## 7. 许可证基线

- [GNU LGPL v3](https://www.gnu.org/licenses/lgpl-3.0.html)
- [FFmpeg Legal](https://ffmpeg.org/legal.html)
- [OpenH264 License](https://github.com/cisco/openh264/blob/master/LICENSE)
- [MPEG LA AVC/H.264](https://www.via-la.com/licensing-2/avc-h-264/)
- [Access Advance HEVC](https://accessadvance.com/hevc-advance-patent-pool/)

许可证和专利链接只作为工程风险入口，不构成法律意见。正式分发前必须由合格人员按产品、地区和商业模式复核。

## 8. 版本冻结要求

进入 Phase 0 必须生成机器可读清单：

- Rust、LLVM、wasm-bindgen、wasm-opt、Emscripten 和 Node 工具链；
- TypeScript、bundler、test runner 和浏览器自动化版本；
- FFmpeg/decoder commit、configure flags、patch 和许可证；
- engine ABI、codec pack ABI、共享核心 tag；
- Chrome、Edge、Firefox、Safari 及操作系统版本；
- Windows HEVC 组件、GPU driver 和参考硬件；
- Android/iOS/鸿蒙 target SDK 与最低版本；
- 协议标准和测试 fixture 版本。

升级清单中的关键版本必须运行 ABI contract、协议 fixture、浏览器矩阵、许可证检查、性能回归和至少一次 soak，不能只依赖 SemVer 或构建成功。
