# WP-50: 麦克风采集、G.711/Opus 编码与语音对讲

## 1. 目标

为 Web 播放器增加本地麦克风采集、音频编码（G.711 A-law/μ-law、Opus）与语音对讲能力，作为发布路径的雏形。能力探测、权限错误、半双工/全双工策略和资源边界必须显式表达，不能冒充已支持全部浏览器。

## 2. 拆分子任务

### 50a：G.711 编码器与 WASM 绑定

- `cheetah-media-bitstream` 新增 G.711 A-law/μ-law `encode`（i16/f32 -> 8-bit companded）。
- 单元测试覆盖静音、最大正/负、对称性、round-trip（encode -> decode）。
- `cheetah-media-web-bindings` 暴露 `g711_encode` 和 `g711_encode_f32` 给 JS runtime。

### 50b：麦克风采集与 AudioWorklet 流水线

- `packages/runtime/src/audio/capture.ts`：
  - 探测 `getUserMedia` 与 `AudioWorkletNode` 能力。
  - 请求指定 sample rate / channel count / echoCancellation 的麦克风流。
  - 通过 `AudioWorkletNode` 拉取 `Float32Array` 样本，resample 到目标 sample rate。
  - 调用 WASM G.711 编码（或 WebCodecs `AudioEncoder` Opus），产出 `AudioPacket`。
- 支持 start/stop/pause/resume；权限拒绝、设备占用、`NotAllowedError` 等返回可恢复错误。
- 保持采集队列有界；取消时释放 `MediaStream` 和 `AudioContext`。

### 50c：语音对讲封装与播放器集成

- `packages/runtime/src/audio/intercom.ts`：
  - 将编码后的 G.711/Opus 帧封装为 minimal RTP/JT808 对讲 payload（或原始 PCM packet）。
  - 提供 `sendPacket` 回调；不直接依赖具体网络传输，以便复用 WebSocket/WebRTC DataChannel。
- `packages/web/src/player.ts` 新增 `startIntercom(options)` / `stopIntercom()`。
- `packages/components/src/player-element.ts` 可选增加对讲按钮与 `intercomactive` 属性。
- 测试覆盖：采集启动/停止、编码 round-trip、packetizer 边界、权限拒绝模拟。

## 3. 完成定义

- `cargo fmt/clippy/test --workspace --all-features` 通过。
- `no_std + alloc` 编译通过（bitstream crate）。
- 无 `todo!()`/`unimplemented()`；对外部输入无 `unwrap()`/`expect()`。
- JS `pnpm typecheck/test/build` 通过。
- 新增 Playwright 用例（若浏览器支持）：麦克风权限拒绝不崩溃、采集 1 秒后停止、G.711 编码包非空。
- Opus 路径在浏览器不支持时返回 `Unsupported`，不以 MSE/WebCodecs 路径冒充。

## 4. 边界

- 50a 仅提供编码器与 WASM 绑定，不连接真实麦克风。
- 50b 负责麦克风采集与本地编码，网络发送留给 50c。
- 50c 的对讲封装只输出 packet payload；传输层复用现有 `WebSocketTransport` 或 future `Publisher`，本 WP 不新增独立网络协议栈。
- 移动端后台音频、AEC/NS/AGC 精细调参列为 Future，能力矩阵中必须如实标记。

## 5. 状态

- 50a：已完成；PR 见 `wp/50a-g711-encoder`。
- 50b：已完成；PR 见 `wp/50b-microphone-capture`。
- 50c：待开始。
