/**
 * MSE (Media Source Extensions) backend for the browser runtime.
 *
 * Wraps `MediaSource` and `SourceBuffer` with a serialised append queue,
 * generation filtering, bounded buffer window management and low-latency
 * playback control.  Like the WebCodecs backend, the `FallbackController`
 * only sees the `MediaBackend` surface (`configure` / `stop` / `identity`);
 * the runtime feeds fMP4 segments through `pushSegment`.
 */

import type { Backend, TrackProfile } from './planner';
import type { BackendContext, MediaBackend } from './fallback';

export type MseErrorCode =
  | 'not-configured'
  | 'invalid-config'
  | 'mse-not-supported'
  | 'source-open-timeout'
  | 'append-error'
  | 'remove-error'
  | 'source-buffer-error'
  | 'media-source-error'
  | 'video-element-error'
  | 'quota-exceeded'
  | 'seek'
  | 'unknown';

export class MseError extends Error {
  readonly code: MseErrorCode;

  constructor(code: MseErrorCode, message: string) {
    super(message);
    this.name = 'MseError';
    this.code = code;
  }
}

export interface MseCallbacks {
  readonly onError?: (error: MseError) => void;
}

export interface HTMLVideoElementLike {
  src: string;
  srcObject?: unknown;
  currentTime: number;
  playbackRate: number;
  paused: boolean;
  readyState: number;
  error: { readonly code: number; readonly message: string } | null;
  play(): Promise<void> | undefined;
  pause(): void;
  load(): void;
  addEventListener(type: string, listener: (event?: unknown) => void): void;
  removeEventListener(type: string, listener: (event?: unknown) => void): void;
}

export interface TimeRangesLike {
  readonly length: number;
  start(index: number): number;
  end(index: number): number;
}

export interface SourceBufferLike {
  updating: boolean;
  buffered: TimeRangesLike;
  timestampOffset: number;
  appendWindowStart: number;
  appendWindowEnd: number;
  appendBuffer(data: ArrayBufferView): void;
  remove(start: number, end: number): void;
  abort(): void;
  changeType?(type: string): void;
  addEventListener(type: string, listener: (event?: unknown) => void): void;
  removeEventListener(type: string, listener: (event?: unknown) => void): void;
}

export interface MediaSourceLike {
  readonly readyState: string;
  sourceBuffers: { readonly length: number } & Iterable<SourceBufferLike>;
  addSourceBuffer(type: string): SourceBufferLike;
  removeSourceBuffer(sb: SourceBufferLike): void;
  endOfStream(): void;
  addEventListener(type: string, listener: (event?: unknown) => void): void;
  removeEventListener(type: string, listener: (event?: unknown) => void): void;
}

interface MediaSourceConstructor {
  new (): MediaSourceLike;
  isTypeSupported(type: string): boolean;
}

interface URLConstructor {
  createObjectURL(obj: unknown): string;
  revokeObjectURL(url: string): void;
}

export interface MseBackendOptions {
  readonly videoElement: HTMLVideoElementLike;
  readonly tracks: readonly TrackProfile[];
  readonly callbacks?: MseCallbacks;
  /** When `false` the backend is in VOD mode and live catch-up is disabled. */
  readonly isLive?: boolean;
  readonly maxBufferAheadMs?: number;
  readonly maxBufferBehindMs?: number;
  readonly liveLatencyTargetMs?: number;
  readonly liveDriftSmallMs?: number;
  readonly liveDriftLargeMs?: number;
  readonly minPlaybackRate?: number;
  readonly maxPlaybackRate?: number;
  readonly liveControlIntervalMs?: number;
  readonly sourceOpenTimeoutMs?: number;
  readonly maxAppendQueue?: number;
  readonly maxQuotaRetries?: number;
  /** Estimated video frame rate for MSE frame stepping. */
  readonly videoFrameRate?: number;
}

export interface MseMetrics {
  readonly appendQueueDepth: number;
  readonly bufferedStart: number;
  readonly bufferedEnd: number;
  readonly mediaSourceReadyState: string;
  readonly stallCount: number;
  readonly quotaCleanupCount: number;
  readonly seekCount: number;
  readonly droppedSegments: number;
  readonly lastErrorCode: MseErrorCode | undefined;
}

