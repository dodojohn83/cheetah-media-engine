/**
 * Browser-based stream recorder for a canvas or video element.
 *
 * Uses `MediaRecorder` over `HTMLCanvasElement.captureStream()` to produce
 * webm/mp4 chunks and writes them to a caller-supplied `WritableStream`.
 * Recording is independent from the player pipeline: it samples the rendered
 * surface, so it does not duplicate decoding.
 */

import { RendererError } from './types';

export type RecordingContainer = 'webm' | 'mp4' | 'flv';

export interface RecordingOptions {
  /** Target stream for the recorded bytes. */
  readonly target: WritableStream<Uint8Array>;
  /** Preferred MIME type; defaults to a browser-supported video/webm. */
  readonly mimeType?: string;
  /** Suggested output filename, without extension. */
  readonly filename?: string;
  /** Capture frame rate; defaults to 30. */
  readonly fps?: number;
  /** Emit chunks at this interval for segmented output. */
  readonly segmentDurationMs?: number;
  /** Stop recording once this many bytes are written. */
  readonly maxSizeBytes?: number;
  /** Stop recording once this many milliseconds elapsed. */
  readonly maxDurationMs?: number;
}

export interface RecordingStats {
  /** Bytes written to the target so far. */
  readonly bytesWritten: number;
  /** Elapsed recording time in milliseconds. */
  readonly durationMs: number;
  /** Approximate pending bytes waiting to be written. */
  readonly queueSize: number;
  /** Whether the writer is currently waiting on backpressure. */
  readonly backpressure: boolean;
}

export interface RecordingResult {
  /** Total bytes produced. */
  readonly bytes: number;
  /** Recorded duration in milliseconds. */
  readonly durationMs: number;
  /** Final MIME type used. */
  readonly mimeType: string;
  /** Output filename, without extension. */
  readonly filename: string;
  /** True if the recording was stopped early by cancel or error. */
  readonly partial: boolean;
  /** Number of segments produced (1 for continuous recordings). */
  readonly segmentCount: number;
}

export interface RecordingSession {
  /** Stop recording and finalize the output. */
  stop(): Promise<RecordingResult>;
  /** Abort recording without finalizing. */
  cancel(): Promise<void>;
  /** Current recording statistics. */
  getStats(): RecordingStats;
}

interface MediaRecorderWithState extends MediaRecorder {
  readonly state: 'inactive' | 'recording' | 'paused';
}

function isCanvasCaptureSupported(source: unknown): source is HTMLCanvasElement {
  return (
    typeof HTMLCanvasElement !== 'undefined' &&
    source instanceof HTMLCanvasElement &&
    typeof source.captureStream === 'function'
  );
}

function chooseMimeType(preferred?: string): string {
  if (preferred && typeof MediaRecorder.isTypeSupported === 'function' && MediaRecorder.isTypeSupported(preferred)) {
    return preferred;
  }
  const candidates = [
    'video/webm;codecs=vp9',
    'video/webm;codecs=vp8',
    'video/webm',
    'video/mp4',
  ];
  for (const candidate of candidates) {
    if (typeof MediaRecorder.isTypeSupported === 'function' && MediaRecorder.isTypeSupported(candidate)) {
      return candidate;
    }
  }
  return 'video/webm';
}

async function blobToUint8Array(blob: Blob): Promise<Uint8Array> {
  const buffer = await blob.arrayBuffer();
  return new Uint8Array(buffer);
}

/**
 * Start recording from a canvas that supports `captureStream()`.
 *
 * @param source An `HTMLCanvasElement` (with `captureStream`) or a `MediaStream`.
 * @param options Recording options.
 */
export function startRecording(
  source: HTMLCanvasElement | MediaStream,
  options: RecordingOptions,
): Promise<RecordingSession> {
  return new MediaStreamRecorderSession(source, options).start();
}

class MediaStreamRecorderSession implements RecordingSession {
  private recorder: MediaRecorderWithState | undefined;
  private writer: WritableStreamDefaultWriter<Uint8Array> | undefined;
  private startTime = 0;
  private bytesWritten = 0;
  private queueSize = 0;
  private backpressure = false;
  private partial = false;
  private segmentCount = 0;
  private stopped = false;
  private stopPromise: Promise<RecordingResult> | undefined;
  private resolveStop: ((result: RecordingResult) => void) | undefined;
  private rejectStop: ((reason: Error) => void) | undefined;

