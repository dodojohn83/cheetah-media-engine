/**
 * Audio pipeline: converts decoded audio frames, resamples if necessary, and
 * feeds an AudioWorklet ring (SharedArrayBuffer in isolated mode, transferable
 * blocks otherwise).  Keeps a bounded queue, handles underrun/overrun and
 * performs A/V drift correction.
 */

import { extractPlanarF32, type AudioFrame } from './format';
import { AudioResampler, type ResamplerOptions } from './resampler';
import {
  SharedAudioRingBuffer,
  LocalAudioRingBuffer,
  type AudioRingBuffer,
  type AudioRingMetrics,
} from './ring';
import {
  buildWorkletBlobUrl,
  getSharedRingProcessorSource,
  getTransferRingProcessorSource,
} from './worklet';

export interface AudioPipelineCallbacks {
  readonly onError?: (error: Error) => void;
  readonly onUnderrun?: (count: number) => void;
  readonly onOverrun?: (count: number) => void;
  readonly onDrift?: (driftMs: number) => void;
}

export interface AudioPipelineOptions {
  readonly audioContext?: AudioContextLike | undefined;
  readonly callbacks?: AudioPipelineCallbacks | undefined;
  readonly ringCapacityFrames?: number | undefined;
  readonly largeDriftMs?: number | undefined;
  readonly smallDriftMs?: number | undefined;
  readonly minRatio?: number | undefined;
  readonly maxRatio?: number | undefined;
  /** Optional pre-built worklet source URL (for tests). */
  readonly workletSourceUrl?: string | undefined;
  /** Optional AudioWorkletNode constructor (for tests). */
  readonly workletNodeCtor?: AudioWorkletNodeConstructor | undefined;
}

export interface AudioPipelineConfig {
  readonly inputSampleRate: number;
  readonly inputChannels: number;
  readonly outputSampleRate?: number | undefined;
  readonly outputChannels?: number | undefined;
}

export interface AudioPipelineMetrics {
  readonly framesPushed: number;
  readonly framesWritten: number;
  readonly framesDropped: number;
  readonly underrun: number;
  readonly overrun: number;
  readonly driftMs: number;
  readonly ratio: number;
  readonly ring: AudioRingMetrics | undefined;
}

export interface AudioContextLike {
  readonly sampleRate: number;
  readonly currentTime: number;
  readonly state: 'closed' | 'running' | 'suspended';
  resume(): Promise<void>;
  suspend(): Promise<void>;
  close(): Promise<void>;
  readonly audioWorklet: AudioWorkletLike;
  readonly destination: AudioDestinationNodeLike;
}

export interface AudioWorkletLike {
  addModule(moduleURL: string): Promise<void>;
}

export interface AudioDestinationNodeLike {
  readonly maxChannelCount: number;
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
    options: {
      processorOptions: Record<string, unknown>;
      outputChannelCount?: number[];
    },
  ): AudioWorkletNodeLike;
}

export interface MessagePortLike {
  onmessage: ((event: { data: unknown }) => void) | null;
  postMessage(message: unknown, transfer?: Transferable[]): void;
}

export class AudioPipelineError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = 'AudioPipelineError';
    this.code = code;
  }
}

const DEFAULT_LARGE_DRIFT_MS = 120;
const DEFAULT_SMALL_DRIFT_MS = 30;
const DEFAULT_RING_CAPACITY_FRAMES = 8192;

function isAudioContextLike(value: unknown): value is AudioContextLike {
  if (!value || typeof value !== 'object') return false;
  const c = value as Record<string, unknown>;
  return (
    typeof c.sampleRate === 'number' &&
    Number.isFinite(c.sampleRate) &&
    c.sampleRate > 0 &&
    typeof c.currentTime === 'number' &&
    Number.isFinite(c.currentTime) &&
    c.currentTime >= 0 &&
    (c.state === 'closed' || c.state === 'running' || c.state === 'suspended') &&
    typeof c.resume === 'function' &&
    typeof c.suspend === 'function' &&
    typeof c.close === 'function' &&
    typeof c.audioWorklet === 'object' &&
    c.audioWorklet !== null &&
    typeof (c.audioWorklet as { addModule?: unknown }).addModule === 'function' &&
    typeof c.destination === 'object' &&
    c.destination !== null &&
    typeof (c.destination as { maxChannelCount?: unknown }).maxChannelCount === 'number' &&
    Number.isFinite((c.destination as { maxChannelCount: number }).maxChannelCount) &&
    (c.destination as { maxChannelCount: number }).maxChannelCount > 0
  );
}