type MutableMseMetrics = {
  bufferedStart: number;
  bufferedEnd: number;
  stallCount: number;
  quotaCleanupCount: number;
  seekCount: number;
  droppedSegments: number;
  lastErrorCode: MseErrorCode | undefined;
};

interface AppendTask {
  readonly type: 'append';
  readonly data: Uint8Array;
  readonly generation: number;
  readonly isInit: boolean;
  readonly timestampOffset: number | undefined;
  readonly appendWindowStart: number | undefined;
  readonly appendWindowEnd: number | undefined;
}

type QueueTask = AppendTask;

function getGlobal<T>(name: string): T | undefined {
  if (typeof globalThis === 'undefined') return undefined;
  try {
    const value = (globalThis as unknown as Record<string, T>)[name];
    return value ?? undefined;
  } catch {
    return undefined;
  }
}

function toMimeCodec(codec: string): string {
  const c = codec.toLowerCase();
  if (c.startsWith('avc')) return c;
  if (c.startsWith('hvc') || c.startsWith('hev')) return c;
  if (c.startsWith('av01')) return c;
  if (c === 'h264') return 'avc1.42001e';
  if (c === 'h265' || c === 'hevc') return 'hvc1.1.6.l93.b0';
  if (c === 'av1') return 'av01.0.04m.10';
  if (c === 'aac') return 'mp4a.40.2';
  if (c === 'mp3') return 'mp3';
  if (c === 'g711a' || c === 'alaw') return 'alaw';
  if (c === 'g711u' || c === 'ulaw') return 'ulaw';
  return c;
}

function buildMime(tracks: readonly TrackProfile[]): string {
  const hasVideo = tracks.some((t) => t.kind === 'video');
  const type = hasVideo ? 'video' : 'audio';
  const codecs = tracks.map((t) => toMimeCodec(t.codec)).join(',');
  return `${type}/mp4;codecs="${codecs}"`;
}

function isQuotaExceeded(err: unknown): boolean {
  if (err === null || typeof err !== 'object') return false;
  return (
    (err as { name?: string }).name === 'QuotaExceededError' ||
    (err as { code?: number }).code === 22
  );
}

function validatePositiveFiniteInteger(value: number, name: string): void {
  if (!Number.isInteger(value) || value < 1) {
    throw new MseError('invalid-config', `${name} must be a finite positive integer`);
  }
}

function validateNonNegativeFiniteInteger(value: number, name: string): void {
  if (!Number.isInteger(value) || value < 0) {
    throw new MseError('invalid-config', `${name} must be a finite non-negative integer`);
  }
}

function validateNonNegativeFiniteNumber(value: number, name: string): void {
  if (!Number.isFinite(value) || value < 0) {
    throw new MseError('invalid-config', `${name} must be a finite non-negative number`);
  }
}

function validatePositiveFiniteNumber(value: number, name: string): void {
  if (!Number.isFinite(value) || value <= 0) {
    throw new MseError('invalid-config', `${name} must be a finite positive number`);
  }
}