  constructor(
    private readonly source: HTMLCanvasElement | MediaStream,
    private readonly options: RecordingOptions,
  ) {}

  async start(): Promise<RecordingSession> {
    if (typeof MediaRecorder === 'undefined') {
      throw new RendererError('unsupported', 'MediaRecorder is not available in this environment');
    }

    let stream: MediaStream;
    if (typeof MediaStream !== 'undefined' && this.source instanceof MediaStream) {
      stream = this.source;
    } else if (isCanvasCaptureSupported(this.source)) {
      const fps = this.options.fps ?? 30;
      stream = this.source.captureStream(fps);
    } else {
      throw new RendererError(
        'unsupported',
        'Recording source must be an HTMLCanvasElement with captureStream() or a MediaStream',
      );
    }

    const mimeType = chooseMimeType(this.options.mimeType);
    this.recorder = new MediaRecorder(stream, { mimeType }) as MediaRecorderWithState;
    this.writer = this.options.target.getWriter();

    this.recorder.ondataavailable = (event) => {
      void this._onData(event.data);
    };
    this.recorder.onstop = () => {
      this._finalize();
    };
    this.recorder.onerror = () => {
      this._fail(new RendererError('recording-failed', 'MediaRecorder encountered an error'));
    };

    this.startTime = performance.now();
    const timeslice = this.options.segmentDurationMs;
    this.recorder.start(timeslice);
    return this;
  }

  private async _onData(blob: Blob | null): Promise<void> {
    if (!blob || blob.size === 0 || !this.writer) return;
    try {
      const data = await blobToUint8Array(blob);
      this.queueSize += data.length;
      this.backpressure = true;
      await this.writer.write(data);
      this.bytesWritten += data.length;
      this.queueSize -= data.length;
      this.segmentCount += 1;
      this._checkLimits();
    } catch (cause) {
      this._fail(cause instanceof Error ? cause : new Error(String(cause)));
    } finally {
      this.backpressure = false;
    }
  }

  private _checkLimits(): void {
    if (this.stopped) return;
    const elapsed = performance.now() - this.startTime;
    const maxDuration = this.options.maxDurationMs;
    const maxSize = this.options.maxSizeBytes;
    if ((maxDuration !== undefined && elapsed >= maxDuration) || (maxSize !== undefined && this.bytesWritten >= maxSize)) {
      void this.stop();
    }
  }

  private _finalize(): void {
    if (this.stopped) return;
    this.stopped = true;
    this._closeWriter();
    const result: RecordingResult = {
      bytes: this.bytesWritten,
      durationMs: Math.max(0, performance.now() - this.startTime),
      mimeType: this.recorder?.mimeType ?? 'video/webm',
      filename: this.options.filename ?? 'recording',
      partial: this.partial,
      segmentCount: Math.max(1, this.segmentCount),
    };
    this.resolveStop?.(result);
  }

  private _fail(error: Error): void {
    if (this.stopped) return;
    this.stopped = true;
    this.partial = true;
    this._closeWriter();
    this.rejectStop?.(error);
  }

  private _closeWriter(): void {
    try {
      this.writer?.close().catch(() => undefined);
    } catch {
      // ignored
    }
  }

  stop(): Promise<RecordingResult> {
    if (this.stopped) {
      return this.stopPromise ?? Promise.reject(new RendererError('stopped', 'Recording already stopped'));
    }
    if (!this.recorder || this.recorder.state === 'inactive') {
      return Promise.reject(new RendererError('stopped', 'Recording is not active'));
    }
    this.stopPromise = new Promise((resolve, reject) => {
      this.resolveStop = resolve;
      this.rejectStop = reject;
    });
    try {
      this.recorder.stop();
    } catch (cause) {
      this._fail(cause instanceof Error ? cause : new Error(String(cause)));
    }
    return this.stopPromise;
  }

  cancel(): Promise<void> {
    if (this.stopped) return Promise.resolve();
    this.partial = true;
    this.stopped = true;
    try {
      this.recorder?.stop();
    } catch {
      // ignored
    }
    this._closeWriter();
    return Promise.resolve();
  }

  getStats(): RecordingStats {
    return {
      bytesWritten: this.bytesWritten,
      durationMs: this.recorder && this.startTime > 0 ? Math.max(0, performance.now() - this.startTime) : 0,
      queueSize: this.queueSize,
      backpressure: this.backpressure,
    };
  }
}
