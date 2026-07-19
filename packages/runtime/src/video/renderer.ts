/**
 * Video renderer factory and high-level `VideoRenderer` that selects the
 * fastest available path (WebGPU, WebGL2, Canvas2D).
 */

import type { Renderer, RendererConfig, RenderFrame, RendererMetrics, SnapshotOptions, SnapshotResult } from './types';
import { RendererError } from './types';
import { encodeSnapshot, computeTargetSize, type CanvasLike } from './snapshot-encoder';
import { Canvas2DRenderer } from './canvas2d';
import { WebGL2Renderer } from './webgl';
import { WebGpuRenderer } from './webgpu';

export type RendererKind = 'webgpu' | 'webgl2' | 'canvas2d';

export interface VideoRendererOptions {
  /** Preferred renderer; if missing or unsupported the factory picks a fallback. */
  readonly preferred?: RendererKind | undefined;
  readonly failIfUnsupported?: boolean | undefined;
}

const VALID_RENDERER_KINDS: readonly RendererKind[] = ['webgpu', 'webgl2', 'canvas2d'];

export class VideoRenderer implements Renderer {
  readonly identity = 'video-renderer';
  private backend: Renderer | undefined = undefined;
  private kind: RendererKind | undefined = undefined;
  private closed = false;

  constructor(private readonly options: VideoRendererOptions = {}) {
    if (options.preferred !== undefined && !VALID_RENDERER_KINDS.includes(options.preferred)) {
      throw new RendererError('invalid-config', `Unknown renderer preferred: ${options.preferred}`);
    }
    if (options.failIfUnsupported !== undefined && typeof options.failIfUnsupported !== 'boolean') {
      throw new RendererError('invalid-config', 'failIfUnsupported must be a boolean');
    }
  }

  async configure(config: RendererConfig): Promise<void> {
    if (this.closed) throw new RendererError('closed', 'VideoRenderer is closed');

    const preferred = this.options.preferred ?? 'webgpu';
    const candidates: RendererKind[] =
      preferred === 'canvas2d' ? ['canvas2d'] : preferred === 'webgl2' ? ['webgl2', 'canvas2d'] : ['webgpu', 'webgl2', 'canvas2d'];

    const canvas = config.canvas;
    const originalWidth = canvas.width;
    const originalHeight = canvas.height;
    for (const kind of candidates) {
      // Reset canvas CSS dimensions before each attempt so a failed renderer's
      // DPR scaling does not compound across fallback candidates.
      canvas.width = originalWidth;
      canvas.height = originalHeight;
      try {
        await this.tryConfigure(kind, config);
        return;
      } catch (err) {
        // A partially configured backend (e.g. lost WebGPU/WebGL context) may
        // hold GPU resources; release it before trying the next fallback.
        this.backend?.close();
        this.backend = undefined;
        this.kind = undefined;
        if (this.options.failIfUnsupported) {
          throw err instanceof RendererError ? err : new RendererError('configure-failed', String(err));
        }
      }
    }

    throw new RendererError('no-renderer', 'No video renderer could be configured');
  }

  private async tryConfigure(kind: RendererKind, config: RendererConfig): Promise<void> {
    this.kind = kind;
    if (kind === 'webgpu') {
      this.backend = new WebGpuRenderer(config.canvas);
    } else if (kind === 'webgl2') {
      this.backend = new WebGL2Renderer(config.canvas);
    } else {
      this.backend = new Canvas2DRenderer(config.canvas);
    }
    await this.backend.configure(config);
  }

  async render(frame: RenderFrame): Promise<void> {
    if (this.closed) throw new RendererError('closed', 'VideoRenderer is closed');
    if (!this.backend) throw new RendererError('not-configured', 'configure() must be called before render()');
    await this.backend.render(frame);
  }

  async snapshot(opts: SnapshotOptions = {}): Promise<SnapshotResult> {
    if (this.closed) throw new RendererError('closed', 'VideoRenderer is closed');
    if (!this.backend) throw new RendererError('not-configured', 'configure() must be called before snapshot()');
    const result = await this.backend.snapshot(opts);
    if (opts.format) {
      const { width, height } = computeTargetSize(result.width, result.height, opts.maxWidth, opts.maxHeight);
      const blob = await encodeSnapshot(result.data as ImageData | CanvasLike, {
        format: opts.format,
        quality: opts.quality,
        maxWidth: opts.maxWidth,
        maxHeight: opts.maxHeight,
        includeOverlay: opts.includeOverlay,
      });
      return { width, height, data: blob };
    }
    return result;
  }

  get currentKind(): RendererKind | undefined {
    return this.kind;
  }

  getMetrics(): RendererMetrics {
    if (!this.backend) {
      return {
        framesSubmitted: 0,
        framesRendered: 0,
        framesDropped: 0,
        snapshotsTaken: 0,
        drawLatencyMs: 0,
      };
    }
    return this.backend.getMetrics();
  }

  close(): void {
    this.closed = true;
    this.backend?.close();
  }
}

export function createRenderer(options?: VideoRendererOptions): Renderer {
  return new VideoRenderer(options);
}