function validateMseOptions(options: MseBackendOptions): void {
  if (!options.videoElement) {
    throw new MseError('invalid-config', 'videoElement is required');
  }
  if (!Array.isArray(options.tracks) || options.tracks.length === 0) {
    throw new MseError('invalid-config', 'tracks must be a non-empty array');
  }
  const o = {
    maxBufferAheadMs: options.maxBufferAheadMs ?? 5000,
    maxBufferBehindMs: options.maxBufferBehindMs ?? 5000,
    liveLatencyTargetMs: options.liveLatencyTargetMs ?? 1000,
    liveDriftSmallMs: options.liveDriftSmallMs ?? 200,
    liveDriftLargeMs: options.liveDriftLargeMs ?? 1000,
    minPlaybackRate: options.minPlaybackRate ?? 0.8,
    maxPlaybackRate: options.maxPlaybackRate ?? 1.2,
    liveControlIntervalMs: options.liveControlIntervalMs ?? 250,
    sourceOpenTimeoutMs: options.sourceOpenTimeoutMs ?? 5000,
    maxAppendQueue: options.maxAppendQueue ?? 32,
    maxQuotaRetries: options.maxQuotaRetries ?? 1,
    videoFrameRate: options.videoFrameRate ?? 30,
  };
  validateNonNegativeFiniteNumber(o.maxBufferAheadMs, 'maxBufferAheadMs');
  validateNonNegativeFiniteNumber(o.maxBufferBehindMs, 'maxBufferBehindMs');
  validatePositiveFiniteNumber(o.liveLatencyTargetMs, 'liveLatencyTargetMs');
  validateNonNegativeFiniteNumber(o.liveDriftSmallMs, 'liveDriftSmallMs');
  validateNonNegativeFiniteNumber(o.liveDriftLargeMs, 'liveDriftLargeMs');
  if (!Number.isFinite(o.minPlaybackRate) || o.minPlaybackRate <= 0) {
    throw new MseError('invalid-config', 'minPlaybackRate must be a finite positive number');
  }
  if (!Number.isFinite(o.maxPlaybackRate) || o.maxPlaybackRate <= 0) {
    throw new MseError('invalid-config', 'maxPlaybackRate must be a finite positive number');
  }
  if (o.minPlaybackRate >= o.maxPlaybackRate) {
    throw new MseError('invalid-config', 'minPlaybackRate must be less than maxPlaybackRate');
  }
  validatePositiveFiniteNumber(o.liveControlIntervalMs, 'liveControlIntervalMs');
  validatePositiveFiniteNumber(o.sourceOpenTimeoutMs, 'sourceOpenTimeoutMs');
  validatePositiveFiniteInteger(o.maxAppendQueue, 'maxAppendQueue');
  validateNonNegativeFiniteInteger(o.maxQuotaRetries, 'maxQuotaRetries');
  validatePositiveFiniteNumber(o.videoFrameRate, 'videoFrameRate');
}

function mediaErrorMessage(code: number): string {
  const map: Record<number, string> = {
    1: 'MEDIA_ERR_ABORTED',
    2: 'MEDIA_ERR_NETWORK',
    3: 'MEDIA_ERR_DECODE',
    4: 'MEDIA_ERR_SRC_NOT_SUPPORTED',
  };
  return map[code] ?? `unknown media error code ${code}`;
}

export class MseBackend implements MediaBackend {
  readonly identity: Backend = 'mse';

  private readonly videoElement: HTMLVideoElementLike;
  private readonly tracks: readonly TrackProfile[];
  private readonly callbacks: MseCallbacks;
  private readonly mime: string;
  private readonly maxBufferAheadMs: number;
  private readonly maxBufferBehindMs: number;
  private readonly liveLatencyTargetMs: number;
  private readonly liveDriftSmallMs: number;
  private readonly liveDriftLargeMs: number;
  private readonly minPlaybackRate: number;
  private readonly maxPlaybackRate: number;
  private readonly liveControlIntervalMs: number;
  private readonly sourceOpenTimeoutMs: number;
  private readonly maxAppendQueue: number;
  private readonly maxQuotaRetries: number;
  private readonly isLive: boolean;

  private mediaSource: MediaSourceLike | undefined = undefined;
  private sourceBuffer: SourceBufferLike | undefined = undefined;
  private objectUrl: string | undefined = undefined;
  private configured = false;
  private stopped = true;
  private closing = false;
  private errored = false;
  private generation = 0;
  private appendQueue: QueueTask[] = [];
  private cleanupInProgress = false;
  private quotaRetryCount = 0;
  private liveControlTimer: ReturnType<typeof setInterval> | undefined = undefined;
  private displayPaused = false;
  private displayKeepConnection = true;
  private readonly defaultFrameStepS: number;

  private _metrics: MutableMseMetrics = {
    bufferedStart: 0,
    bufferedEnd: 0,
    stallCount: 0,
    quotaCleanupCount: 0,
    seekCount: 0,
    droppedSegments: 0,
    lastErrorCode: undefined,
  };