function validateAudioPipelineCallbacks(callbacks: unknown): void {
  if (!callbacks || typeof callbacks !== 'object') {
    throw new AudioPipelineError('bad-config', 'callbacks must be an object');
  }
  const c = callbacks as Partial<AudioPipelineCallbacks>;
  if (c.onError !== undefined && typeof c.onError !== 'function') {
    throw new AudioPipelineError('bad-config', 'callbacks.onError must be a function');
  }
  if (c.onUnderrun !== undefined && typeof c.onUnderrun !== 'function') {
    throw new AudioPipelineError('bad-config', 'callbacks.onUnderrun must be a function');
  }
  if (c.onOverrun !== undefined && typeof c.onOverrun !== 'function') {
    throw new AudioPipelineError('bad-config', 'callbacks.onOverrun must be a function');
  }
  if (c.onDrift !== undefined && typeof c.onDrift !== 'function') {
    throw new AudioPipelineError('bad-config', 'callbacks.onDrift must be a function');
  }
}

export class AudioPipeline {
  private audioContext: AudioContextLike;
  private callbacks: AudioPipelineCallbacks;
  private ringCapacity: number;
  private largeDriftMs: number;
  private smallDriftMs: number;
  private minRatio: number;
  private maxRatio: number;
  private workletSourceUrl: string | undefined;
  private workletNodeCtor: AudioWorkletNodeConstructor | undefined;

  private configured = false;
  private inputSampleRate = 0;
  private inputChannels = 0;
  private outputSampleRate = 0;
  private outputChannels = 0;
  private resampler: AudioResampler | undefined;
  private ring: AudioRingBuffer | undefined;
  private workletNode: AudioWorkletNodeLike | undefined;
  private transferPort: MessagePortLike | undefined;

  private baseMediaTimeMs = 0;
  private baseAudioContextTimeS: number | undefined = undefined;
  private generation = 0;

  private framesPushed = 0;
  private framesWritten = 0;
  private framesDropped = 0;
  private underrun = 0;
  private overrun = 0;
  private lastDriftMs = 0;

  constructor(options: AudioPipelineOptions = {}) {
    const audioContext = options.audioContext ?? createDefaultAudioContext();
    if (!isAudioContextLike(audioContext)) {
      throw new AudioPipelineError('bad-config', 'audioContext must be an AudioContext-like object');
    }
    this.audioContext = audioContext;
    if (options.callbacks !== undefined) {
      validateAudioPipelineCallbacks(options.callbacks);
    }
    this.callbacks = options.callbacks ?? {};
    if (options.workletSourceUrl !== undefined && typeof options.workletSourceUrl !== 'string') {
      throw new AudioPipelineError('bad-config', 'workletSourceUrl must be a string');
    }
    if (options.workletNodeCtor !== undefined && typeof options.workletNodeCtor !== 'function') {
      throw new AudioPipelineError('bad-config', 'workletNodeCtor must be a function');
    }
    this.ringCapacity = options.ringCapacityFrames ?? DEFAULT_RING_CAPACITY_FRAMES;
    this.largeDriftMs = options.largeDriftMs ?? DEFAULT_LARGE_DRIFT_MS;
    this.smallDriftMs = options.smallDriftMs ?? DEFAULT_SMALL_DRIFT_MS;
    this.minRatio = options.minRatio ?? 0.95;
    this.maxRatio = options.maxRatio ?? 1.05;

    if (!Number.isFinite(this.ringCapacity) || this.ringCapacity < 1 || !Number.isInteger(this.ringCapacity)) {
      throw new AudioPipelineError('bad-config', 'ringCapacityFrames must be a finite positive integer');
    }
    if (!Number.isFinite(this.smallDriftMs) || this.smallDriftMs <= 0) {
      throw new AudioPipelineError('bad-config', 'smallDriftMs must be a finite positive number');
    }
    if (!Number.isFinite(this.largeDriftMs) || this.largeDriftMs <= this.smallDriftMs) {
      throw new AudioPipelineError('bad-config', 'largeDriftMs must be a finite number greater than smallDriftMs');
    }
    if (!Number.isFinite(this.minRatio) || this.minRatio <= 0) {
      throw new AudioPipelineError('bad-config', 'minRatio must be a finite positive number');
    }
    if (!Number.isFinite(this.maxRatio) || this.maxRatio <= this.minRatio) {
      throw new AudioPipelineError('bad-config', 'maxRatio must be a finite number greater than minRatio');
    }

    this.workletSourceUrl = options.workletSourceUrl;
    this.workletNodeCtor = options.workletNodeCtor;
  }

