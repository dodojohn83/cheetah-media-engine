/**
 * WebGPU renderer.
 *
 * Prefers zero-copy external-image import for RGBA `VideoFrame`/`ImageBitmap`
 * sources. For YUV or when external import is unsupported it falls back to
 * drawing the decoded frame to a 2D offscreen canvas and uploading that to a
 * `GPUTexture`. This gives a single WebGPU presentation path while relying on
 * the existing 2D color conversion until a full shader-based YUV pipeline is
 * wired.
 */

import type { RenderFrame, Renderer, RendererConfig, RendererMetrics, SnapshotOptions, SnapshotResult } from './types';
import { RendererError } from './types';
import { RendererSurface } from './surface';
import { Canvas2DRenderer } from './canvas2d';

const VERTEX_SHADER = `@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
  // An oversized right triangle that, once clipped, covers the whole viewport.
  let x = f32(i32(vi) / 2) * 4.0 - 1.0;
  let y = f32(i32(vi) % 2) * 4.0 - 1.0;
  return vec4f(x, y, 0.0, 1.0);
}`;

const FRAGMENT_SHADER = `@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var u_texture: texture_2d<f32>;

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let uv = pos.xy / vec2f(textureDimensions(u_texture));
  return textureSample(u_texture, u_sampler, uv);
}
`;

// GPUTextureUsage numeric constants.
const TEXTURE_BINDING = 0x04;
const COPY_DST = 0x08;
const RENDER_ATTACHMENT = 0x10;
const MAP_READ = 0x01;
const BUFFER_COPY_DST = 0x0008;
const MAP_READ_MODE = 1;

function hasWebGPU(): boolean {
  return typeof navigator !== 'undefined' && 'gpu' in navigator;
}

export class WebGpuRenderer implements Renderer {
  readonly identity = 'webgpu';
  private surface: RendererSurface;
  private device: GPUDevice | undefined = undefined;
  private context: GPUCanvasContext | undefined = undefined;
  private pipeline: GPURenderPipeline | undefined = undefined;
  private bindGroup: GPUBindGroup | undefined = undefined;
  private sampler: GPUSampler | undefined = undefined;
  private frameTexture: GPUTexture | undefined = undefined;
  private fallbackRenderer: Canvas2DRenderer | undefined = undefined;
  private fallbackCanvas: OffscreenCanvas | undefined = undefined;
  private lost = false;
  private metrics: {
    framesSubmitted: number;
    framesRendered: number;
    framesDropped: number;
    snapshotsTaken: number;
    drawLatencyMs: number;
  } = {
    framesSubmitted: 0,
    framesRendered: 0,
    framesDropped: 0,
    snapshotsTaken: 0,
    drawLatencyMs: 0,
  };

  constructor(canvas: HTMLCanvasElement | OffscreenCanvas) {
    this.surface = new RendererSurface(canvas);
  }

  async configure(config: RendererConfig): Promise<void> {
    this.surface.configure(config);
    if (!hasWebGPU()) {
      throw new RendererError('no-webgpu', 'WebGPU not available');
    }
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) throw new RendererError('no-adapter', 'No WebGPU adapter');
    this.device = await adapter.requestDevice();
    this.device.lost.then((info) => {
      this.lost = true;
      this.device = undefined;
      this.pipeline = undefined;
      this.frameTexture = undefined;
      this.bindGroup = undefined;
      throw new RendererError('device-lost', info.reason);
    });

    const canvas = this.surface.getCanvas();
    const ctx =
      (canvas as HTMLCanvasElement).getContext?.('webgpu') ??
      (canvas as OffscreenCanvas).getContext?.('webgpu') ??
      null;
    if (!ctx) throw new RendererError('no-context', 'Cannot get WebGPU canvas context');
    this.context = ctx as GPUCanvasContext;
    this.context.configure({ device: this.device, format: navigator.gpu.getPreferredCanvasFormat() });

    this.sampler = this.device.createSampler({ magFilter: 'linear', minFilter: 'linear' });
    this.createPipeline();
    this.createFrameTexture();