  constructor(_ctx: BackendContext, options: MseBackendOptions) {
    validateMseOptions(options);
    this.videoElement = options.videoElement;
    this.tracks = options.tracks;
    this.callbacks = options.callbacks ?? {};
    this.mime = buildMime(options.tracks);
    this.maxBufferAheadMs = options.maxBufferAheadMs ?? 5000;
    this.maxBufferBehindMs = options.maxBufferBehindMs ?? 5000;
    this.liveLatencyTargetMs = options.liveLatencyTargetMs ?? 1000;
    this.liveDriftSmallMs = options.liveDriftSmallMs ?? 200;
    this.liveDriftLargeMs = options.liveDriftLargeMs ?? 1000;
    this.minPlaybackRate = options.minPlaybackRate ?? 0.8;
    this.maxPlaybackRate = options.maxPlaybackRate ?? 1.2;
    this.liveControlIntervalMs = options.liveControlIntervalMs ?? 250;
    this.sourceOpenTimeoutMs = options.sourceOpenTimeoutMs ?? 5000;
    this.maxAppendQueue = options.maxAppendQueue ?? 32;
    this.maxQuotaRetries = options.maxQuotaRetries ?? 1;
    this.isLive = options.isLive ?? true;
    this.defaultFrameStepS = 1 / (options.videoFrameRate ?? 30);
  }

  get metrics(): MseMetrics {
    return {
      ...this._metrics,
      appendQueueDepth: this.appendQueue.length,
      mediaSourceReadyState: this.mediaSource?.readyState ?? 'closed',
    };
  }

  async configure(): Promise<void> {
    if (this.configured) {
      throw new MseError('not-configured', 'MseBackend already configured');
    }

    const MediaSource = getGlobal<unknown>('MediaSource') as MediaSourceConstructor | undefined;
    const URL = getGlobal<unknown>('URL') as URLConstructor | undefined;

    if (!MediaSource) {
      throw new MseError('mse-not-supported', 'MediaSource API not available');
    }
    if (!URL) {
      throw new MseError('mse-not-supported', 'URL API not available');
    }
    if (this.tracks.length === 0) {
      throw new MseError('mse-not-supported', 'No tracks provided');
    }
    if (!MediaSource.isTypeSupported(this.mime)) {
      throw new MseError('mse-not-supported', `MIME type not supported: ${this.mime}`);
    }

    this.generation += 1;
    this.stopped = false;
    this.errored = false;

    const mediaSource = new MediaSource();
    this.mediaSource = mediaSource;

    try {
      mediaSource.addEventListener('error', this.onMediaSourceError);

      this.objectUrl = URL.createObjectURL(mediaSource);
      this.videoElement.src = this.objectUrl;

      if (mediaSource.readyState !== 'open') {
        await this.waitForSourceOpen(mediaSource);
      }

      this.sourceBuffer = this.addSourceBuffer(mediaSource);
      this.videoElement.addEventListener('error', this.onVideoError);

      this.startLiveControl();
      this.configured = true;
    } catch (err) {
      await this.stop();
      throw err;
    }
  }

  pushSegment(
    data: Uint8Array,
    opts?: {
      isInit?: boolean;
      timestampOffset?: number;
      appendWindowStart?: number;
      appendWindowEnd?: number;
    },
  ): void {
    if (this.stopped || this.closing || this.errored || !this.configured) return;
    if (this.displayPaused && !this.displayKeepConnection) {
      this._metrics.droppedSegments += 1;
      return;
    }
    if (this.appendQueue.length >= this.maxAppendQueue) {
      this._metrics.droppedSegments += 1;
      return;
    }
    this.appendQueue.push({
      type: 'append',
      data,
      generation: this.generation,
      isInit: opts?.isInit ?? false,
      timestampOffset: opts?.timestampOffset,
      appendWindowStart: opts?.appendWindowStart,
      appendWindowEnd: opts?.appendWindowEnd,
    });
    this.processQueue();
  }