  async configure(config: AudioPipelineConfig): Promise<void> {
    if (!Number.isFinite(config.inputSampleRate) || config.inputSampleRate <= 0 || !Number.isInteger(config.inputSampleRate)) {
      throw new AudioPipelineError('bad-config', 'inputSampleRate must be a finite positive integer');
    }
    if (!Number.isFinite(config.inputChannels) || config.inputChannels <= 0 || !Number.isInteger(config.inputChannels)) {
      throw new AudioPipelineError('bad-config', 'inputChannels must be a finite positive integer');
    }

    this.inputSampleRate = config.inputSampleRate;
    this.inputChannels = config.inputChannels;

    const outputSampleRate = config.outputSampleRate ?? this.audioContext.sampleRate;
    if (!Number.isFinite(outputSampleRate) || outputSampleRate <= 0 || !Number.isInteger(outputSampleRate)) {
      throw new AudioPipelineError('bad-config', 'outputSampleRate must be a finite positive integer');
    }
    const outputChannels = config.outputChannels ?? this.inputChannels;
    if (!Number.isFinite(outputChannels) || outputChannels <= 0 || !Number.isInteger(outputChannels)) {
      throw new AudioPipelineError('bad-config', 'outputChannels must be a finite positive integer');
    }

    this.outputSampleRate = outputSampleRate;
    this.outputChannels = outputChannels;

    const resamplerOptions: ResamplerOptions = {
      inputSampleRate: this.inputSampleRate,
      outputSampleRate: this.outputSampleRate,
      channels: this.outputChannels,
      minRatio: this.minRatio,
      maxRatio: this.maxRatio,
    };
    this.resampler = new AudioResampler(resamplerOptions);

    this.ring = this.createRingBuffer();
    this.configured = true;
    this.generation += 1;
  }

  async start(): Promise<void> {
    if (!this.configured) {
      throw new AudioPipelineError('not-configured', 'call configure() before start()');
    }
    await this.audioContext.resume();
    await this.attachWorklet();
  }

  async suspend(): Promise<void> {
    await this.audioContext.suspend();
  }

  async resume(): Promise<void> {
    await this.audioContext.resume();
  }

  async close(): Promise<void> {
    this.configured = false;
    if (this.workletNode) {
      this.workletNode.disconnect();
      this.workletNode = undefined;
    }
    await this.audioContext.close();
  }

