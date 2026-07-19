/**
 * Composite recorder: captures a source frame (video/canvas/image) into a
 * browser-supported container with optional audio mixing and watermark overlay.
 *
 * It composes each frame onto an internal canvas, then records that canvas via
 * `HTMLCanvasElement.captureStream()` and `MediaRecorder`. This keeps the
 * recorder independent from the player pipeline while allowing overlays,
 * subtitles and watermarks to be burned into the output.
 */

import { startRecording, type RecordingResult, type RecordingSession, type RecordingOptions } from './recorder';
import { RendererError } from './types';

export type CompositeWatermark =
  | {
      readonly type: 'text';
      readonly text: string;
      readonly x: number;
      readonly y: number;
      readonly font?: string;
      readonly color?: string;
    }
  | {
      readonly type: 'image';
      readonly image: CanvasImageSource;
      readonly x: number;
      readonly y: number;
      readonly width?: number;
      readonly height?: number;
    };

export type CompositeRecordingState = 'inactive' | 'recording' | 'paused' | 'stopped' | 'error';

export interface CompositeRecordingOptions extends Omit<RecordingOptions, 'target'> {
  /** Visual source to capture each frame from. */
  readonly source: CanvasImageSource | ImageData;
  /** Optional audio stream to mux into the output. */
  readonly audioStream?: MediaStream;
  /** Optional watermark burned into every recorded frame. */
  readonly watermark?: CompositeWatermark;
  /** Optional list of watermarks burned into every recorded frame. */
  readonly watermarks?: readonly CompositeWatermark[];
  /** Explicit output width; defaults to the source intrinsic width. */
  readonly width?: number;
  /** Explicit output height; defaults to the source intrinsic height. */
  readonly height?: number;
  /** Capture frame rate; defaults to 30. */
  readonly fps?: number;
  /** Called when the recording completes, including auto-stop and manual stop. */
  readonly onComplete?: (result: CompositeRecordingResult) => void;
  /** Called when the recording fails. */
  readonly onError?: (error: Error) => void;
}

export interface CompositeRecordingResult extends RecordingResult {
  /** Final recorded blob. */
  readonly blob: Blob;
}

export interface CompositeRecordingProgress {
  readonly bytesWritten: number;
  readonly durationMs: number;
  readonly state: CompositeRecordingState;
}

class BlobStreamSink {
  private chunks: Blob[] = [];
  private resolve: (blob: Blob) => void = () => undefined;
  private reject: (reason: Error) => void = () => undefined;
  private resultPromise = new Promise<Blob>((resolve, reject) => {
    this.resolve = resolve;
    this.reject = reject;
  });

  get writable(): WritableStream<Uint8Array> {
    return new WritableStream<Uint8Array>({
      write: (chunk) => {
        this.chunks.push(new Blob([chunk as unknown as BlobPart]));
      },
      close: () => {
        this.resolve(new Blob(this.chunks as unknown as BlobPart[]));
      },
      abort: (reason) => {
        this.reject(reason instanceof Error ? reason : new Error(String(reason)));
      },
    });
  }

  get result(): Promise<Blob> {
    return this.resultPromise;
  }
}

function createCanvas(width: number, height: number): HTMLCanvasElement {
  if (typeof document !== 'undefined' && typeof document.createElement === 'function') {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    return canvas;
  }
  throw new RendererError('unsupported', 'No canvas implementation available for composite recording');
}