  async stop(): Promise<void> {
    if (this.stopped || this.closing) return;
    this.closing = true;
    this.stopLiveControl();

    const mediaSource = this.mediaSource;
    const sourceBuffer = this.sourceBuffer;

    if (mediaSource) {
      mediaSource.removeEventListener('error', this.onMediaSourceError);
    }
    if (sourceBuffer) {
      sourceBuffer.removeEventListener('updateend', this.onSourceBufferUpdateEnd);
      sourceBuffer.removeEventListener('error', this.onSourceBufferError);
      sourceBuffer.removeEventListener('abort', this.onSourceBufferAbort);
      try {
        if (!sourceBuffer.updating) sourceBuffer.abort();
      } catch {
        // ignore
      }
      try {
        mediaSource?.removeSourceBuffer(sourceBuffer);
      } catch {
        // ignore
      }
    }
    if (mediaSource && mediaSource.readyState === 'open' && !this.errored) {
      try {
        mediaSource.endOfStream();
      } catch {
        // ignore
      }
    }

    this.videoElement.removeEventListener('error', this.onVideoError);
    if (this.objectUrl) {
      const URL = getGlobal<unknown>('URL') as URLConstructor | undefined;
      try {
        URL?.revokeObjectURL(this.objectUrl);
      } catch {
        // ignore
      }
      this.objectUrl = undefined;
    }
    this.videoElement.src = '';
    try {
      this.videoElement.load?.();
    } catch {
      // ignore
    }

    this.mediaSource = undefined;
    this.sourceBuffer = undefined;
    this.appendQueue = [];
    this.quotaRetryCount = 0;
    this.cleanupInProgress = false;
    this.displayPaused = false;
    this.displayKeepConnection = true;
    this.stopped = true;
    this.closing = false;
    this.errored = false;
    this.configured = false;
    this.generation += 1;
  }

