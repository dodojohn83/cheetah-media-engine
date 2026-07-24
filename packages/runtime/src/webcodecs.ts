/**
 * WebCodecs backend for the browser runtime.
 *
 * Wraps `VideoDecoder` / `AudioDecoder` with a bounded input queue, lifecycle
 * management, epoch filtering and output ownership hooks.  It is intentionally
 * decoupled from the fallback controller: the controller only sees the
 * `MediaBackend` surface (`configure` / `stop` / `identity`), while the runtime
 * feeds encoded chunks through the `pushVideo` / `pushAudio` methods.
 */

import type { Backend, TrackProfile } from './planner';
import type { BackendContext, MediaBackend } from './fallback';

export interface CloseableVideoFrame {
  readonly timestamp: number;
  readonly codedWidth: number;
  readonly codedHeight: number;
  readonly format: string | null;
  close(): void;
}

export interface CloseableAudioData {
  readonly timestamp: number;
  readonly numberOfFrames: number;
  readonly sampleRate: number;
  readonly numberOfChannels: number;
  close(): void;
}

export interface WebCodecsCallbacks {
  readonly onVideoFrame?: (frame: CloseableVideoFrame) => void;
  readonly onAudioData?: (data: CloseableAudioData) => void;
  readonly onError?: (error: Error) => void;
}

export interface WebCodecsBackendOptions {
  readonly tracks: readonly TrackProfile[];
  readonly callbacks: WebCodecsCallbacks;
  readonly maxPendingDecodes?: number;
  readonly maxVideoQueue?: number;
}

export interface WebCodecsMetrics {
  readonly decodedVideoFrames: number;
  readonly decodedAudioFrames: number;
  readonly droppedChunks: number;
  readonly pendingDecodes: number;
}

type MutableMetrics = {
  decodedVideoFrames: number;
  decodedAudioFrames: number;
  droppedChunks: number;
};

// Minimal WebCodecs type surface; real browser objects are structurally
// compatible with these interfaces.
interface EncodedVideoChunkInit {
  readonly type: 'key' | 'delta';
  readonly timestamp: number;
  readonly duration?: number | undefined;
  readonly data: ArrayBufferView;
}

interface EncodedVideoChunkLike {}

interface EncodedVideoChunkConstructor {
  new (init: EncodedVideoChunkInit): EncodedVideoChunkLike;
}

interface EncodedAudioChunkInit {
  readonly type: 'key' | 'delta';
  readonly timestamp: number;
  readonly duration?: number | undefined;
  readonly data: ArrayBufferView;
  readonly numberOfFrames?: number | undefined;
}

interface EncodedAudioChunkLike {}

interface EncodedAudioChunkConstructor {
  new (init: EncodedAudioChunkInit): EncodedAudioChunkLike;
}

interface VideoDecoderConfig {
  readonly codec: string;
  readonly description?: Uint8Array | undefined;
  readonly codedWidth?: number | undefined;
  readonly codedHeight?: number | undefined;
  readonly hardwareAcceleration?: string | undefined;
  readonly optimizeForLatency?: boolean | undefined;
}

interface VideoDecoderInit {
  readonly output: (frame: CloseableVideoFrame) => void;
  readonly error: (error: Error) => void;
}

interface VideoDecoderLike {
  readonly state: string;
  configure(config: VideoDecoderConfig): void;
  decode(chunk: EncodedVideoChunkLike): void;
  flush(): Promise<void>;
  reset(): void;
  close(): void;
}

interface VideoDecoderConstructor {
  new (init: VideoDecoderInit): VideoDecoderLike;
  isConfigSupported(config: VideoDecoderConfig): Promise<{ supported: boolean }>;
}

interface AudioDecoderConfig {
  readonly codec: string;
  readonly description?: Uint8Array | undefined;
  readonly sampleRate: number;
  readonly numberOfChannels: number;
}

interface AudioDecoderInit {
  readonly output: (data: CloseableAudioData) => void;
  readonly error: (error: Error) => void;
}

interface AudioDecoderLike {
  readonly state: string;
  configure(config: AudioDecoderConfig): void;
  decode(chunk: EncodedAudioChunkLike): void;
  flush(): Promise<void>;
  reset(): void;
  close(): void;
}