function getSourceSize(source: CanvasImageSource | ImageData): { width: number; height: number } {
  if (typeof HTMLVideoElement !== 'undefined' && source instanceof HTMLVideoElement) {
    return { width: source.videoWidth || 1, height: source.videoHeight || 1 };
  }
  if (typeof HTMLCanvasElement !== 'undefined' && source instanceof HTMLCanvasElement) {
    return { width: source.width || 1, height: source.height || 1 };
  }
  if (typeof OffscreenCanvas !== 'undefined' && source instanceof OffscreenCanvas) {
    return { width: source.width || 1, height: source.height || 1 };
  }
  if (typeof ImageBitmap !== 'undefined' && source instanceof ImageBitmap) {
    return { width: source.width || 1, height: source.height || 1 };
  }
  if (typeof HTMLImageElement !== 'undefined' && source instanceof HTMLImageElement) {
    return { width: source.width || 1, height: source.height || 1 };
  }
  if (typeof ImageData !== 'undefined' && source instanceof ImageData) {
    return { width: source.width, height: source.height };
  }
  if (source && typeof source === 'object') {
    const maybe = source as { width?: unknown; height?: unknown; videoWidth?: unknown; videoHeight?: unknown };
    const width = Number(maybe.videoWidth ?? maybe.width ?? 0);
    const height = Number(maybe.videoHeight ?? maybe.height ?? 0);
    if (width > 0 && height > 0) {
      return { width, height };
    }
  }
  throw new RendererError('invalid-source', 'Cannot determine source size for composite recording');
}

function drawSource(ctx: CanvasRenderingContext2D, source: CanvasImageSource | ImageData, width: number, height: number): void {
  if (typeof ImageData !== 'undefined' && source instanceof ImageData) {
    ctx.putImageData(source, 0, 0);
  } else {
    ctx.drawImage(source as CanvasImageSource, 0, 0, width, height);
  }
}

function isImageComplete(image: CanvasImageSource): boolean {
  if (typeof HTMLImageElement !== 'undefined' && image instanceof HTMLImageElement) {
    return image.complete && image.naturalWidth > 0;
  }
  if (typeof ImageBitmap !== 'undefined' && image instanceof ImageBitmap) {
    return image.width > 0;
  }
  return true;
}

async function awaitImage(image: CanvasImageSource): Promise<void> {
  if (typeof HTMLImageElement !== 'undefined' && image instanceof HTMLImageElement && !image.complete) {
    await new Promise<void>((resolve, reject) => {
      const onLoad = () => {
        image.removeEventListener('load', onLoad);
        image.removeEventListener('error', onError);
        resolve();
      };
      const onError = () => {
        image.removeEventListener('load', onLoad);
        image.removeEventListener('error', onError);
        reject(new RendererError('invalid-source', 'Watermark image failed to load'));
      };
      image.addEventListener('load', onLoad);
      image.addEventListener('error', onError);
    });
  }
}

export class CompositeRecorder {
  private state: CompositeRecordingState = 'inactive';
  private canvas: HTMLCanvasElement | undefined;
  private ctx: CanvasRenderingContext2D | null | undefined;
  private session: RecordingSession | undefined;
  private sink: BlobStreamSink | undefined;
  private options: CompositeRecordingOptions | undefined;
  private rafId: number | undefined;
  private limitTimer: ReturnType<typeof setInterval> | undefined;
  private lastResult: CompositeRecordingResult | undefined;
  private startTime = 0;
  private bytesWritten = 0;

  get progress(): CompositeRecordingProgress {
    const stats = this.session?.getStats();
    return {
      bytesWritten: stats?.bytesWritten ?? this.bytesWritten,
      durationMs: stats?.durationMs ?? Math.max(0, performance.now() - this.startTime),
      state: this.state,
    };
  }

  get recordingActive(): boolean {
    return this.state === 'recording';
  }