  async seek(timeMs: number): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new MseError('not-configured', 'Cannot seek before configure');
    }
    if (!Number.isFinite(timeMs) || timeMs < 0) {
      throw new MseError('seek', 'seek timeMs must be finite and non-negative');
    }
    const sourceBuffer = this.sourceBuffer;
    if (!sourceBuffer) {
      throw new MseError('not-configured', 'No SourceBuffer');
    }

    this.stopLiveControl();
    this.generation += 1;
    this.appendQueue = [];

    try {
      if (sourceBuffer.updating) {
        sourceBuffer.abort();
      }
      const buffered = sourceBuffer.buffered;
      if (buffered.length > 0) {
        sourceBuffer.remove(buffered.start(0), buffered.end(buffered.length - 1));
      }
    } catch {
      // ignore cleanup errors
    }

    this._metrics.seekCount += 1;
    const timeS = timeMs / 1000;
    this.videoElement.currentTime = timeS;

    return new Promise<void>((resolve, reject) => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const onSeeked = (): void => {
        cleanup();
        resolve();
      };
      const onError = (event?: unknown): void => {
        cleanup();
        reject(this.eventToError(event, 'video-element-error'));
      };
      const cleanup = (): void => {
        if (timer) clearTimeout(timer);
        this.videoElement.removeEventListener('seeked', onSeeked);
        this.videoElement.removeEventListener('error', onError);
        if (!this.stopped && !this.closing && !this.errored) {
          this.startLiveControl();
        }
      };
      this.videoElement.addEventListener('seeked', onSeeked);
      this.videoElement.addEventListener('error', onError);
      timer = setTimeout(() => {
        cleanup();
        reject(new MseError('seek', 'Seeked event timeout'));
      }, 5000);
    });
  }

  async setPlaybackRate(rate: number): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new MseError('not-configured', 'Cannot set playback rate before configure');
    }
    if (!Number.isFinite(rate) || rate < 0.1 || rate > 16) {
      throw new MseError('seek', 'playback rate must be between 0.1 and 16');
    }
    this.videoElement.playbackRate = rate;
  }

  async pauseDisplay(keepConnection = true): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new MseError('not-configured', 'Cannot pause display before configure');
    }
    // Freeze the displayed picture while keeping the media pipeline alive.
    this.videoElement.pause?.();
    this.displayPaused = true;
    this.displayKeepConnection = keepConnection;
    if (!keepConnection) {
      // Stop accepting new segments and clear the append queue.
      this.appendQueue = [];
    }
    // Stop live latency correction; buffer cleanup still runs in the background.
    if (this.isLive) {
      this.stopLiveControl();
      this.startLiveControl();
    }
  }

  async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new MseError('not-configured', 'Cannot frame step before configure');
    }
    if (direction !== 'forward' && direction !== 'backward') {
      throw new MseError('seek', 'frameStep direction must be forward or backward');
    }
    if (keyframeOnly) {
      // MSE does not have frame-type visibility into the buffered segment; this
      // would require decoder feedback. Report unsupported rather than guess.
      throw new MseError('not-configured', 'Keyframe-only frame stepping is not supported in MSE mode');
    }

    const video = this.videoElement;
    const delta = direction === 'forward' ? this.defaultFrameStepS : -this.defaultFrameStepS;
    const target = Math.max(0, video.currentTime + delta);

    return new Promise<void>((resolve, reject) => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const onSeeked = (): void => {
        cleanup();
        resolve();
      };
      const onError = (event?: unknown): void => {
        cleanup();
        reject(this.eventToError(event, 'video-element-error'));
      };
      const cleanup = (): void => {
        if (timer) clearTimeout(timer);
        video.removeEventListener('seeked', onSeeked);
        video.removeEventListener('error', onError);
      };
      video.addEventListener('seeked', onSeeked);
      video.addEventListener('error', onError);
      video.currentTime = target;
      timer = setTimeout(() => {
        cleanup();
        // If the browser did not fire seeked, treat the time adjustment as best-effort.
        resolve();
      }, 1000);
    });
  }

  private addSourceBuffer(mediaSource: MediaSourceLike): SourceBufferLike {
    const sourceBuffer = mediaSource.addSourceBuffer(this.mime);
    sourceBuffer.addEventListener('updateend', this.onSourceBufferUpdateEnd);
    sourceBuffer.addEventListener('error', this.onSourceBufferError);
    sourceBuffer.addEventListener('abort', this.onSourceBufferAbort);
    return sourceBuffer;
  }

  private waitForSourceOpen(mediaSource: MediaSourceLike): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const onOpen = (): void => {
        cleanup();
        resolve();
      };
      const onError = (): void => {
        cleanup();
        reject(new MseError('media-source-error', 'MediaSource error while waiting for sourceopen'));
      };
      const timer = setTimeout(() => {
        cleanup();
        reject(new MseError('source-open-timeout', `MediaSource did not open within ${this.sourceOpenTimeoutMs}ms`));
      }, this.sourceOpenTimeoutMs);
      function cleanup(): void {
        clearTimeout(timer);
        mediaSource.removeEventListener('sourceopen', onOpen);
        mediaSource.removeEventListener('error', onError);
      }
      mediaSource.addEventListener('sourceopen', onOpen);
      mediaSource.addEventListener('error', onError);
    });
  }

  private startLiveControl(): void {
    if (this.liveControlTimer !== undefined) return;
    this.liveControlTimer = setInterval(() => this.adjustPlayback(), this.liveControlIntervalMs);
  }

  private stopLiveControl(): void {
    if (this.liveControlTimer !== undefined) {
      clearInterval(this.liveControlTimer);
      this.liveControlTimer = undefined;
    }
  }

  private adjustPlayback(): void {
    if (this.stopped || this.errored || !this.sourceBuffer) return;
    const buffered = this.sourceBuffer.buffered;
    if (buffered.length === 0) return;

    const video = this.videoElement;
    const current = video.currentTime;
    const bufferedStart = buffered.start(0);
    const bufferedEnd = buffered.end(buffered.length - 1);
    const bufferAheadMs = (bufferedEnd - current) * 1000;
    const bufferBehindMs = (current - bufferedStart) * 1000;

    this._metrics.bufferedStart = bufferedStart;
    this._metrics.bufferedEnd = bufferedEnd;

    // Drop stale buffer behind the playhead.
    if (bufferBehindMs > this.maxBufferBehindMs && !this.cleanupInProgress && !this.sourceBuffer.updating) {
      const removeEnd = current - this.maxBufferBehindMs / 1000;
      if (removeEnd > bufferedStart) {
        try {
          this.sourceBuffer.remove(bufferedStart, Math.min(removeEnd, bufferedEnd));
          this.cleanupInProgress = true;
        } catch {
          // ignore
        }
      }
    }

    // Drop too far ahead to avoid unbounded growth.
    if (
      bufferAheadMs > this.maxBufferAheadMs &&
      !this.cleanupInProgress &&
      !this.sourceBuffer.updating
    ) {
      const removeStart = current + this.maxBufferAheadMs / 1000;
      if (removeStart < bufferedEnd) {
        try {
          this.sourceBuffer.remove(removeStart, bufferedEnd);
          this.cleanupInProgress = true;
        } catch {
          // ignore
        }
      }
    }

    // Live latency correction; skip rate/seek nudging in VOD or display-paused mode.
    if (this.isLive && !this.displayPaused) {
      if (bufferAheadMs > this.liveLatencyTargetMs + this.liveDriftLargeMs) {
        const target = bufferedEnd - this.liveLatencyTargetMs / 1000;
        video.currentTime = Math.max(bufferedStart, target);
        // Reset playback rate immediately after a catch-up seek to avoid a
        // brief fast-forward while the nudge logic slowly returns to 1.0.
        video.playbackRate = 1.0;
        this._metrics.seekCount += 1;
      } else if (bufferAheadMs > this.liveLatencyTargetMs + this.liveDriftSmallMs) {
        video.playbackRate = Math.min(video.playbackRate + 0.05, this.maxPlaybackRate);
      } else if (bufferAheadMs < this.liveLatencyTargetMs - this.liveDriftSmallMs) {
        video.playbackRate = Math.max(video.playbackRate - 0.05, this.minPlaybackRate);
      } else {
        // Nudge back toward 1.0 when close to the target.
        if (video.playbackRate > 1.0) {
          video.playbackRate = Math.max(video.playbackRate - 0.05, 1.0);
        } else if (video.playbackRate < 1.0) {
          video.playbackRate = Math.min(video.playbackRate + 0.05, 1.0);
        }
      }
    }

    // Resume playback if paused and we have a useful buffer.
    if (!this.displayPaused && video.paused && bufferAheadMs > this.liveDriftSmallMs) {
      // play() is async; a synchronous try/catch cannot catch a rejected
      // promise caused by autoplay policy, so attach a no-op rejection handler.
      video.play?.()?.catch(() => undefined);
    }
  }

  private processQueue(): void {
    if (
      this.stopped ||
      this.errored ||
      !this.sourceBuffer ||
      this.sourceBuffer.updating ||
      this.cleanupInProgress ||
      this.appendQueue.length === 0
    ) {
      return;
    }

    const task = this.appendQueue[0]!;
    if (task.generation !== this.generation) {
      this.appendQueue.shift();
      this._metrics.droppedSegments += 1;
      this.processQueue();
      return;
    }

    if (task.type === 'append') {
      this.performAppend(task);
    }
  }

  private performAppend(task: AppendTask): void {
    if (this.stopped || this.errored || !this.sourceBuffer) return;
    const sourceBuffer = this.sourceBuffer;

    try {
      if (task.isInit && typeof sourceBuffer.changeType === 'function') {
        sourceBuffer.changeType(this.mime);
      }
      if (task.timestampOffset !== undefined) {
        sourceBuffer.timestampOffset = task.timestampOffset / 1000;
      }
      if (task.appendWindowStart !== undefined) {
        sourceBuffer.appendWindowStart = task.appendWindowStart / 1000;
      }
      if (task.appendWindowEnd !== undefined) {
        sourceBuffer.appendWindowEnd = task.appendWindowEnd / 1000;
      }
      sourceBuffer.appendBuffer(task.data);
    } catch (err) {
      if (isQuotaExceeded(err) && !this.cleanupInProgress && this.quotaRetryCount < this.maxQuotaRetries) {
        this.quotaRetryCount += 1;
        this._metrics.quotaCleanupCount += 1;
        if (!this.startBufferCleanup()) {
          // Nothing could be removed; fail the append immediately.
          this.appendQueue.shift();
          this.handleError(
            new MseError('quota-exceeded', 'QuotaExceededError and no removable buffered range'),
            'quota-exceeded',
          );
        }
        return;
      }
      this.appendQueue.shift();
      const error = err instanceof Error ? err : new Error(String(err));
      this.handleError(new MseError('append-error', error.message), 'append-error');
    }
  }

  private startBufferCleanup(): boolean {
    if (this.cleanupInProgress || !this.sourceBuffer || this.sourceBuffer.updating || this.sourceBuffer.buffered.length === 0) {
      return false;
    }
    const buffered = this.sourceBuffer.buffered;
    const current = this.videoElement.currentTime;
    const bufferedStart = buffered.start(0);
    const bufferedEnd = buffered.end(buffered.length - 1);

    // Remove from the oldest buffered region up to just behind the playhead.
    const removeEnd = current - this.maxBufferBehindMs / 1000;
    if (removeEnd > bufferedStart) {
      try {
        this.sourceBuffer.remove(bufferedStart, Math.min(removeEnd, bufferedEnd));
        this.cleanupInProgress = true;
        return true;
      } catch {
        // ignore; try ahead
      }
    }

    // If we cannot free anything behind, remove a portion ahead instead.
    const removeStart = current + this.maxBufferAheadMs / 1000;
    if (removeStart < bufferedEnd) {
      try {
        this.sourceBuffer.remove(removeStart, bufferedEnd);
        this.cleanupInProgress = true;
        return true;
      } catch {
        // ignore
      }
    }

    return false;
  }

  private handleError(error: Error | MseError, code: MseErrorCode): void {
    if (this.stopped || this.errored) return;
    this.errored = true;
    this._metrics.lastErrorCode = code;
    if (error instanceof MseError) {
      this.callbacks.onError?.(error);
    } else {
      this.callbacks.onError?.(new MseError(code, error.message));
    }
    this.stop().catch(() => undefined);
  }

  private readonly onSourceBufferUpdateEnd = (): void => {
    if (this.stopped || this.errored) return;
    if (this.cleanupInProgress) {
      this.cleanupInProgress = false;
      this.processQueue();
      return;
    }
    if (this.appendQueue.length > 0) {
      this.appendQueue.shift();
      // A successful (non-cleanup) SourceBuffer operation completed; reset the
      // per-append quota retry budget so future appends can recover again.
      this.quotaRetryCount = 0;
    }
    this.processQueue();
  };

  private readonly onSourceBufferError = (event?: unknown): void => {
    const error = this.eventToError(event, 'source-buffer-error');
    if (isQuotaExceeded(error) && !this.cleanupInProgress && this.quotaRetryCount < this.maxQuotaRetries) {
      this.quotaRetryCount += 1;
      this._metrics.quotaCleanupCount += 1;
      if (!this.startBufferCleanup()) {
        this.handleError(
          new MseError('quota-exceeded', 'QuotaExceededError and no removable buffered range'),
          'quota-exceeded',
        );
      }
      return;
    }
    this.handleError(error, 'source-buffer-error');
  };

  private readonly onSourceBufferAbort = (): void => {
    if (this.stopped || this.errored) return;
    this.processQueue();
  };

  private readonly onMediaSourceError = (): void => {
    this.handleError(new MseError('media-source-error', 'MediaSource error'), 'media-source-error');
  };

  private readonly onVideoError = (): void => {
    const code = this.videoElement.error?.code ?? 0;
    const message = mediaErrorMessage(code);
    this.handleError(new MseError('video-element-error', message), 'video-element-error');
  };

  private eventToError(event: unknown, defaultCode: MseErrorCode): Error {
    if (event instanceof Error) return event;
    if (event !== null && typeof event === 'object' && 'error' in event) {
      const nested = (event as { error: unknown }).error;
      if (nested instanceof Error) return nested;
      return new Error(String(nested ?? defaultCode));
    }
    return new Error(defaultCode);
  }
}

export function mseBackendFactory(options: MseBackendOptions): (ctx: BackendContext) => MseBackend {
  return (ctx: BackendContext) =>
    new MseBackend(ctx, {
      ...options,
      isLive: options.isLive ?? ctx.candidate.isLive ?? true,
    });
}