    this.fallbackCanvas = new OffscreenCanvas(1, 1);
    this.fallbackRenderer = new Canvas2DRenderer(this.fallbackCanvas);
    await this.fallbackRenderer.configure({ canvas: this.fallbackCanvas, fit: 'fill' });
  }

  private createPipeline(): void {
    const device = this.device;
    if (!device) return;
    const shader = device.createShaderModule({ code: VERTEX_SHADER + FRAGMENT_SHADER });
    this.pipeline = device.createRenderPipeline({
      layout: 'auto',
      vertex: { module: shader, entryPoint: 'vs_main' },
      fragment: {
        module: shader,
        entryPoint: 'fs_main',
        targets: [{ format: navigator.gpu.getPreferredCanvasFormat() }],
      },
    });
  }

  private createFrameTexture(): void {
    const device = this.device;
    if (!device) return;
    const canvas = this.surface.getCanvas();
    this.frameTexture = device.createTexture({
      size: [canvas.width || 1, canvas.height || 1, 1],
      format: 'rgba8unorm',
      usage: TEXTURE_BINDING | COPY_DST | RENDER_ATTACHMENT,
    });
    this.bindGroup = device.createBindGroup({
      layout: this.pipeline!.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: this.sampler! },
        { binding: 1, resource: this.frameTexture.createView() },
      ],
    });
  }

  async render(frame: RenderFrame): Promise<void> {
    if (this.lost || !this.device) throw new RendererError('device-lost', 'WebGPU device is lost');
    const start = performance.now();
    this.metrics.framesSubmitted += 1;
    try {
      const visibleRect = RendererSurface.resolveVisibleRect(frame);
      const canvas = this.surface.getCanvas();
      const w = canvas.width;
      const h = canvas.height;
      if (w === 0 || h === 0) throw new RendererError('invalid-surface', 'Canvas has zero size');

      // Recreate the texture if the surface resized.
      if (this.frameTexture && (this.frameTexture.width !== w || this.frameTexture.height !== h)) {
        this.frameTexture.destroy();
        this.createFrameTexture();
      }

      const isRgba = (frame.format ?? '').toLowerCase() === 'rgba';
      let useFallback = !isRgba;

      if (isRgba) {
        try {
          this.copyExternalToTexture(frame as unknown as GPUCopyExternalImageSource, { width: visibleRect.width, height: visibleRect.height });
        } catch {
          useFallback = true;
        }
      }

      if (useFallback) {
        if (!this.fallbackCanvas || !this.fallbackRenderer) {
          throw new RendererError('no-fallback', 'WebGPU fallback renderer unavailable');
        }
        this.fallbackCanvas.width = visibleRect.width;
        this.fallbackCanvas.height = visibleRect.height;
        await this.fallbackRenderer.render(frame);
        this.copyExternalToTexture(this.fallbackCanvas, { width: visibleRect.width, height: visibleRect.height });
      }

      this.present();
      this.metrics.framesRendered += 1;
    } catch (err) {
      this.metrics.framesDropped += 1;
      throw err instanceof RendererError ? err : new RendererError('render-failed', String(err));
    } finally {
      this.metrics.drawLatencyMs = performance.now() - start;
    }
  }

  private copyExternalToTexture(source: GPUCopyExternalImageSource, size: { width: number; height: number }): void {
    const device = this.device;
    if (!device || !this.frameTexture) return;
    device.queue.copyExternalImageToTexture(
      { source, flipY: true },
      { texture: this.frameTexture },
      [Math.max(1, size.width), Math.max(1, size.height), 1],
    );
  }

  private present(): void {
    const device = this.device;
    const ctx = this.context;
    const texture = ctx?.getCurrentTexture();
    if (!device || !ctx || !texture || !this.pipeline || !this.bindGroup) return;
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        { view: texture.createView(), loadOp: 'clear', storeOp: 'store', clearValue: { r: 0, g: 0, b: 0, a: 1 } },
      ],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bindGroup);
    pass.draw(3);
    pass.end();
    device.queue.submit([encoder.finish()]);
  }

  async snapshot(opts: SnapshotOptions = {}): Promise<SnapshotResult> {
    // Keep opts alive for future max-size cropping; currently snapshots the full texture.
    void opts;
    if (!this.device || !this.frameTexture) throw new RendererError('not-configured', 'WebGPU renderer not configured');
    const width = this.frameTexture.width;
    const height = this.frameTexture.height;
    const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
    const buffer = this.device.createBuffer({
      size: bytesPerRow * height,
      usage: MAP_READ | BUFFER_COPY_DST,
    });
    const encoder = this.device.createCommandEncoder();
    encoder.copyTextureToBuffer(
      { texture: this.frameTexture },
      { buffer, bytesPerRow },
      [width, height, 1],
    );
    this.device.queue.submit([encoder.finish()]);
    await buffer.mapAsync(MAP_READ_MODE);
    const mapped = new Uint8Array(buffer.getMappedRange());
    const data = new Uint8ClampedArray(width * height * 4);
    for (let row = 0; row < height; row += 1) {
      for (let col = 0; col < width * 4; col += 1) {
        data[row * width * 4 + col] = mapped[row * bytesPerRow + col]!;
      }
    }
    buffer.unmap();
    this.metrics.snapshotsTaken += 1;
    return { width, height, data: new ImageData(data, width, height) };
  }

  getMetrics(): RendererMetrics {
    return { ...this.metrics };
  }

  close(): void {
    this.device?.destroy();
    this.device = undefined;
    this.context = undefined;
    this.fallbackRenderer?.close();
  }
}
