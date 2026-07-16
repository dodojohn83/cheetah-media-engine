# VR / AI 扩展入口

本文档说明 Web SDK 为 VR/360° 渲染与 AI 帧处理预留的扩展接口。当前里程碑（WP-51c）只实现**入口和默认占位**，不内置完整的 360° 全景渲染器或机器学习推理管线。

## 1. 设计目标

- 允许第三方插件在不修改核心播放器的情况下接入 VR 渲染或 AI 分析能力。
- 播放器只负责暴露统一的注册接口、传递解码帧、提供预算信息，并汇报 `vrActive` / `aiActive` 状态。
- 默认实现为无操作（no-op），不影响普通 2D 播放路径的性能。

## 2. 公共接口

### 2.1 VR 渲染器

```ts
export interface VrRenderer {
  readonly active: boolean;
  initialize(surface: HTMLCanvasElement | OffscreenCanvas, metadata: VrProjectionMetadata): boolean;
  render(frame: ProcessableFrame): void;
  destroy(): void;
}
```

- `VrProjectionMetadata` 中的 `projection` 可以是 `equirectangular`、`cubemap` 或 `flat`。
- `NoopVrRenderer` 默认返回 `active = false`，`initialize` 返回 `false`，`render` 为空实现。
- 真实实现应在检测到 360° metadata 且浏览器支持 `WebGL`/`WebGPU` 时才返回 `active = true`；播放器不替代实现做这一判断。

### 2.2 AI 帧处理器

```ts
export interface AiFrameProcessor {
  readonly active: boolean;
  initialize(): boolean;
  process(frame: ProcessableFrame, budget: AiFrameBudget): AiFrameResult | undefined | Promise<AiFrameResult | undefined>;
  destroy(): void;
}
```

- `AiFrameBudget` 提供 `deadlineMs` 和 `canAllocate`（是否有额外 CPU/GPU 预算）。
- `NoopAiFrameProcessor` 默认 `active = false`，`process` 始终返回 `undefined`（skip）。
- 真实实现应在预算不足（`canAllocate: false` 或 `deadlineMs` 太小）时主动返回 `undefined`，避免拖慢渲染管线。

### 2.3 帧类型

```ts
export interface ProcessableFrame {
  readonly width: number;
  readonly height: number;
  readonly timestampMs: number;
  readonly source: HTMLVideoElement | HTMLCanvasElement | OffscreenCanvas | ImageBitmap | VideoFrame;
}
```

`source` 可以是普通 DOM 元素或 WebCodecs `VideoFrame`，实现方需自行判断可处理的类型。

## 3. 播放器 API

```ts
player.setVrRenderer(renderer: VrRenderer): void;
player.setAiProcessor(processor: AiFrameProcessor): void;
```

- 调用 `setVrRenderer` / `setAiProcessor` 会**销毁当前**已注册的实现，再替换为新实现。
- 传入 `null`/`undefined` 会回退到默认的 `NoopVrRenderer` / `NoopAiFrameProcessor`。
- 在 `destroy()` 时播放器会主动调用 `destroy()` 释放扩展资源。

状态查询：

```ts
player.vrActive: boolean; // 当前 VR 渲染器是否 active
player.aiActive: boolean; // 当前 AI 处理器是否 active
```

这两个属性**不代表浏览器原生支持 VR/AI**，仅表示当前已注册的扩展是否报告自己处于活动状态。

## 4. 激活条件与预算

- VR：建议插件在以下条件下才返回 `active = true`：
  1. 流 metadata 包含 360°/equirectangular/cubemap 标记；
  2. 浏览器支持 `WebGLRenderingContext` 或 `GPU`（WebGPU）。
- AI：建议插件在以下条件下才返回 `active = true`：
  1. 模型已加载并可用；
  2. 当前帧 `budget.canAllocate` 为 `true` 且 `budget.deadlineMs` 足够完成推理。

播放器只负责传递 `AiFrameBudget`，不做模型加载、不管理模型缓存。

## 5. 默认占位行为

- `NoopVrRenderer` 和 `NoopAiFrameProcessor` 不会分配 GPU/ML 资源，不会阻塞帧管线。
- 默认状态下 `vrActive` 和 `aiActive` 均为 `false`。
- 它们可以作为不启用扩展时的兜底，也可以作为自定义实现的基类。

## 6. 未来工作

- 当 `EngineRuntime` 暴露 `requestVideoFrameCallback` 或 `VideoFrame` 事件时，把 `ProcessableFrame` 分发给已注册的 VR/AI 扩展。
- 在 `PlayerStats` / 遥测中增加 `vrActive` / `aiActive` 和每帧处理耗时指标。
- 提供示例 `WebGLVrRenderer` 与 `WebGpuAiProcessor` 作为参考实现（后续 WP）。