  async start(options: CompositeRecordingOptions): Promise<CompositeRecorder> {
    if (this.state !== 'inactive' && this.state !== 'stopped' && this.state !== 'error') {
      throw new RendererError('invalid-state', `Cannot start composite recording from state ${this.state}`);
    }

    // Reset any state left over from a previous recording session so that
    // progress and early stop() calls do not report stale results.
    this.lastResult = undefined;
    this.bytesWritten = 0;
    this.startTime = 0;

    if (options.fps !== undefined && (!Number.isFinite(options.fps) || options.fps <= 0)) {
      throw new RendererError('invalid-option', 'fps must be a finite positive number');
    }
    if (options.segmentDurationMs !== undefined && (!Number.isFinite(options.segmentDurationMs) || options.segmentDurationMs < 0)) {
      throw new RendererError('invalid-option', 'segmentDurationMs must be a finite non-negative number');
    }
    if (options.maxDurationMs !== undefined && (!Number.isFinite(options.maxDurationMs) || options.maxDurationMs < 0)) {
      throw new RendererError('invalid-option', 'maxDurationMs must be a finite non-negative number');
    }
    if (options.maxSizeBytes !== undefined && (!Number.isFinite(options.maxSizeBytes) || options.maxSizeBytes < 0)) {
      throw new RendererError('invalid-option', 'maxSizeBytes must be a finite non-negative number');
    }

    const { width, height } = getSourceSize(options.source);
    const outputWidth = options.width && options.width > 0 ? options.width : width;
    const outputHeight = options.height && options.height > 0 ? options.height : height;

    this.canvas = createCanvas(outputWidth, outputHeight);
    this.ctx = this.canvas.getContext('2d');
    if (!this.ctx) {
      throw new RendererError('no-context', 'Cannot create 2D context for composite recording');
    }

    const marks = options.watermarks && options.watermarks.length > 0 ? options.watermarks : options.watermark ? [options.watermark] : [];
    for (const watermark of marks) {
      if (watermark.type === 'image') {
        await awaitImage(watermark.image);
        if (!isImageComplete(watermark.image)) {
          throw new RendererError('invalid-source', 'Watermark image is not ready');
        }
      }
    }

    this.options = options;
    this.sink = new BlobStreamSink();
    this._drawFrame();

    const stream = this._buildStream();
    let segmentDurationMs = options.segmentDurationMs;
    if (segmentDurationMs === undefined && options.maxSizeBytes !== undefined) {
      segmentDurationMs = Math.min(options.maxDurationMs ?? 1000, 1000);
    }
    const recordingOptions: RecordingOptions = {
      target: this.sink.writable,
      ...(options.mimeType !== undefined ? { mimeType: options.mimeType } : {}),
      ...(options.filename !== undefined ? { filename: options.filename } : {}),
      ...(options.fps !== undefined ? { fps: options.fps } : {}),
      ...(segmentDurationMs !== undefined ? { segmentDurationMs } : {}),
      ...(options.maxSizeBytes !== undefined ? { maxSizeBytes: options.maxSizeBytes } : {}),
      ...(options.maxDurationMs !== undefined ? { maxDurationMs: options.maxDurationMs } : {}),
    };

    this.session = await startRecording(stream, recordingOptions);
    this.state = 'recording';
    this.startTime = performance.now();
    this._scheduleFrame();
    this._startLimitWatch();
    return this;
  }

  private _buildStream(): MediaStream {
    if (!this.canvas || typeof this.canvas.captureStream !== 'function') {
      throw new RendererError('unsupported', 'Canvas captureStream is not available');
    }
    const fps = this.options?.fps ?? 30;
    const videoStream = this.canvas.captureStream(fps);
    const audioStream = this.options?.audioStream;
    if (audioStream && typeof MediaStream !== 'undefined' && audioStream.getAudioTracks && audioStream.getAudioTracks().length > 0) {
      return new MediaStream([...videoStream.getVideoTracks(), ...audioStream.getAudioTracks()]);
    }
    return videoStream;
  }

  pause(): void {
    if (this.state !== 'recording') return;
    this.state = 'paused';
    this._stopLimitWatch();
    if (this.rafId !== undefined) {
      cancelAnimationFrame(this.rafId);
      this.rafId = undefined;
    }
    this.session?.pause();
  }

  resume(): void {
    if (this.state !== 'paused') return;
    this.state = 'recording';
    this.session?.resume();
    this._scheduleFrame();
    this._startLimitWatch();
  }

