# 22. FFmpeg WASM Codec Packs

## FFM-001：可重复制品构建

- [ ] 固定 FFmpeg 8.1.2 source archive hash、Emscripten 6.0.2、patch 和 configure flags。
- [ ] 只构建 H.264/H.265/AAC/MP3 所需 decoder、parser、swresample/util；禁用 program/network/device/filter/muxer/demuxer。
- [ ] 明确关闭 GPL/nonfree，CI 从 configure 输出和二进制符号双重检查。
- [ ] 构建过程离线可重放，输出 source offer、NOTICE、SBOM、export list 和 hash。

## FFM-002：三种 pack variant

- [ ] `threads-simd`：SharedArrayBuffer/Pthreads/SIMD，固定最大 worker 和内存。
- [ ] `simd`：单线程 SIMD，适用于非隔离页面。
- [ ] `baseline`：单线程无 SIMD，作为最终兼容路径并公开性能限制。
- [ ] 三种 variant 使用相同 pack ABI/manifest；loader 不向上层泄漏 FFmpeg struct。

## FFM-003：decoder shim

- [ ] configure 接受共享 TrackInfo/config descriptor，send packet/receive frame 使用有界队列。
- [ ] frame plane 通过 descriptor/pool 交给 renderer；释放前 FFmpeg buffer 生命周期有效。
- [ ] flush、drain、reset、reconfigure、close 对 EAGAIN/EOF/error 有确定映射。
- [ ] 色彩、stride、sample format/channel layout 完整传递，不默认假设 I420/stereo。

## FFM-004：性能、替换和许可验收

- [ ] H.265 1080p25 在参考桌面验证实时，报告每 variant CPU、内存、decode p95 和 drop。
- [ ] loader 可用同 ABI 的 mock/replacement pack 替换 FFmpeg，不改 SDK/engine。
- [ ] pack 缺失、hash/ABI 不匹配、worker 创建失败均触发下一回退或 Unsupported。
- [ ] npm 主包不静态捆绑 codec pack，用户可选择 self-host/CDN/完全禁用。

