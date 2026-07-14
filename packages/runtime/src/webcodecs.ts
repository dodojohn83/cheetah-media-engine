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
  pendingDecodes: number;
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

function isAnnexBKeyFrame(data: Uint8Array, codec: string): boolean | undefined {
  let offset = 0;
  let found = false;
  // Allow 3- and 4-byte start codes and protect the NAL header read.
  while (offset + 3 <= data.length) {
    const b0 = data[offset]!;
    const b1 = data[offset + 1]!;
    const b2 = data[offset + 2]!;
    if (b0 === 0 && b1 === 0 && b2 === 1) {
      offset += 3;
      found = true;
      break;
    }
    if (offset + 4 <= data.length) {
      const b3 = data[offset + 3]!;
      if (b0 === 0 && b1 === 0 && b2 === 0 && b3 === 1) {
        offset += 4;
        found = true;
        break;
      }
    }
    offset += 1;
  }
  if (!found || offset >= data.length) return undefined;

  const header = data[offset]!;
  const c = codec.toLowerCase();
  if (c.startsWith('avc') || c === 'h264') {
    const nalType = header & 0x1f;
    return nalType === 5;
  }
  if (c.startsWith('hvc') || c === 'h265' || c === 'hevc') {
    const nalType = (header >> 1) & 0x3f;
    // BLA / IDR / CRA NAL types are random access points.
    return nalType >= 16 && nalType <= 23;
  }
  return undefined;
}

function guessKeyFrame(data: Uint8Array, codec: string): boolean {
  const annexB = isAnnexBKeyFrame(data, codec);
  if (annexB !== undefined) return annexB;
  // For length-prefixed or unknown formats, conservatively treat as delta.
  return false;
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
  private _metrics: MutableMetrics = {
    decodedVideoFrames: 0,
    decodedAudioFrames: 0,
    droppedChunks: 0,
    pendingDecodes: 0,
  };

  constructor(_ctx: BackendContext, options: WebCodecsBackendOptions) {
    this.tracks = options.tracks;
    this.callbacks = options.callbacks;
    this.maxPending = options.maxPendingDecodes ?? 32;
  }

  get metrics(): WebCodecsMetrics {
    return this._metrics;
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
      this.generation += 1;
      this.pendingReconfigure = false;
      throw err;
    }

    this.configured = true;
  }

  pushVideo(data: Uint8Array, timestamp: number, opts?: { isKeyFrame?: boolean; duration?: number }): void {
    if (this.stopped || this.closing || this.errored || !this.videoDecoder || !this.videoConfig) return;

    if (this._metrics.pendingDecodes >= this.maxPending) {
      this._metrics.droppedChunks += 1;
      return;
    }

    const EncodedVideoChunk = getGlobal<unknown>('EncodedVideoChunk') as EncodedVideoChunkConstructor | undefined;
    if (!EncodedVideoChunk) return;

    const type = opts?.isKeyFrame ?? guessKeyFrame(data, this.videoConfig.codec) ? 'key' : 'delta';
    if (type === 'key' && this.pendingReconfigure) {
      this.reconfigureVideo();
    }

    const chunk = new EncodedVideoChunk({
      type,
      timestamp,
      duration: opts?.duration,
      data,
    });

    this._metrics.pendingDecodes += 1;
    this.videoDecoder.decode(chunk);
  }

  pushAudio(data: Uint8Array, timestamp: number, opts?: { duration?: number }): void {
    if (this.stopped || this.closing || this.errored || !this.audioDecoder || !this.audioConfig) return;

    if (this._metrics.pendingDecodes >= this.maxPending) {
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

    this._metrics.pendingDecodes += 1;
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

  private reconfigureVideo(): void {
    if (!this.videoDecoder || !this.videoConfig) return;
    this.pendingReconfigure = false;
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
    decoder.close();
  }

  private handleVideoOutput(frame: CloseableVideoFrame, gen: number): void {
    this._metrics.pendingDecodes -= 1;
    if (this.stopped || gen !== this.generation) {
      frame.close();
      return;
    }
    this._metrics.decodedVideoFrames += 1;
    this.callbacks.onVideoFrame?.(frame);
  }

  private handleAudioOutput(data: CloseableAudioData, gen: number): void {
    this._metrics.pendingDecodes -= 1;
    if (this.stopped || gen !== this.generation) {
      data.close();
      return;
    }
    this._metrics.decodedAudioFrames += 1;
    this.callbacks.onAudioData?.(data);
  }

  private handleError(error: Error): void {
    if (this.stopped || this.errored) return;
    this.errored = true;
    // Pending decodes will never complete once the decoder is in an error state;
    // reset the counter so the bounded queue does not stay saturated.
    this._metrics.pendingDecodes = 0;
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