  /** Push one decoded audio frame into the pipeline. */
  push(frame: AudioFrame): void {
    if (!this.configured || !this.resampler || !this.ring) {
      this.emitError(new AudioPipelineError('not-configured', 'pipeline not configured'));
      return;
    }

    if (
      !frame ||
      !Number.isFinite(frame.timestamp) ||
      !Number.isFinite(frame.sampleRate) ||
      frame.sampleRate <= 0 ||
      !Number.isFinite(frame.channels) ||
      frame.channels <= 0 ||
      frame.channels % 1 !== 0 ||
      !Number.isFinite(frame.numberOfFrames) ||
      frame.numberOfFrames <= 0 ||
      frame.numberOfFrames % 1 !== 0
    ) {
      this.emitError(new AudioPipelineError('bad-frame', 'AudioFrame has invalid timestamp, sampleRate, channels or numberOfFrames'));
      return;
    }

    const resampler = this.resampler;
    this.framesPushed += frame.numberOfFrames;

    try {
      const planar = extractPlanarF32(frame, this.outputChannels);
      const resampled = resampler.push(planar);

      // Always write resampler output; if it buffered the input for later
      // interpolation it must not be duplicated by writing the raw input.
      const toWrite = resampled;

      if (this.transferPort && !this.isIsolated()) {
        this.writeTransferable(toWrite);
      } else {
        this.writeRing(toWrite);
      }

      this.updateSync(frame.timestamp);
    } catch (err) {
      this.emitError(err instanceof Error ? err : new Error(String(err)));
    }
  }

  /** Reset the ring and resampler after a discontinuity. */
  reset(): void {
    this.resampler?.reset();
    this.ring?.reset();
    this.transferPort?.postMessage({ type: 'reset', generation: this.generation });
    this.generation += 1;
  }

  getMetrics(): AudioPipelineMetrics {
    return {
      framesPushed: this.framesPushed,
      framesWritten: this.framesWritten,
      framesDropped: this.framesDropped,
      underrun: this.underrun,
      overrun: this.overrun,
      driftMs: this.lastDriftMs,
      ratio: this.resampler?.currentRatio ?? 1,
      ring: this.ring?.getMetrics(),
    };
  }

