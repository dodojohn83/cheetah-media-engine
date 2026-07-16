/**
 * Browser microphone capture pipeline.
 *
 * Probes `getUserMedia`/`AudioWorklet`, requests a microphone stream, pulls
 * Float32 samples through an AudioWorklet processor, resamples to the target
 * rate, encodes to G.711 A-law or mu-law and emits `AudioPacket`s.
 */

import { AudioResampler } from './resampler';
import { encodeG711F32, type G711Kind } from './g711';
import { buildWorkletBlobUrl, getCaptureProcessorSource } from './worklet';

export interface AudioPacket {
  readonly kind: G711Kind;
  readonly payload: Uint8Array;
  readonly timestampMs: number;
  readonly sampleRate: number;
  readonly channels: number;
}

export interface MicrophoneCaptureCallbacks {
  readonly onPacket?: ((packet: AudioPacket) => void) | undefined;
  readonly onError?: ((error: CaptureError) => void) | undefined;
}

export interface MicrophoneCaptureOptions {
  /** A-law or mu-law encoding (default: 'mulaw'). */
  readonly encoder?: G711Kind | undefined;
  /** Output sample rate in Hz (default: 8000). */
  readonly sampleRate?: number | undefined;
  /** Frame duration in milliseconds (default: 20). */
  readonly frameDurationMs?: number | undefined;
  /** Maximum number of resampled frames to buffer before dropping. */
  readonly maxBufferedFrames?: number | undefined;
  /** Optional pre-built worklet source URL (for tests). */
  readonly workletSourceUrl?: string | undefined;
  /** Optional AudioWorkletNode constructor (for tests). */
  readonly workletNodeCtor?: AudioWorkletNodeConstructor | undefined;
  /** Optional AudioContext instance (for tests). */
  readonly audioContext?: AudioContextLike | undefined;
  /** Optional getUserMedia implementation (for tests). */
  readonly getUserMedia?:
    | ((constraints: MediaStreamConstraints) => Promise<MediaStream>)
    | undefined;
}

export interface AudioContextLike {
  readonly sampleRate: number;
  readonly state: 'closed' | 'running' | 'suspended';
  resume(): Promise<void>;
  close(): Promise<void>;
  readonly audioWorklet: AudioWorkletLike;
  createMediaStreamSource(stream: MediaStream): MediaStreamAudioSourceNodeLike;
}

export interface AudioContextConstructor {
  new (options?: { readonly sampleRate?: number }): AudioContextLike;
}

export interface AudioWorkletLike {
  addModule(moduleURL: string): Promise<void>;
}

export interface MediaStreamAudioSourceNodeLike {
  connect(node: AudioWorkletNodeLike): void;
  disconnect(): void;
}

export interface AudioWorkletNodeLike {
  readonly port: MessagePortLike;
  connect(destination: AudioDestinationNodeLike): void;
  disconnect(): void;
}

export interface AudioWorkletNodeConstructor {
  new (
    context: AudioContextLike,
    name: string,
    options: { readonly processorOptions: { readonly frameSize: number } },
  ): AudioWorkletNodeLike;
}

export interface AudioDestinationNodeLike {
  readonly maxChannelCount: number;
}

export interface MessagePortLike {
  onmessage: ((event: { data: unknown }) => void) | null;
  postMessage(message: unknown, transfer?: Transferable[]): void;
}

export class CaptureError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = 'CaptureError';
    this.code = code;
  }
}

type CaptureState = 'idle' | 'starting' | 'running' | 'stopping' | 'error';

interface ResolvedOptions {
  readonly encoder: G711Kind;
  readonly sampleRate: number;
  readonly frameDurationMs: number;
  readonly maxBufferedFrames: number;
  readonly workletSourceUrl: string | undefined;
  readonly workletNodeCtor: AudioWorkletNodeConstructor | undefined;
  readonly audioContext: AudioContextLike | undefined;
  readonly getUserMedia:
    | ((constraints: MediaStreamConstraints) => Promise<MediaStream>)
    | undefined;
}

const DEFAULT_SAMPLE_RATE = 8000;
const DEFAULT_FRAME_DURATION_MS = 20;
const DEFAULT_MAX_BUFFERED_FRAMES = 4;

function getGlobalAudioContext(): AudioContextConstructor | undefined {
  if (typeof globalThis !== 'undefined' && 'AudioContext' in globalThis) {
    return (globalThis as unknown as { AudioContext: AudioContextConstructor }).AudioContext;
  }
  if (typeof globalThis !== 'undefined' && 'webkitAudioContext' in globalThis) {
    return (globalThis as unknown as { webkitAudioContext: AudioContextConstructor }).webkitAudioContext;
  }
  return undefined;
}