  async stop(): Promise<CompositeRecordingResult> {
    if (this.state === 'inactive') {
      throw new RendererError('invalid-state', 'Composite recording is not active');
    }
    if (this.state === 'stopped') {
      if (this.lastResult !== undefined) return this.lastResult;
      throw new RendererError('invalid-state', 'Composite recording already stopped');
    }
    return this._finish(false);
  }

  private async _finish(notify: boolean): Promise<CompositeRecordingResult> {
    if (this.lastResult !== undefined) return this.lastResult;
    if (this.rafId !== undefined) {
      cancelAnimationFrame(this.rafId);
      this.rafId = undefined;
    }
    this._stopLimitWatch();
    this.state = 'stopped';
    if (!this.session || !this.sink) {
      throw new RendererError('invalid-state', 'Composite recording session is missing');
    }
    try {
      const [result, blob] = await Promise.all([this.session.stop(), this.sink.result]);
      const compositeResult: CompositeRecordingResult = { ...result, blob };
      this.bytesWritten = result.bytes;
      this.lastResult = compositeResult;
      if (notify) {
        try {
          this.options?.onComplete?.(compositeResult);
        } catch {
          // Notification callbacks must not break finalization.
        }
      }
      return compositeResult;
    } catch (cause) {
      const error = cause instanceof Error ? cause : new Error(String(cause));
      if (notify) {
        try {
          this.options?.onError?.(error);
        } catch {
          // Notification callbacks must not break finalization.
        }
      }
      throw error;
    }
  }

  private _scheduleFrame(): void {
    if (this.state !== 'recording') return;
    this.rafId = requestAnimationFrame(() => {
      this._drawFrame();
      this._scheduleFrame();
    });
  }

  private _drawFrame(): void {
    if (!this.ctx || !this.canvas || !this.options) return;
    const { source, width, height } = this.options;
    const marks = this._watermarks();
    const outputWidth = width && width > 0 ? width : this.canvas.width;
    const outputHeight = height && height > 0 ? height : this.canvas.height;

    this.ctx.clearRect(0, 0, outputWidth, outputHeight);
    drawSource(this.ctx, source, outputWidth, outputHeight);
    this._checkLimits();

    for (const watermark of marks) {
      if (watermark.type === 'text') {
        this.ctx.font = watermark.font ?? '24px sans-serif';
        this.ctx.fillStyle = watermark.color ?? 'rgba(255,255,255,0.8)';
        this.ctx.textBaseline = 'top';
        this.ctx.textAlign = 'left';
        this.ctx.fillText(watermark.text, watermark.x, watermark.y);
      } else if (watermark.type === 'image' && isImageComplete(watermark.image)) {
        const w = watermark.width ?? (watermark.image as ImageBitmap).width ?? 0;
        const h = watermark.height ?? (watermark.image as ImageBitmap).height ?? 0;
        if (w > 0 && h > 0) {
          this.ctx.drawImage(watermark.image, watermark.x, watermark.y, w, h);
        }
      }
    }
  }

  private _startLimitWatch(): void {
    if (this.limitTimer !== undefined) return;
    this.limitTimer = setInterval(() => this._checkLimits(), 250);
  }

  private _stopLimitWatch(): void {
    if (this.limitTimer === undefined) return;
    clearInterval(this.limitTimer);
    this.limitTimer = undefined;
  }

  private _checkLimits(): void {
    if (this.state !== 'recording' || !this.session || !this.options) return;
    const stats = this.session.getStats();
    const maxDuration = this.options.maxDurationMs;
    const maxSize = this.options.maxSizeBytes;
    if ((maxDuration !== undefined && stats.durationMs >= maxDuration) || (maxSize !== undefined && stats.bytesWritten >= maxSize)) {
      void this._finish(true).catch(() => undefined);
    }
  }

  private _watermarks(): readonly CompositeWatermark[] {
    if (!this.options) return [];
    if (this.options.watermarks && this.options.watermarks.length > 0) {
      return this.options.watermarks;
    }
    return this.options.watermark ? [this.options.watermark] : [];
  }
}
