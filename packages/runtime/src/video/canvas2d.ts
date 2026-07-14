/**
 * Canvas 2D fallback renderer. Draws a `RenderFrame` (e.g. a WebCodecs VideoFrame)
 * to the canvas via `drawImage`. Also used as a diagnostic/compat path.
 */

import type {
  RenderFrame,
  Renderer,
  RendererConfig,
  RendererMetrics,
  MutableRendererMetrics,
  SnapshotOptions,
  SnapshotResult,
} from './types';
import { RendererError } from './types';
import { RendererSurface } from './surface';

export class Canvas2DRenderer implements Renderer {
  readonly identity = 'canvas2d';
  private surface: RendererSurface;
  private ctx: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D | null = null;
  private configured = false;
  private metrics: MutableRendererMetrics = {
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
    this.ctx = this.surface.getContext2d();
    if (!this.ctx) {
      throw new RendererError('no-context', 'Canvas 2D context not available');
    }
    this.configured = true;
  }

  async render(frame: RenderFrame): Promise<void> {
    if (!this.configured || !this.ctx) {
      throw new RendererError('not-configured', 'Canvas2DRenderer not configured');
    }
    this.metrics.framesSubmitted += 1;

    const start = performance.now();
    const ctx = this.ctx;
    const visibleRect = RendererSurface.resolveVisibleRect(frame);
    const viewport = this.surface.computeViewport(visibleRect.width, visibleRect.height);

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.surface.getCanvas().width, this.surface.getCanvas().height);

    const transform = this.surface.getTransform();
    const centerX = viewport.x + viewport.width / 2;
    const centerY = viewport.y + viewport.height / 2;

    ctx.translate(centerX, centerY);
    if (transform.rotation !== 0) {
      ctx.rotate((transform.rotation * Math.PI) / 180);
    }
    ctx.scale(transform.scaleX, transform.scaleY);
    ctx.translate(-viewport.width / 2, -viewport.height / 2);

    try {
      // VideoFrame is a CanvasImageSource in modern browsers.
      ctx.drawImage(
        frame as unknown as CanvasImageSource,
        visibleRect.x,
        visibleRect.y,
        visibleRect.width,
        visibleRect.height,
        0,
        0,
        viewport.width,
        viewport.height,
      );
      this.metrics.framesRendered += 1;
    } catch (err) {
      this.metrics.framesDropped += 1;
      throw new RendererError('draw-failed', err instanceof Error ? err.message : String(err));
    } finally {
      this.metrics.drawLatencyMs = performance.now() - start;
    }
  }

  async snapshot(opts: SnapshotOptions = {}): Promise<SnapshotResult> {
    if (!this.ctx) {
      throw new RendererError('not-configured', 'Canvas2DRenderer not configured');
    }
    const canvas = this.surface.getCanvas();
    const w = canvas.width;
    const h = canvas.height;
    if (opts.maxWidth && opts.maxHeight) {
      const scale = Math.min(1, opts.maxWidth / w, opts.maxHeight / h);
      const sw = Math.max(1, Math.floor(w * scale));
      const sh = Math.max(1, Math.floor(h * scale));
      const data = this.ctx.getImageData(0, 0, sw, sh);
      this.metrics.snapshotsTaken += 1;
      return { width: sw, height: sh, data };
    }
    const data = this.ctx.getImageData(0, 0, w, h);
    this.metrics.snapshotsTaken += 1;
    return { width: w, height: h, data };
  }

  getMetrics(): RendererMetrics {
    return { ...this.metrics };
  }

  close(): void {
    this.configured = false;
    this.ctx = null;
  }
}