function getGlobalGetUserMedia():
  | ((constraints: MediaStreamConstraints) => Promise<MediaStream>)
  | undefined {
  const nav = typeof globalThis !== 'undefined' ? (globalThis as unknown as { navigator?: Navigator }).navigator : undefined;
  if (nav && 'mediaDevices' in nav && nav.mediaDevices) {
    const md = nav.mediaDevices as {
      getUserMedia?: (constraints: MediaStreamConstraints) => Promise<MediaStream>;
    };
    return md.getUserMedia?.bind(md);
  }
  return undefined;
}

function getGlobalWorkletNode(): AudioWorkletNodeConstructor | undefined {
  if (typeof globalThis !== 'undefined' && 'AudioWorkletNode' in globalThis) {
    return (globalThis as unknown as { AudioWorkletNode: AudioWorkletNodeConstructor }).AudioWorkletNode;
  }
  return undefined;
}

export class MicrophoneCapture {
  private state: CaptureState = 'idle';
  private options: ResolvedOptions;
  private callbacks: MicrophoneCaptureCallbacks;
  private resampler: AudioResampler | undefined;
  private pending = new Float32Array(0);
  private targetFrameSize = 0;
  private audioContext: AudioContextLike | undefined;
  private workletNode: AudioWorkletNodeLike | undefined;
  private sourceNode: MediaStreamAudioSourceNodeLike | undefined;
  private stream: MediaStream | undefined;
  private droppedFrames = 0;
  private startPromise: Promise<void> | undefined;

  constructor(options: MicrophoneCaptureOptions = {}, callbacks: MicrophoneCaptureCallbacks = {}) {
    this.options = {
      encoder: options.encoder ?? 'mulaw',
      sampleRate: options.sampleRate ?? DEFAULT_SAMPLE_RATE,
      frameDurationMs: options.frameDurationMs ?? DEFAULT_FRAME_DURATION_MS,
      maxBufferedFrames: options.maxBufferedFrames ?? DEFAULT_MAX_BUFFERED_FRAMES,
      workletSourceUrl: options.workletSourceUrl,
      workletNodeCtor: options.workletNodeCtor,
      audioContext: options.audioContext,
      getUserMedia: options.getUserMedia,
    };
    this.callbacks = callbacks;
    this.targetFrameSize = Math.round(
      (this.options.sampleRate * this.options.frameDurationMs) / 1000,
    );
  }

  private getState(): CaptureState {
    return this.state;
  }

  get isRunning(): boolean {
    return this.state === 'running';
  }

  getMetrics(): { readonly bufferedFrames: number; readonly droppedFrames: number } {
    return {
      bufferedFrames: Math.floor(this.pending.length / Math.max(1, this.targetFrameSize)),
      droppedFrames: this.droppedFrames,
    };
  }

  async start(): Promise<void> {
    if (this.state !== 'idle') {
      throw new CaptureError('bad-state', `Cannot start from state ${this.state}`);
    }
    this.state = 'starting';
    this.startPromise = this.doStart();
    await this.startPromise;
  }

  private async doStart(): Promise<void> {
    try {
      const getUserMedia = this.options.getUserMedia ?? getGlobalGetUserMedia();
      if (!getUserMedia) {
        throw new CaptureError('not-supported', 'getUserMedia is not available');
      }

      let stream: MediaStream;
      try {
        stream = await getUserMedia({ audio: true });
      } catch (err) {
        const code = this.classifyMediaError(err);
        throw new CaptureError(code, `getUserMedia failed: ${String(err)}`);
      }
      this.stream = stream;

      let audioContext: AudioContextLike;
      if (this.options.audioContext) {
        audioContext = this.options.audioContext;
      } else {
        const AudioContextCtor = getGlobalAudioContext();
        if (!AudioContextCtor) {
          throw new CaptureError('not-supported', 'AudioContext is not available');
        }
        audioContext = new AudioContextCtor({ sampleRate: this.options.sampleRate });
      }
      this.audioContext = audioContext;

      if (this.audioContext.state === 'suspended') {
        await this.audioContext.resume();
      }

      this.resampler = new AudioResampler({
        inputSampleRate: this.audioContext.sampleRate,
        outputSampleRate: this.options.sampleRate,
        channels: 1,
      });

      const workletUrl =
        this.options.workletSourceUrl ?? buildWorkletBlobUrl(getCaptureProcessorSource());
      await this.audioContext.audioWorklet.addModule(workletUrl);

      const contextFrameSize = Math.round(
        (this.audioContext.sampleRate * this.options.frameDurationMs) / 1000,
      );
      const WorkletNodeCtor = this.options.workletNodeCtor ?? getGlobalWorkletNode();
      if (!WorkletNodeCtor) {
        throw new CaptureError('not-supported', 'AudioWorkletNode is not available');
      }

      this.workletNode = new WorkletNodeCtor(this.audioContext, 'cheetah-capture-processor', {
        processorOptions: { frameSize: contextFrameSize },
      });
      this.workletNode.port.onmessage = (event: { data: unknown }) => this.handlePortMessage(event.data);

      this.sourceNode = this.audioContext.createMediaStreamSource(stream);
      this.sourceNode.connect(this.workletNode);

      this.state = 'running';
    } catch (err) {
      await this.cleanup(false);
      const captureError = err instanceof CaptureError ? err : new CaptureError('capture-failed', String(err));
      this.state = 'error';
      this.callbacks.onError?.(captureError);
      throw captureError;
    } finally {
      this.startPromise = undefined;
    }
  }

