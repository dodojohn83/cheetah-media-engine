/**
 * Public types and small helpers for the video renderer.
 */

export type FitMode = 'contain' | 'cover' | 'fill' | 'stretch';

export interface VisibleRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface ColorSpaceInfo {
  readonly primaries?: string;
  readonly transfer?: string;
  readonly matrix?: string;
  readonly fullRange?: boolean;
}

/**
 * Minimal abstraction over a WebCodecs `VideoFrame` so renderer tests can
 * inject mock frames without a real `VideoFrame` instance.
 */
export interface RenderFrame {
  readonly timestamp: number;
  readonly codedWidth: number;
  readonly codedHeight: number;
  readonly visibleRect?: VisibleRect | undefined;
  readonly format: string | null;
  readonly colorSpace?: ColorSpaceInfo | undefined;
  close(): void;
  copyTo(
    destination: ArrayBufferView,
    options: { planeIndex?: number; rect?: VisibleRect | undefined },
  ): Promise<number>;
  allocationSize(options?: { planeIndex?: number; rect?: VisibleRect | undefined }): number;
}

export interface RendererConfig {
  readonly canvas: HTMLCanvasElement | OffscreenCanvas;
  readonly fit?: FitMode | undefined;
  readonly rotation?: number | undefined; // degrees, clockwise
  readonly mirror?: boolean | undefined;
  readonly colorSpace?: ColorSpaceInfo | undefined;
  readonly dpr?: number | undefined;
}

export type SnapshotFormat = 'png' | 'jpeg' | 'webp';

export interface SnapshotOptions {
  /** Target image format; when omitted the renderer returns raw `ImageData`. */
  readonly format?: SnapshotFormat | undefined;
  /** Compression quality for lossy formats, 0..1. */
  readonly quality?: number | undefined;
  /** Maximum width in CSS pixels; the image is scaled down preserving aspect ratio. */
  readonly maxWidth?: number | undefined;
  /** Maximum height in CSS pixels; the image is scaled down preserving aspect ratio. */
  readonly maxHeight?: number | undefined;
  /** Whether to include on-screen overlays (not yet implemented). */
  readonly includeOverlay?: boolean | undefined;
}

export interface SnapshotResult {
  readonly width: number;
  readonly height: number;
  readonly data: ImageData | Blob;
}

export interface RendererMetrics {
  readonly framesSubmitted: number;
  readonly framesRendered: number;
  readonly framesDropped: number;
  readonly snapshotsTaken: number;
  readonly drawLatencyMs: number;
}

export type MutableRendererMetrics = {
  framesSubmitted: number;
  framesRendered: number;
  framesDropped: number;
  snapshotsTaken: number;
  drawLatencyMs: number;
};

export interface Renderer {
  readonly identity: string;
  configure(config: RendererConfig): Promise<void>;
  render(frame: RenderFrame): Promise<void>;
  snapshot(opts?: SnapshotOptions): Promise<SnapshotResult>;
  close(): void;
  getMetrics(): RendererMetrics;
}

export class RendererError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = 'RendererError';
    this.code = code;
  }
}