interface AudioDecoderConstructor {
  new (init: AudioDecoderInit): AudioDecoderLike;
  isConfigSupported(config: AudioDecoderConfig): Promise<{ supported: boolean }>;
}

function getGlobal<T>(name: string): T | undefined {
  if (typeof globalThis === 'undefined') return undefined;
  try {
    const value = (globalThis as unknown as Record<string, T>)[name];
    return value ?? undefined;
  } catch {
    return undefined;
  }
}

function codecString(track: TrackProfile): string {
  const c = track.codec.toLowerCase();
  if (c.startsWith('avc')) return 'avc1.42001e';
  if (c.startsWith('hvc') || c.startsWith('hev')) return 'hvc1.1.6.l93.b0';
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

function isAudioCodec(codec: string): boolean {
  const c = codec.toLowerCase();
  return ['aac', 'mp3', 'g711a', 'g711u', 'alaw', 'ulaw'].includes(c) || c.startsWith('mp4a') || c === 'mp3';
}

function buildVideoConfig(track: TrackProfile): VideoDecoderConfig {
  return {
    codec: codecString(track),
    codedWidth: track.width,
    codedHeight: track.height,
    hardwareAcceleration: 'no-preference',
    optimizeForLatency: true,
  };
}

function buildAudioConfig(track: TrackProfile): AudioDecoderConfig {
  return {
    codec: codecString(track),
    sampleRate: track.sampleRate ?? 48000,
    numberOfChannels: track.channels ?? 2,
  };
}

function findStartCode(data: Uint8Array, from: number): [start: number, end: number] | undefined {
  let offset = from;
  while (offset + 3 <= data.length) {
    const b0 = data[offset]!;
    const b1 = data[offset + 1]!;
    const b2 = data[offset + 2]!;
    if (b0 === 0 && b1 === 0 && b2 === 1) {
      return [offset, offset + 3];
    }
    if (offset + 4 <= data.length) {
      const b3 = data[offset + 3]!;
      if (b0 === 0 && b1 === 0 && b2 === 0 && b3 === 1) {
        return [offset, offset + 4];
      }
    }
    offset += 1;
  }
  return undefined;
}

function isAnnexBKeyFrame(data: Uint8Array, codec: string): boolean | undefined {
  const c = codec.toLowerCase();
  const isH264 = c.startsWith('avc') || c === 'h264';
  const isH265 = c.startsWith('hvc') || c === 'h265' || c === 'hevc';
  if (!isH264 && !isH265) return undefined;

  let foundStart = false;
  let offset = 0;
  while (offset + 3 <= data.length) {
    const start = findStartCode(data, offset);
    if (!start) break;
    foundStart = true;
    const end = start[1];
    if (end >= data.length) return undefined; // start code without a NAL header

    const header = data[end]!;
    offset = end + 1;
    if (isH264) {
      const nalType = header & 0x1f;
      if (nalType === 5) return true;
    } else {
      const nalType = (header >> 1) & 0x3f;
      // BLA / IDR / CRA NAL types are random access points.
      if (nalType >= 16 && nalType <= 23) return true;
    }
  }
  return foundStart ? false : undefined;
}

function guessKeyFrame(data: Uint8Array, codec: string): boolean {
  const annexB = isAnnexBKeyFrame(data, codec);
  if (annexB !== undefined) return annexB;
  // For length-prefixed or unknown formats, conservatively treat as delta.
  return false;
}

function validatePositiveFiniteInteger(value: number, name: string): void {
  if (!Number.isFinite(value) || value <= 0 || !Number.isInteger(value)) {
    throw new Error(`${name} must be a finite positive integer`);
  }
}

function validateWebCodecsOptions(options: unknown): void {
  if (!options || typeof options !== 'object') {
    throw new Error('WebCodecsBackendOptions is required');
  }
  const o = options as Partial<WebCodecsBackendOptions>;
  if (!Array.isArray(o.tracks) || o.tracks.length === 0) {
    throw new Error('tracks must be a non-empty array');
  }
  for (let i = 0; i < o.tracks.length; i += 1) {
    const track = o.tracks[i] as unknown;
    if (!track || typeof track !== 'object') {
      throw new Error(`track at index ${i} must be an object`);
    }
    const t = track as {
      kind?: unknown;
      codec?: unknown;
      width?: unknown;
      height?: unknown;
      sampleRate?: unknown;
      channels?: unknown;
    };
    if (t.kind !== 'video' && t.kind !== 'audio') {
      throw new Error(`track at index ${i} must have kind 'video' or 'audio'`);
    }
    if (typeof t.codec !== 'string' || t.codec.length === 0) {
      throw new Error(`track at index ${i} must have a non-empty codec string`);
    }
    if (t.kind === 'video') {
      if (t.width !== undefined) {
        validatePositiveFiniteInteger(t.width as number, `track ${i} width`);
      }
      if (t.height !== undefined) {
        validatePositiveFiniteInteger(t.height as number, `track ${i} height`);
      }
    } else {
      if (t.sampleRate !== undefined) {
        validatePositiveFiniteInteger(t.sampleRate as number, `track ${i} sampleRate`);
      }
      if (t.channels !== undefined) {
        validatePositiveFiniteInteger(t.channels as number, `track ${i} channels`);
      }
    }
  }
  if (o.callbacks !== undefined) {
    if (typeof o.callbacks !== 'object' || o.callbacks === null) {
      throw new Error('callbacks must be an object');
    }
    const c = o.callbacks;
    if (c.onVideoFrame !== undefined && typeof c.onVideoFrame !== 'function') {
      throw new Error('callbacks.onVideoFrame must be a function');
    }
    if (c.onAudioData !== undefined && typeof c.onAudioData !== 'function') {
      throw new Error('callbacks.onAudioData must be a function');
    }
    if (c.onError !== undefined && typeof c.onError !== 'function') {
      throw new Error('callbacks.onError must be a function');
    }
  }
  if (o.maxPendingDecodes !== undefined) {
    if (!Number.isFinite(o.maxPendingDecodes) || !Number.isInteger(o.maxPendingDecodes) || o.maxPendingDecodes < 1) {
      throw new Error('maxPendingDecodes must be a finite positive integer');
    }
  }
  if (o.maxVideoQueue !== undefined) {
    if (!Number.isFinite(o.maxVideoQueue) || !Number.isInteger(o.maxVideoQueue) || o.maxVideoQueue < 1) {
      throw new Error('maxVideoQueue must be a finite positive integer');
    }
  }
}

function isBufferSource(data: unknown): boolean {
  return ArrayBuffer.isView(data) || data instanceof ArrayBuffer;
}

function validatePushVideoArgs(data: unknown, timestamp: unknown, opts?: unknown): void {
  if (!isBufferSource(data)) {
    throw new Error('pushVideo data must be a Uint8Array or ArrayBuffer');
  }
  if (typeof timestamp !== 'number' || !Number.isFinite(timestamp)) {
    throw new Error('pushVideo timestamp must be a finite number');
  }
  if (opts !== undefined) {
    if (typeof opts !== 'object' || opts === null) {
      throw new Error('pushVideo opts must be an object');
    }
    const o = opts as { isKeyFrame?: unknown; duration?: unknown };
    if (o.isKeyFrame !== undefined && typeof o.isKeyFrame !== 'boolean') {
      throw new Error('pushVideo isKeyFrame must be a boolean');
    }
    if (o.duration !== undefined && (typeof o.duration !== 'number' || !Number.isFinite(o.duration) || o.duration < 0)) {
      throw new Error('pushVideo duration must be a finite non-negative number');
    }
  }
}

function validatePushAudioArgs(data: unknown, timestamp: unknown, opts?: unknown): void {
  if (!isBufferSource(data)) {
    throw new Error('pushAudio data must be a Uint8Array or ArrayBuffer');
  }
  if (typeof timestamp !== 'number' || !Number.isFinite(timestamp)) {
    throw new Error('pushAudio timestamp must be a finite number');
  }
  if (opts !== undefined) {
    if (typeof opts !== 'object' || opts === null) {
      throw new Error('pushAudio opts must be an object');
    }
    const o = opts as { duration?: unknown };
    if (o.duration !== undefined && (typeof o.duration !== 'number' || !Number.isFinite(o.duration) || o.duration < 0)) {
      throw new Error('pushAudio duration must be a finite non-negative number');
    }
  }
}

export class WebCodecsBackend implements MediaBackend {
  readonly identity: Backend = 'webcodecs';

  private readonly tracks: readonly TrackProfile[];
  private readonly callbacks: WebCodecsCallbacks;
  private readonly maxPending: number;
  private generation = 0;
  private stopped = true;
  private closing = false;
  private errored = false;
  private configured = false;
  private videoConfig: VideoDecoderConfig | undefined = undefined;
  private audioConfig: AudioDecoderConfig | undefined = undefined;
  private videoDecoder: VideoDecoderLike | undefined = undefined;
  private audioDecoder: AudioDecoderLike | undefined = undefined;
  private pendingReconfigure = false;
  private _pendingVideo = 0;
  private _pendingAudio = 0;
  private displayPaused = false;
  private keepConnectionOnPause = false;
  private frameStepResolver: (() => void) | undefined = undefined;
  private frameStepRejecter: ((error: Error) => void) | undefined = undefined;
  private frameStepKeyframeOnly = false;
  private frameStepDecodeDispatched = false;
  private readonly maxVideoQueue: number;
  private videoInputQueue: {
    readonly data: Uint8Array;
    readonly timestamp: number;
    readonly isKeyFrame?: boolean;
    readonly duration?: number;
  }[] = [];
  private _metrics: MutableMetrics = {
    decodedVideoFrames: 0,
    decodedAudioFrames: 0,
    droppedChunks: 0,
  };

  constructor(_ctx: BackendContext, options: WebCodecsBackendOptions) {
    validateWebCodecsOptions(options);
    this.tracks = options.tracks;
    this.callbacks = options.callbacks ?? {};
    this.maxPending = options.maxPendingDecodes ?? 32;
    this.maxVideoQueue = options.maxVideoQueue ?? options.maxPendingDecodes ?? 32;
    if (!Number.isInteger(this.maxPending) || this.maxPending < 1) {
      throw new Error('maxPendingDecodes must be a finite positive integer');
    }
    if (!Number.isInteger(this.maxVideoQueue) || this.maxVideoQueue < 1) {
      throw new Error('maxVideoQueue must be a finite positive integer');
    }
  }

  private totalPending(): number {
    return this._pendingVideo + this._pendingAudio;
  }

  get metrics(): WebCodecsMetrics {
    return {
      ...this._metrics,
      pendingDecodes: this.totalPending(),
    };
  }

  async configure(): Promise<void> {
    if (this.configured) {
      throw new Error('WebCodecsBackend already configured');
    }

    const VideoDecoder = getGlobal<unknown>('VideoDecoder') as VideoDecoderConstructor | undefined;
    const AudioDecoder = getGlobal<unknown>('AudioDecoder') as AudioDecoderConstructor | undefined;

    const videoTrack = this.tracks.find((t) => t.kind === 'video');
    const audioTrack = this.tracks.find((t) => t.kind === 'audio');

    if (videoTrack) {
      if (!VideoDecoder) {
        throw new Error('WebCodecs API not available (VideoDecoder missing)');
      }
      this.videoConfig = buildVideoConfig(videoTrack);
      const support = await VideoDecoder.isConfigSupported(this.videoConfig);
      if (!support.supported) {
        throw new Error(`VideoDecoder does not support ${this.videoConfig.codec}`);
      }
    }

    if (audioTrack && isAudioCodec(audioTrack.codec)) {
      if (!AudioDecoder) {
        throw new Error('WebCodecs API not available (AudioDecoder missing)');
      }
      this.audioConfig = buildAudioConfig(audioTrack);
      const support = await AudioDecoder.isConfigSupported(this.audioConfig);
      if (!support.supported) {
        throw new Error(`AudioDecoder does not support ${this.audioConfig.codec}`);
      }
    }

    this.generation += 1;
    this.stopped = false;

    try {
      if (this.videoConfig) {
        if (!VideoDecoder) throw new Error('VideoDecoder disappeared during configure');
        const gen = this.generation;
        this.videoDecoder = new VideoDecoder({
          output: (frame) => this.handleVideoOutput(frame, gen),
          error: (err) => this.handleError(err),
        });
        this.videoDecoder.configure(this.videoConfig);
      }

      if (this.audioConfig) {
        if (!AudioDecoder) throw new Error('AudioDecoder disappeared during configure');
        const gen = this.generation;
        this.audioDecoder = new AudioDecoder({
          output: (data) => this.handleAudioOutput(data, gen),
          error: (err) => this.handleError(err),
        });
        this.audioDecoder.configure(this.audioConfig);
      }
    } catch (err) {
      // Close any decoder that was created before the failure so hardware
      // resources are not leaked.
      await this.closeDecoder(this.videoDecoder);
      this.videoDecoder = undefined;
      await this.closeDecoder(this.audioDecoder);
      this.audioDecoder = undefined;
      this.stopped = true;
      this.configured = false;
      this._pendingVideo = 0;
      this._pendingAudio = 0;
      this.generation += 1;
      this.pendingReconfigure = false;
      throw err;
    }

    this.configured = true;
  }

  pushVideo(data: Uint8Array, timestamp: number, opts?: { isKeyFrame?: boolean; duration?: number }): void {
    if (this.stopped || this.closing || this.errored || !this.videoDecoder || !this.videoConfig) return;

    validatePushVideoArgs(data, timestamp, opts);

    const isKeyFrame = opts?.isKeyFrame ?? guessKeyFrame(data, this.videoConfig.codec);

    // When a frame step is pending, decode exactly one matching chunk and
    // queue/drop any additional incoming chunks so the decoder stays in order.
    if (this.displayPaused && this.frameStepResolver && !this.frameStepDecodeDispatched) {
      if (this.frameStepKeyframeOnly && !isKeyFrame) {
        // Drop non-keyframes while waiting for the next keyframe.
        this._metrics.droppedChunks += 1;
        return;
      }
      this.frameStepDecodeDispatched = true;
      this.decodeVideoChunk(data, timestamp, isKeyFrame, opts?.duration);
      return;
    }

    // While display is paused, suppress decoding to freeze the output.
    // When keepConnection is true we buffer a bounded window so that frame
    // stepping can advance; otherwise incoming chunks are dropped.
    if (this.displayPaused) {
      if (this.keepConnectionOnPause) {
        if (this.videoInputQueue.length >= this.maxVideoQueue) {
          // Drop the newest chunk so the buffered window stays contiguous from
          // the pause point; decoding a delta whose reference was discarded
          // would produce broken pictures.
          this._metrics.droppedChunks += 1;
          return;
        }
        const entry: { data: Uint8Array; timestamp: number; isKeyFrame: boolean; duration?: number } = {
          data,
          timestamp,
          isKeyFrame,
        };
        if (opts?.duration !== undefined) {
          entry.duration = opts.duration;
        }
        this.videoInputQueue.push(entry);
      } else {
        this._metrics.droppedChunks += 1;
      }
      return;
    }

    if (this.totalPending() >= this.maxPending) {
      this._metrics.droppedChunks += 1;
      return;
    }

    this.decodeVideoChunk(data, timestamp, isKeyFrame, opts?.duration);
  }

  private decodeVideoChunk(data: Uint8Array, timestamp: number, isKeyFrame: boolean, duration?: number): void {
    if (!this.videoDecoder || !this.videoConfig) return;
    const EncodedVideoChunk = getGlobal<unknown>('EncodedVideoChunk') as EncodedVideoChunkConstructor | undefined;
    if (!EncodedVideoChunk) return;

    const type = isKeyFrame ? 'key' : 'delta';
    if (type === 'key' && this.pendingReconfigure) {
      this.reconfigureVideo();
    }

    const chunk = new EncodedVideoChunk({
      type,
      timestamp,
      duration,
      data,
    });

    this._pendingVideo += 1;
    this.videoDecoder.decode(chunk);
  }

  pushAudio(data: Uint8Array, timestamp: number, opts?: { duration?: number }): void {
    if (this.stopped || this.closing || this.errored || !this.audioDecoder || !this.audioConfig) return;

    validatePushAudioArgs(data, timestamp, opts);

    // Drop audio while the display is paused to keep it in sync with the
    // frozen video picture.
    if (this.displayPaused) {
      this._metrics.droppedChunks += 1;
      return;
    }

    if (this.totalPending() >= this.maxPending) {
      this._metrics.droppedChunks += 1;
      return;
    }

    const EncodedAudioChunk = getGlobal<unknown>('EncodedAudioChunk') as EncodedAudioChunkConstructor | undefined;
    if (!EncodedAudioChunk) return;

    const chunk = new EncodedAudioChunk({
      type: 'key',
      timestamp,
      duration: opts?.duration,
      data,
    });

    this._pendingAudio += 1;
    this.audioDecoder.decode(chunk);
  }

  /**
   * Mark the video decoder configuration as stale; the next keyframe will
   * trigger a `reset()` and re-configure.  This avoids dropping in-flight
   * delta frames that still belong to the previous configuration.
   */
  markVideoConfigChanged(): void {
    this.pendingReconfigure = true;
  }

  async pauseDisplay(keepConnection = true): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new Error('Cannot pause display before configure');
    }
    if (typeof keepConnection !== 'boolean') {
      throw new Error('pauseDisplay keepConnection must be a boolean');
    }
    this.displayPaused = true;
    this.keepConnectionOnPause = keepConnection;
    if (!keepConnection) {
      // Stop decoding without tearing down the decoder objects.
      if (this.frameStepRejecter) {
        const rejecter = this.frameStepRejecter;
        this.frameStepRejecter = undefined;
        rejecter(new Error('Display paused without connection'));
      }
      this.frameStepResolver = undefined;
      this.frameStepDecodeDispatched = false;
      this.videoInputQueue = [];
      this.videoDecoder?.reset();
      this.audioDecoder?.reset();
      // reset() discards pending decodes; their output callbacks never fire.
      this._pendingVideo = 0;
      this._pendingAudio = 0;
    }
  }

  async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
    if (this.stopped || this.closing || this.errored || !this.configured) {
      throw new Error('Cannot frame step before configure');
    }
    if (direction !== 'forward' && direction !== 'backward') {
      throw new Error('frameStep direction must be forward or backward');
    }
    if (typeof keyframeOnly !== 'boolean') {
      throw new Error('frameStep keyframeOnly must be a boolean');
    }
    if (direction === 'backward') {
      throw new Error('Backward frame step requires a GOP cache and is not yet implemented');
    }
    if (!this.displayPaused) {
      throw new Error('frameStep requires pauseDisplay() to be active');
    }
    if (this.frameStepResolver) {
      throw new Error('A frame step is already pending');
    }

    // If we already have queued chunks, decode the next matching one now.
    return new Promise<void>((resolve, reject) => {
      this.frameStepResolver = resolve;
      this.frameStepRejecter = reject;
      this.frameStepKeyframeOnly = keyframeOnly;
      while (this.videoInputQueue.length > 0) {
        const entry = this.videoInputQueue.shift()!;
        if (keyframeOnly && !entry.isKeyFrame) {
          continue;
        }
        this.frameStepDecodeDispatched = true;
        this.decodeVideoChunk(entry.data, entry.timestamp, entry.isKeyFrame ?? false, entry.duration);
        return;
      }
      // If the queue was empty, pushVideo will decode the next incoming chunk
      // and the output handler will resolve this promise.
    });
  }

  private reconfigureVideo(): void {
    if (!this.videoDecoder || !this.videoConfig) return;
    this.pendingReconfigure = false;
    // `reset()` discards all pending video decode operations and their output
    // callbacks will never fire, so we must zero only the video counter.
    this._pendingVideo = 0;
    // `reset()` keeps the decoder object but clears internal state; reconfigure
    // with the current (presumably updated) description.
    this.videoDecoder.reset();
    this.videoDecoder.configure(this.videoConfig);
  }

  async stop(): Promise<void> {
    if (this.stopped || this.closing) return;
    this.closing = true;

    await this.closeDecoder(this.videoDecoder);
    this.videoDecoder = undefined;
    await this.closeDecoder(this.audioDecoder);
    this.audioDecoder = undefined;

    this.stopped = true;
    this.closing = false;
    this.errored = false;
    this.configured = false;
    this._pendingVideo = 0;
    this._pendingAudio = 0;
    this.displayPaused = false;
    this.keepConnectionOnPause = false;
    // Reject any in-flight frame-step request so callers do not hang.
    if (this.frameStepRejecter) {
      const rejecter = this.frameStepRejecter;
      this.frameStepRejecter = undefined;
      rejecter(new Error('Backend stopped during frame step'));
    }
    this.frameStepResolver = undefined;
    this.frameStepKeyframeOnly = false;
    this.frameStepDecodeDispatched = false;
    this.videoInputQueue = [];
    this.generation += 1;
    this.pendingReconfigure = false;
  }

  private async closeDecoder(decoder: VideoDecoderLike | AudioDecoderLike | undefined): Promise<void> {
    if (!decoder) return;
    try {
      // Flush so that pending outputs are delivered before close().  If the
      // decoder is already in an error state, flush() may throw; close anyway.
      await decoder.flush();
    } catch {
      // ignore
    }
    try {
      decoder.close();
    } catch {
      // ignore – decoder may already be closed (e.g. after an error)
    }
  }

  private handleVideoOutput(frame: CloseableVideoFrame, gen: number): void {
    if (this.stopped || this.errored || gen !== this.generation) {
      frame.close();
      return;
    }
    this._pendingVideo -= 1;
    this._metrics.decodedVideoFrames += 1;

    // During a frame step, emit this single frame and then return to the
    // paused state. The renderer can take ownership of the frame.
    if (this.frameStepResolver) {
      const resolver = this.frameStepResolver;
      this.frameStepResolver = undefined;
      this.frameStepRejecter = undefined;
      this.frameStepKeyframeOnly = false;
      this.frameStepDecodeDispatched = false;
      const onVideoFrame = this.callbacks.onVideoFrame;
      if (onVideoFrame) {
        try {
          onVideoFrame(frame);
        } catch (err) {
          frame.close();
          this.handleError(err instanceof Error ? err : new Error(String(err)));
          resolver();
          return;
        }
      } else {
        frame.close();
      }
      resolver();
      return;
    }

    // While display is paused, suppress new frames so the renderer keeps the
    // last displayed picture. Close the frame to release GPU-backed memory.
    if (this.displayPaused) {
      frame.close();
      return;
    }

    const onVideoFrame = this.callbacks.onVideoFrame;
    if (!onVideoFrame) {
      // No consumer registered; close the GPU-backed frame immediately.
      frame.close();
      return;
    }
    try {
      onVideoFrame(frame);
    } catch (err) {
      // The callback failed to take ownership of the frame; close it so the
      // GPU-backed resource is not leaked.
      frame.close();
      this.handleError(err instanceof Error ? err : new Error(String(err)));
    }
  }

  private handleAudioOutput(data: CloseableAudioData, gen: number): void {
    if (this.stopped || this.errored || gen !== this.generation) {
      data.close();
      return;
    }
    this._pendingAudio -= 1;
    // Suppress audio while the display is paused; it would be out of sync with
    // the frozen video picture.
    if (this.displayPaused) {
      data.close();
      return;
    }
    this._metrics.decodedAudioFrames += 1;
    const onAudioData = this.callbacks.onAudioData;
    if (!onAudioData) {
      data.close();
      return;
    }
    try {
      onAudioData(data);
    } catch (err) {
      data.close();
      this.handleError(err instanceof Error ? err : new Error(String(err)));
    }
  }

  private handleError(error: Error): void {
    if (this.stopped || this.errored) return;
    this.errored = true;
    // Pending decodes will never complete once the decoder is in an error state;
    // reset the counters so the bounded queue does not stay saturated.
    this._pendingVideo = 0;
    this._pendingAudio = 0;
    this.callbacks.onError?.(error);
    // Begin async cleanup; the fallback controller will normally replace this
    // backend, but stopping here prevents further decode() calls on a broken
    // decoder.
    this.stop().catch(() => undefined);
  }
}

export interface WebCodecsBackendFactoryOptions {
  readonly tracks: readonly TrackProfile[];
  readonly callbacks: WebCodecsCallbacks;
  readonly maxPendingDecodes?: number;
}

export function webcodecsBackendFactory(options: WebCodecsBackendFactoryOptions): (ctx: BackendContext) => WebCodecsBackend {
  return (ctx: BackendContext) => new WebCodecsBackend(ctx, options);
}