  async stop(): Promise<void> {
    if (this.state === 'idle' || this.state === 'stopping') {
      return;
    }
    if (this.state === 'starting') {
      const promise = this.startPromise;
      if (promise) {
        try {
          await promise;
        } catch {
          // Start failed; doStart's catch block has already released resources.
        }
      }
      if (this.getState() === 'error') {
        this.state = 'idle';
        return;
      }
      if (this.getState() === 'running') {
        return this.stop();
      }
      return;
    }

    this.state = 'stopping';
    await this.cleanup(true);
    this.state = 'idle';
  }

  private async cleanup(flush: boolean): Promise<void> {
    const worklet = this.workletNode;
    this.workletNode = undefined;
    worklet?.disconnect();
    if (worklet) {
      worklet.port.onmessage = null;
    }

    const source = this.sourceNode;
    this.sourceNode = undefined;
    source?.disconnect();

    const stream = this.stream;
    this.stream = undefined;
    if (stream) {
      for (const track of stream.getAudioTracks()) {
        track.stop();
      }
    }

    if (flush && this.resampler) {
      const flushed = this.resampler.flush();
      const out = flushed[0];
      if (out) {
        this.appendResampled(out);
      }
      this.emitFullFrames();
    }

    const ctx = this.audioContext;
    this.audioContext = undefined;
    if (ctx && !this.options.audioContext) {
      try {
        await ctx.close();
      } catch {
        // close() may fail if the context was already closed; ignore.
      }
    }

    this.resampler = undefined;
    this.pending = new Float32Array(0);
  }

  private classifyMediaError(err: unknown): string {
    const e = err as { name?: string; message?: string };
    const name = e.name ?? '';
    if (name === 'NotAllowedError') return 'permission-denied';
    if (name === 'NotFoundError' || name === 'DevicesNotFoundError') return 'no-device';
    if (name === 'NotReadableError' || name === 'AbortError') return 'device-busy';
    if (name === 'OverconstrainedError') return 'not-supported';
    return 'capture-failed';
  }

  private handlePortMessage(data: unknown): void {
    if (!data || typeof data !== 'object') return;
    const msg = data as { type?: string; samples?: Float32Array };
    if (msg.type === 'frame' && msg.samples instanceof Float32Array) {
      this.onWorkletFrame(msg.samples);
    }
  }

  private onWorkletFrame(samples: Float32Array): void {
    if (!this.resampler) return;
    const resampled = this.resampler.push([samples]);
    const out = resampled[0];
    if (!out) return;

    const maxFrames = Math.max(1, this.options.maxBufferedFrames);
    const maxSamples = maxFrames * this.targetFrameSize;
    if (this.pending.length + out.length > maxSamples) {
      // Drop the newest frame when the buffer is full to avoid unbounded growth.
      this.droppedFrames += 1;
      return;
    }

    this.appendResampled(out);
    this.emitFullFrames();
  }

  private appendResampled(out: Float32Array): void {
    const combined = new Float32Array(this.pending.length + out.length);
    combined.set(this.pending);
    combined.set(out, this.pending.length);
    this.pending = combined;
  }

  private emitFullFrames(): void {
    while (this.pending.length >= this.targetFrameSize) {
      const frame = this.pending.subarray(0, this.targetFrameSize);
      const payload = new Uint8Array(this.targetFrameSize);
      encodeG711F32(this.options.encoder, frame, payload);

      const packet: AudioPacket = {
        kind: this.options.encoder,
        payload,
        timestampMs: performance.now(),
        sampleRate: this.options.sampleRate,
        channels: 1,
      };
      this.callbacks.onPacket?.(packet);

      if (this.pending.length === this.targetFrameSize) {
        this.pending = new Float32Array(0);
      } else {
        const remaining = new Float32Array(this.pending.length - this.targetFrameSize);
        remaining.set(this.pending.subarray(this.targetFrameSize));
        this.pending = remaining;
      }
    }
  }
}