  private isIsolated(): boolean {
    const crossOriginIsolated = (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated;
    return typeof SharedArrayBuffer !== 'undefined' && crossOriginIsolated === true;
  }

  private createRingBuffer(): AudioRingBuffer {
    const channels = this.outputChannels;
    if (this.isIsolated()) {
      const sab = new SharedArrayBuffer(32 + this.ringCapacity * channels * 4);
      return new SharedAudioRingBuffer(sab, this.ringCapacity, channels);
    }
    return new LocalAudioRingBuffer(this.ringCapacity, channels);
  }

  private async attachWorklet(): Promise<void> {
    const AudioWorkletNode = this.workletNodeCtor ?? getAudioWorkletNodeConstructor();
    if (this.isIsolated()) {
      const generatedUrl = this.workletSourceUrl ? undefined : buildWorkletBlobUrl(getSharedRingProcessorSource());
      const url = this.workletSourceUrl ?? generatedUrl;
      if (!url) {
        throw new AudioPipelineError('no-worklet-source', 'No AudioWorklet source URL available');
      }
      try {
        await this.audioContext.audioWorklet.addModule(url);
      } catch (err) {
        if (generatedUrl) {
          try { URL.revokeObjectURL(generatedUrl); } catch { /* ignore */ }
        }
        this.emitError(err instanceof Error ? err : new Error(String(err)));
        return;
      }
      if (generatedUrl) {
        try { URL.revokeObjectURL(generatedUrl); } catch { /* ignore */ }
      }
      this.workletNode = new AudioWorkletNode(this.audioContext, 'cheetah-audio-processor', {
        processorOptions: {
          sab: this.getSharedArrayBuffer(),
          capacity: this.ringCapacity,
          channels: this.outputChannels,
        },
        outputChannelCount: [this.outputChannels],
      });
      this.workletNode.connect(this.audioContext.destination);
    } else {
      const generatedUrl = this.workletSourceUrl ? undefined : buildWorkletBlobUrl(getTransferRingProcessorSource());
      const url = this.workletSourceUrl ?? generatedUrl;
      if (!url) {
        throw new AudioPipelineError('no-worklet-source', 'No AudioWorklet source URL available');
      }
      try {
        await this.audioContext.audioWorklet.addModule(url);
      } finally {
        if (generatedUrl) {
          try { URL.revokeObjectURL(generatedUrl); } catch { /* ignore */ }
        }
      }
      const channel = new MessageChannel();
      this.transferPort = channel.port1 as MessagePortLike;
      this.workletNode = new AudioWorkletNode(this.audioContext, 'cheetah-audio-transfer-processor', {
        processorOptions: {
          port: channel.port2,
          capacity: this.ringCapacity,
          channels: this.outputChannels,
        },
        outputChannelCount: [this.outputChannels],
      });
      this.workletNode.connect(this.audioContext.destination);
      this.transferPort.onmessage = (event: { data: unknown }) => {
        const msg = event.data as { type: string; count?: number };
        if (msg.type === 'underrun' && msg.count) {
          this.underrun += msg.count;
          this.callbacks.onUnderrun?.(msg.count);
        } else if (msg.type === 'overrun' && msg.count) {
          this.overrun += msg.count;
          this.callbacks.onOverrun?.(msg.count);
        }
      };
    }
  }

  private getSharedArrayBuffer(): SharedArrayBuffer | undefined {
    if (this.ring instanceof SharedAudioRingBuffer) {
      return this.ring.getBuffer();
    }
    return undefined;
  }

  private writeRing(frames: readonly Float32Array[]): void {
    const ring = this.ring;
    if (!ring) return;
    const frameCount = frames.at(0)?.length ?? 0;
    const written = ring.write(frames);
    this.framesWritten += written;
    if (written < frameCount) {
      const dropped = frameCount - written;
      this.framesDropped += dropped;
      this.overrun += dropped;
      ring.reportOverrun(dropped);
      this.callbacks.onOverrun?.(dropped);
    }
  }

  private writeTransferable(frames: readonly Float32Array[]): void {
    if (!this.transferPort) return;
    const frameCount = frames.at(0)?.length ?? 0;
    const channels = this.outputChannels;
    const interleaved = new Float32Array(frameCount * channels);
    for (let c = 0; c < channels; c += 1) {
      const src = frames.at(c) ?? frames.at(0) ?? new Float32Array(0);
      for (let i = 0; i < frameCount; i += 1) {
        interleaved[i * channels + c] = src.at(i) ?? 0;
      }
    }
    this.framesWritten += frameCount;
    this.transferPort.postMessage(
      { frames: frameCount, channels, buffer: interleaved.buffer },
      [interleaved.buffer],
    );
  }

  private updateSync(frameTimestampMs: number): void {
    if (this.baseAudioContextTimeS === undefined) {
      this.rebaseSync(frameTimestampMs);
      return;
    }

    const contextTimeMs = this.audioContext.currentTime * 1000;
    const expectedAudioMs =
      this.baseMediaTimeMs + (contextTimeMs - this.baseAudioContextTimeS * 1000);
    const drift = frameTimestampMs - expectedAudioMs;
    this.lastDriftMs = drift;
    this.callbacks.onDrift?.(drift);

    const absDrift = Math.abs(drift);
    if (absDrift > this.largeDriftMs) {
      this.rebaseSync(frameTimestampMs);
      this.reset();
      return;
    }

    if (absDrift > this.smallDriftMs && this.resampler) {
      // Pull resampler ratio slightly toward correcting the drift.
      const correction = -Math.sign(drift) * 0.001;
      const target = this.resampler.currentRatio * (1 + correction);
      this.resampler.setRatio(target);
    }
  }

  private rebaseSync(mediaTimeMs: number): void {
    this.baseMediaTimeMs = mediaTimeMs;
    this.baseAudioContextTimeS = this.audioContext.currentTime;
  }

  private emitError(error: Error): void {
    this.callbacks.onError?.(error);
  }
}

function createDefaultAudioContext(): AudioContextLike {
  if (typeof globalThis === 'undefined' || !('AudioContext' in globalThis)) {
    throw new AudioPipelineError('no-audio-context', 'AudioContext not available');
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return new (globalThis as unknown as { AudioContext: new () => AudioContextLike }).AudioContext();
}

function getAudioWorkletNodeConstructor(): AudioWorkletNodeConstructor {
  if (typeof globalThis === 'undefined' || !('AudioWorkletNode' in globalThis)) {
    throw new AudioPipelineError('no-audio-worklet', 'AudioWorkletNode not available');
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (globalThis as unknown as { AudioWorkletNode: AudioWorkletNodeConstructor }).AudioWorkletNode;
}
