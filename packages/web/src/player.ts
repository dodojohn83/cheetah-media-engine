import {
  createRuntime,
  RUNTIME_VERSION,
  MetricRegistry,
  MicrophoneCapture,
  IntercomPacketizer,
  CaptureError,
  type EngineRuntime,
  type MetricsSnapshot,
  type WorkerErrorPayload,
  type AudioPacket,
} from '@cheetah-media/runtime';
import { createGb28181PtzCmd, type PtzCommand } from './ptz';
import type { IntercomPacket } from '@cheetah-media/runtime';

export interface MemoryDescriptor {
  readonly region: number;
  readonly offset: bigint;
  readonly length: number;
  readonly capacity: number;
  readonly generation: bigint;
  readonly flags: number;
}

export interface PacketDescriptor {
  readonly trackIndex: number;
  readonly payload: MemoryDescriptor;
  readonly sideData: MemoryDescriptor;
  readonly ptsMs: bigint;
  readonly dtsMs: bigint;
  readonly durationMs: bigint;
  readonly flags: number;
  readonly epoch: bigint;
}

export interface FrameDescriptor {
  readonly trackIndex: number;
  readonly payload: MemoryDescriptor;
  readonly planes: readonly MemoryDescriptor[];
  readonly sideData: MemoryDescriptor;
  readonly width: number;
  readonly height: number;
  readonly ptsMs: bigint;
  readonly durationMs: bigint;
  readonly flags: number;
  readonly epoch: bigint;
}

export const enum AbiFeatureFlags {
  Threads = 1 << 0,
  Simd = 1 << 1,
  SharedArrayBuffer = 1 << 2,
}

/** Player lifecycle states reported to the application. */
export type PlayerState =
  | 'idle'
  | 'loading'
  | 'preroll'
  | 'playing'
  | 'paused'
  | 'rebuffering'
  | 'stopping'
  | 'failed'
  | 'destroyed';

/** Public event types emitted by the player. */
export type CheetahPlayerEventType =
  | 'statechange'
  | 'tracks'
  | 'firstframe'
  | 'backendchange'
  | 'variantchange'
  | 'buffering'
  | 'stats'
  | 'warning'
  | 'error'
  | 'recording'
  | 'ptz'
  | 'metadata'
  | 'intercom';

/** A single metadata item extracted from a stream or injected by an external caller. */
export interface MetadataItem {
  readonly source: number;
  readonly key: number;
  readonly timestampMs?: number;
  readonly value: Uint8Array | string;
}

/** Details for a `metadata` player event. */
export interface MetadataEventDetails {
  readonly items: readonly MetadataItem[];
}

/** Base event emitted to application listeners. */
export interface CheetahPlayerEvent<T extends CheetahPlayerEventType = CheetahPlayerEventType> {
  readonly type: T;
  readonly playerId: string;
  readonly epoch: number;
  readonly sequence: number;
  readonly timestamp: number;
  readonly details?: Record<string, unknown> | undefined;
}

export type EventListener<T extends CheetahPlayerEventType = CheetahPlayerEventType> = (
  event: CheetahPlayerEvent<T>,
) => void;

/** Live edge / latency target. */
export interface LatencyConfig {
  /** Soft target in milliseconds; playback may speed up to drain. */
  readonly softMs?: number;
  /** Hard target in milliseconds; the controller may drop to a keyframe. */
  readonly hardMs?: number;
  /** Maximum playback rate used to catch up (e.g. 1.05). */
  readonly maxPlaybackRate?: number;
}

/** Transport and source preferences. */
export interface TransportConfig {
  /** Preferred protocol; the runtime may fall back. */
  readonly protocol?: 'auto' | 'http-flv' | 'ws-flv' | 'http-fmp4' | 'ws-fmp4' | 'hls' | 'll-hls';
  /** Whether to prefer low-latency variants when available. */
  readonly lowLatency?: boolean;
  /** Custom headers sent with each request. Redacted in diagnostics. */
  readonly headers?: Record<string, string>;
}

/** Backend isolation and capability preferences. */
export interface BackendConfig {
  /** Ordered backend preference. Unsupported entries are skipped. */
  readonly preference?: readonly ('webcodecs' | 'mse' | 'wasm-threads' | 'wasm-simd' | 'wasm-baseline' | 'auto')[];
  /** Whether software decoding is allowed when hardware is unavailable. */
  readonly allowSoftware?: boolean;
  /** Whether MSE is allowed. */
  readonly allowMse?: boolean;
  /** Whether WebCodecs is allowed. */
  readonly allowWebCodecs?: boolean;
}

/** Memory and thread budgets. */
export interface MemoryConfig {
  readonly maxWasmMemoryMB?: number;
  readonly maxThreads?: number;
}

/** Renderer configuration. */
export interface RenderConfig {
  /** Ordered renderer preference. Unsupported entries are skipped. */
  readonly renderer?: readonly ('webgpu' | 'webgl2' | 'canvas2d' | 'auto')[];
  /** Maximum video resolution allowed for rendering. */
  readonly maxResolution?: { readonly width: number; readonly height: number };
  /** Default fit mode. */
  readonly fit?: 'contain' | 'cover' | 'fill' | 'none';
}

/** Audio output configuration. */
export interface AudioConfig {
  readonly enabled?: boolean;
  readonly preferNativeDecoder?: boolean;
  readonly volume?: number;
}

/** Recording options. */
export interface RecordingConfig {
  readonly mimeType?: string;
  readonly filename?: string;
}

/** Security / auth configuration. Redacted in diagnostics. */
export interface SecurityConfig {
  /** Custom credentials object; never serialized by toJSON or diagnostics. */
  readonly credentials?: unknown | undefined;
  /** Token or secret; redacted in diagnostics. */
  readonly token?: string | undefined;
}

/** Runtime bootstrap URLs. */
export interface RuntimeConfig {
  /** Base URL used to resolve default worker and wasm paths. */
  readonly assetBaseUrl?: string | undefined;
  readonly workerUrl?: string | undefined;
  readonly wasmUrl?: string | undefined;
}

/** Diagnostic and telemetry settings. */
export interface DiagnosticsConfig {
  /** Maximum number of recent events kept for exportDiagnostics. */
  readonly maxEventHistory?: number;
  /** Minimum milliseconds between `stats` event emissions. */
  readonly statsIntervalMs?: number;
  /** Whether to include full event history in diagnostics. */
  readonly includeEventHistory?: boolean;
}

/** Top-level player configuration. */
export interface PlayerConfig {
  readonly transport?: TransportConfig;
  readonly latency?: LatencyConfig;
  readonly backend?: BackendConfig;
  readonly memory?: MemoryConfig;
  readonly render?: RenderConfig;
  readonly audio?: AudioConfig;
  readonly recording?: RecordingConfig;
  readonly security?: SecurityConfig;
  readonly diagnostics?: DiagnosticsConfig;
  readonly runtime?: RuntimeConfig;
}

/** Contract for all public player instances. */
export interface IntercomOptions {
  /** Audio codec for the outgoing packet stream. */
  readonly codec?: 'g711a' | 'g711u' | 'opus';
  /** Optional payload type override for the RTP-like header. */
  readonly payloadType?: number;
  /** Called for each produced intercom packet. The transport layer is supplied by the caller. */
  readonly sendPacket: (packet: IntercomPacket) => void;
  /** Optional callback for capture errors. */
  readonly onError?: (error: Error) => void;
}

export interface CheetahPlayer {
  readonly id: string;
  readonly version: string;
  readonly state: PlayerState;
  readonly intercomActive: boolean;

  load(url: string, options?: { isLive?: boolean }): Promise<void>;
  play(): void;
  pause(): void;
  frameStep(direction: 'forward' | 'backward', keyframeOnly?: boolean): Promise<void>;
  pauseDisplay(keepConnection?: boolean): Promise<void>;
  ptz(command: import('./ptz').PtzCommand): Promise<void>;
  seek(timeMs: number): Promise<void>;
  setPlaybackRate(rate: number): Promise<void>;
  stop(): Promise<void>;
  destroy(): Promise<void>;

  snapshot(options?: { maxWidth?: number; maxHeight?: number }): Promise<ImageData>;
  startRecording(options?: { mimeType?: string; filename?: string }): Promise<void>;
  stopRecording(): Promise<void>;
  switchVariant(variant: { bandwidth?: number; index?: number }): Promise<void>;

  startIntercom(options: IntercomOptions): Promise<void>;
  stopIntercom(): Promise<void>;

  getStats(): PlayerStats;
  getMetrics(): MetricsSnapshot;
  exportDiagnostics(): DiagnosticsSnapshot;

  addEventListener<T extends CheetahPlayerEventType>(
    type: T,
    listener: EventListener<T>,
  ): void;
  removeEventListener<T extends CheetahPlayerEventType>(
    type: T,
    listener: EventListener<T>,
  ): void;
}

/** Snapshot returned by getStats. */
export interface PlayerStats {
  readonly state: PlayerState;
  readonly epoch: number;
  readonly bufferedMs: number | undefined;
  readonly decodedFrames: number | undefined;
  readonly droppedFrames: number | undefined;
  readonly networkBytes: number | undefined;
  readonly latencyMs: number | undefined;
  readonly timestamp: number;
}

/** Diagnostic export; no sensitive fields are exposed unredacted. */
export interface DiagnosticsSnapshot {
  readonly playerId: string;
  readonly version: string;
  readonly state: PlayerState;
  readonly epoch: number;
  readonly config: PlayerConfig;
  readonly lastStats: PlayerStats | undefined;
  readonly metrics?: MetricsSnapshot | undefined;
  readonly recentEvents: readonly CheetahPlayerEvent[];
}

/** Public error class with stable code/stage/recoverability. */
export class CheetahMediaError extends Error {
  readonly code: number;
  readonly stage: string;
  readonly recoverable: boolean;

  constructor(
    code: number,
    stage: string,
    message: string,
    options?: { readonly cause?: unknown; readonly recoverable?: boolean },
  ) {
    super(message);
    this.name = 'CheetahMediaError';
    this.code = code;
    this.stage = stage;
    this.recoverable = options?.recoverable ?? false;
    if (options?.cause && options.cause instanceof Error) {
      this.cause = options.cause;
    }
  }

  toJSON(): { readonly code: number; readonly stage: string; readonly recoverable: boolean; readonly message: string } {
    return {
      code: this.code,
      stage: this.stage,
      recoverable: this.recoverable,
      message: this.message,
    };
  }
}

const DEFAULT_LATENCY: Required<LatencyConfig> = {
  softMs: 5_000,
  hardMs: 15_000,
  maxPlaybackRate: 1.05,
};

const DEFAULT_TRANSPORT: Required<TransportConfig> = {
  protocol: 'auto',
  lowLatency: false,
  headers: {},
};

const DEFAULT_BACKEND: Required<BackendConfig> = {
  preference: ['auto'],
  allowSoftware: true,
  allowMse: true,
  allowWebCodecs: true,
};

const DEFAULT_MEMORY: Required<MemoryConfig> = {
  maxWasmMemoryMB: 256,
  maxThreads: 4,
};

const DEFAULT_RENDER: Required<RenderConfig> = {
  renderer: ['auto'],
  maxResolution: { width: 3840, height: 2160 },
  fit: 'contain',
};

const DEFAULT_AUDIO: Required<AudioConfig> = {
  enabled: true,
  preferNativeDecoder: true,
  volume: 1,
};

const DEFAULT_RECORDING: Required<RecordingConfig> = {
  mimeType: 'video/mp4',
  filename: 'recording.mp4',
};

const DEFAULT_SECURITY: Required<SecurityConfig> = {
  credentials: undefined,
  token: undefined,
};

const DEFAULT_RUNTIME: Required<RuntimeConfig> = {
  assetBaseUrl: undefined,
  workerUrl: undefined,
  wasmUrl: undefined,
};

const DEFAULT_DIAGNOSTICS: Required<DiagnosticsConfig> = {
  maxEventHistory: 500,
  statsIntervalMs: 250,
  includeEventHistory: true,
};

/** Internal fully-resolved configuration. */
interface FullPlayerConfig {
  readonly transport: Required<TransportConfig>;
  readonly latency: Required<LatencyConfig>;
  readonly backend: Required<BackendConfig>;
  readonly memory: Required<MemoryConfig>;
  readonly render: Required<RenderConfig>;
  readonly audio: Required<AudioConfig>;
  readonly recording: Required<RecordingConfig>;
  readonly security: Required<SecurityConfig>;
  readonly diagnostics: Required<DiagnosticsConfig>;
  readonly runtime: Required<RuntimeConfig>;
}

function resolveRuntimeUrls(runtime: Required<RuntimeConfig>): { workerUrl: string | undefined; wasmUrl: string | undefined } {
  const base =
    runtime.assetBaseUrl !== undefined ? runtime.assetBaseUrl.replace(/\/$/, '') : undefined;
  const workerUrl =
    runtime.workerUrl ?? (base !== undefined ? `${base}/worker.js` : undefined);
  const wasmUrl =
    runtime.wasmUrl ??
    (base !== undefined ? `${base}/wasm/cheetah_media_web_bindings.js` : undefined);
  return { workerUrl, wasmUrl };
}

function withDefaults(config: PlayerConfig = {}): FullPlayerConfig {
  return {
    transport: { ...DEFAULT_TRANSPORT, ...config.transport },
    latency: { ...DEFAULT_LATENCY, ...config.latency },
    backend: { ...DEFAULT_BACKEND, ...config.backend },
    memory: { ...DEFAULT_MEMORY, ...config.memory },
    render: { ...DEFAULT_RENDER, ...config.render },
    audio: { ...DEFAULT_AUDIO, ...config.audio },
    recording: { ...DEFAULT_RECORDING, ...config.recording },
    security: { ...DEFAULT_SECURITY, ...config.security },
    diagnostics: { ...DEFAULT_DIAGNOSTICS, ...config.diagnostics },
    runtime: { ...DEFAULT_RUNTIME, ...config.runtime },
  };
}

function validateConfig(config: FullPlayerConfig): void {
  if (config.latency.softMs < 0 || config.latency.hardMs < 0) {
    throw new CheetahMediaError(6001, 'config', 'Latency targets must be non-negative');
  }
  if (config.latency.hardMs <= config.latency.softMs) {
    throw new CheetahMediaError(6001, 'config', 'Latency hard target must be greater than soft target');
  }
  if (config.latency.maxPlaybackRate < 1 || config.latency.maxPlaybackRate > 2) {
    throw new CheetahMediaError(6001, 'config', 'maxPlaybackRate must be between 1 and 2');
  }
  if (config.memory.maxWasmMemoryMB < 16) {
    throw new CheetahMediaError(6001, 'config', 'maxWasmMemoryMB must be at least 16');
  }
  if (config.memory.maxThreads < 1) {
    throw new CheetahMediaError(6001, 'config', 'maxThreads must be at least 1');
  }
  if (config.diagnostics.maxEventHistory < 0) {
    throw new CheetahMediaError(6001, 'config', 'maxEventHistory must be non-negative');
  }
  if (config.diagnostics.statsIntervalMs < 16) {
    throw new CheetahMediaError(6001, 'config', 'statsIntervalMs must be at least 16');
  }
}

function mapState(raw: string): PlayerState {
  const normalized = raw.toLowerCase();
  switch (normalized) {
    case 'idle':
    case 'loading':
    case 'preroll':
    case 'playing':
    case 'paused':
    case 'rebuffering':
    case 'stopping':
    case 'failed':
    case 'destroyed':
      return normalized;
    default:
      return 'idle';
  }
}

function redactConfig(config: FullPlayerConfig): PlayerConfig {
  return {
    transport: {
      ...config.transport,
      headers: Object.freeze({}),
    },
    latency: config.latency,
    backend: config.backend,
    memory: config.memory,
    render: config.render,
    audio: config.audio,
    recording: config.recording,
    security: {
      credentials: config.security.credentials === undefined ? undefined : '<redacted>',
      token: config.security.token === undefined ? undefined : '<redacted>',
    },
    diagnostics: config.diagnostics,
    runtime: {
      assetBaseUrl: config.runtime.assetBaseUrl === undefined ? undefined : '<redacted>',
      workerUrl: config.runtime.workerUrl === undefined ? undefined : '<redacted>',
      wasmUrl: config.runtime.wasmUrl === undefined ? undefined : '<redacted>',
    },
  };
}

let playerCounter = 0;

export class CheetahPlayerImpl implements CheetahPlayer {
  readonly id: string;
  readonly version = RUNTIME_VERSION;
  private runtime: EngineRuntime;
  private config: FullPlayerConfig;
  private _state: PlayerState = 'idle';
  private destroyed = false;
  private sequence = 0;
  private lastStats: PlayerStats | undefined;
  private lastStatsTime = 0;
  private eventHistory: CheetahPlayerEvent[] = [];
  private listeners = new Map<CheetahPlayerEventType, Set<EventListener>>();
  private metricRegistry = new MetricRegistry();
  private _intercomActive = false;
  private _intercomCapture: MicrophoneCapture | undefined;
  private _intercomPacketizer: IntercomPacketizer | undefined;
  private _intercomSend: ((packet: IntercomPacket) => void) | undefined;
  private _intercomError: ((error: Error) => void) | undefined;

  constructor(
    config: PlayerConfig,
    runtimeFactory: (opts: { readonly workerUrl?: string | undefined; readonly wasmUrl?: string | undefined }) => EngineRuntime,
  ) {
    playerCounter += 1;
    this.id = `cheetah-${playerCounter}`;
    this.config = withDefaults(config);
    validateConfig(this.config);
    const resolved = resolveRuntimeUrls(this.config.runtime);
    this.runtime = runtimeFactory({
      workerUrl: resolved.workerUrl,
      wasmUrl: resolved.wasmUrl,
    });
    this.runtime.onEvent = (event, details) => this.handleRuntimeEvent(event, details);
    this.runtime.onError = (error) => this.handleRuntimeError(error);
  }

  get state(): PlayerState {
    return this._state;
  }

  get intercomActive(): boolean {
    return this._intercomActive;
  }

  private nextSequence(): number {
    this.sequence += 1;
    return this.sequence;
  }

  private now(): number {
    return typeof performance !== 'undefined' ? performance.now() : Date.now();
  }

  private setState(to: PlayerState): void {
    if (this._state === to) return;
    const from = this._state;
    this._state = to;
    this.emit('statechange', { from, to });
  }

  private emit<T extends CheetahPlayerEventType>(
    type: T,
    details?: Record<string, unknown>,
  ): void {
    if (this.destroyed && type !== 'error') return;

    const event: CheetahPlayerEvent<T> = {
      type,
      playerId: this.id,
      epoch: this.runtime.epoch,
      sequence: this.nextSequence(),
      timestamp: this.now(),
      details,
    };

    this.eventHistory.push(event as CheetahPlayerEvent);
    while (this.eventHistory.length > this.config.diagnostics.maxEventHistory) {
      this.eventHistory.shift();
    }

    if (type === 'stats') {
      this.updateStatsFromDetails(event.details, event.epoch, event.timestamp);
    }

    const set = this.listeners.get(type);
    if (!set) return;
    for (const listener of set) {
      try {
        listener(event as CheetahPlayerEvent);
      } catch {
        // User handler exceptions must not break the engine.
      }
    }
  }

  private updateStatsFromDetails(
    details: Record<string, unknown> | undefined,
    epoch: number,
    timestamp: number,
  ): void {
    const d = details ?? {};
    this.lastStats = {
      state: this._state,
      epoch,
      bufferedMs: typeof d.bufferedMs === 'number' ? d.bufferedMs : this.lastStats?.bufferedMs,
      decodedFrames: typeof d.decodedFrames === 'number' ? d.decodedFrames : this.lastStats?.decodedFrames,
      droppedFrames: typeof d.droppedFrames === 'number' ? d.droppedFrames : this.lastStats?.droppedFrames,
      networkBytes: typeof d.networkBytes === 'number' ? d.networkBytes : this.lastStats?.networkBytes,
      latencyMs: typeof d.latencyMs === 'number' ? d.latencyMs : this.lastStats?.latencyMs,
      timestamp,
    };

    if (typeof d.bufferedMs === 'number') this.metricRegistry.gauge('buffered-ms', 'timeline', 'ms').set(d.bufferedMs);
    if (typeof d.decodedFrames === 'number') this.metricRegistry.gauge('decoded-frames', 'decode', 'frames').set(d.decodedFrames);
    if (typeof d.droppedFrames === 'number') this.metricRegistry.gauge('dropped-frames', 'render', 'frames').set(d.droppedFrames);
    if (typeof d.networkBytes === 'number') this.metricRegistry.gauge('network-bytes', 'source', 'bytes').set(d.networkBytes);
    if (typeof d.latencyMs === 'number') this.metricRegistry.gauge('latency-ms', 'timeline', 'ms').set(d.latencyMs);
  }

  private handleRuntimeEvent(event: string, details?: Record<string, unknown>): void {
    const normalized = event.toLowerCase();

    if (normalized === 'statechanged' || normalized === 'statechange' || normalized === 'state') {
      const to = typeof details?.to === 'string' ? mapState(details.to) : this._state;
      this.setState(to);
      return;
    }

    // State changes emitted first: an error forces failed before the error event.
    if (normalized === 'error') {
      this.setState('failed');
      this.emit('error', details);
      return;
    }

    if (normalized === 'track' || normalized === 'trackadded' || normalized === 'tracks') {
      this.emit('tracks', details);
      return;
    }

    if (normalized === 'firstframe') {
      this.emit('firstframe', details);
      return;
    }

    if (normalized === 'backend' || normalized === 'backendchange') {
      this.emit('backendchange', details);
      return;
    }

    if (normalized === 'variant' || normalized === 'variantchange') {
      this.emit('variantchange', details);
      return;
    }

    if (normalized === 'buffering' || normalized === 'rebuffering') {
      this.setState('rebuffering');
      this.emit('buffering', details);
      return;
    }

    if (normalized === 'stats' || normalized === 'metrics') {
      const now = this.now();
      this.updateStatsFromDetails(details, this.runtime.epoch, now);
      if (now - this.lastStatsTime < this.config.diagnostics.statsIntervalMs) {
        return;
      }
      this.lastStatsTime = now;
      this.emit('stats', details);
      return;
    }

    if (normalized === 'warning' || normalized === 'resourcewarning') {
      this.emit('warning', details);
      return;
    }

    if (normalized === 'recording') {
      this.emit('recording', details);
      return;
    }

    if (normalized === 'metadata') {
      this.emit('metadata', details);
      return;
    }

    // Forward any unknown event with its original name in details.
    this.emit('warning', { unknownEvent: event, ...details });
  }

  private handleRuntimeError(error: WorkerErrorPayload): void {
    this.setState('failed');
    const err = new CheetahMediaError(
      error.code,
      error.stage,
      error.message,
      { recoverable: error.recoverable },
    );
    this.emit('error', { error: err.toJSON() });
  }

  private guardDestroyed(): void {
    if (this.destroyed) {
      throw new CheetahMediaError(6999, 'lifecycle', 'Player destroyed', { recoverable: false });
    }
  }

  async load(url: string, options: { isLive?: boolean } = {}): Promise<void> {
    this.guardDestroyed();
    this.setState('loading');
    try {
      await this.runtime.load(url, { isLive: options.isLive ?? false });
      if (this._state !== 'failed') {
        this.setState('preroll');
      }
    } catch (cause) {
      this.setState('failed');
      throw new CheetahMediaError(6100, 'load', 'Load failed', { cause, recoverable: true });
    }
  }

  play(): void {
    this.guardDestroyed();
    this.runtime.play();
    this.setState('playing');
  }

  pause(): void {
    this.guardDestroyed();
    this.runtime.pause();
    this.setState('paused');
  }

  async seek(timeMs: number): Promise<void> {
    this.guardDestroyed();
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Seek requires an active stream', { recoverable: true });
    }
    try {
      await this.runtime.seek(timeMs);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'seek', 'Seek failed', { cause, recoverable: true });
    }
  }

  async setPlaybackRate(rate: number): Promise<void> {
    this.guardDestroyed();
    try {
      await this.runtime.setPlaybackRate(rate);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'playback-rate', 'Set playback rate failed', { cause, recoverable: true });
    }
  }

  async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
    this.guardDestroyed();
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Frame step requires an active stream', { recoverable: true });
    }
    try {
      await this.runtime.frameStep(direction, keyframeOnly);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'frame-step', 'Frame step failed', { cause, recoverable: true });
    }
  }

  async pauseDisplay(keepConnection = true): Promise<void> {
    this.guardDestroyed();
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Pause display requires an active stream', { recoverable: true });
    }
    try {
      await this.runtime.pauseDisplay(keepConnection);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'pause-display', 'Pause display failed', { cause, recoverable: true });
    }
  }

  async ptz(command: PtzCommand): Promise<void> {
    this.guardDestroyed();
    try {
      const ptzCmd = createGb28181PtzCmd(command);
      this.emit('ptz', { ptzCmd, action: command.action, speeds: command.speeds, protocol: 'gb28181' });
    } catch (cause) {
      throw new CheetahMediaError(6999, 'ptz', cause instanceof Error ? cause.message : 'PTZ command failed', {
        cause,
        recoverable: true,
      });
    }
  }

  async stop(): Promise<void> {
    this.guardDestroyed();
    this.setState('stopping');
    try {
      await this.runtime.stop();
    } catch (cause) {
      this.setState('failed');
      throw new CheetahMediaError(6100, 'stop', 'Stop failed', { cause, recoverable: true });
    }
    this.setState('idle');
  }

  async destroy(): Promise<void> {
    if (this.destroyed) return;
    this.destroyed = true;
    this._intercomActive = false;
    await this.cleanupIntercom().catch(() => undefined);
    this._state = 'destroyed';
    this.listeners.clear();
    this.eventHistory.length = 0;
    this.metricRegistry.reset();
    await this.runtime.destroy();
  }

  async snapshot(options: { maxWidth?: number; maxHeight?: number } = {}): Promise<ImageData> {
    this.guardDestroyed();
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Snapshot requires an active stream', { recoverable: true });
    }
    try {
      const result = await this.runtime.request('snapshot', options, 5000);
      const data = result as { width: number; height: number; data?: Uint8ClampedArray } | undefined;
      if (!data || typeof data.width !== 'number' || typeof data.height !== 'number') {
        throw new CheetahMediaError(6999, 'snapshot', 'Invalid snapshot result', { recoverable: false });
      }
      const clamped = data.data
        ? new Uint8ClampedArray(data.data)
        : new Uint8ClampedArray(data.width * data.height * 4);
      return new ImageData(clamped, data.width, data.height);
    } catch (cause) {
      if (cause instanceof CheetahMediaError) throw cause;
      throw new CheetahMediaError(6999, 'snapshot', 'Snapshot failed', { cause, recoverable: false });
    }
  }

  async startRecording(options: { mimeType?: string; filename?: string } = {}): Promise<void> {
    this.guardDestroyed();
    if (this._state !== 'playing' && this._state !== 'paused') {
      throw new CheetahMediaError(6002, 'sdk', 'Recording requires an active stream', { recoverable: true });
    }
    try {
      await this.runtime.request('start-recording', options, 5000);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'recording', 'Recording failed', { cause, recoverable: false });
    }
  }

  async stopRecording(): Promise<void> {
    this.guardDestroyed();
    try {
      await this.runtime.request('stop-recording', undefined, 5000);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'recording', 'Stop recording failed', { cause, recoverable: false });
    }
  }

  async startIntercom(options: IntercomOptions): Promise<void> {
    this.guardDestroyed();
    if (this._intercomActive) {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom already active', { recoverable: true });
    }
    if (options.codec === 'opus') {
      throw new CheetahMediaError(6003, 'sdk', 'Opus intercom is not supported in this version', { recoverable: true });
    }

    const encoder = options.codec === 'g711a' ? 'alaw' : 'mulaw';
    const payloadType = options.payloadType ?? (encoder === 'alaw' ? 8 : 0);

    this._intercomSend = options.sendPacket;
    this._intercomError = options.onError;
    this._intercomPacketizer = new IntercomPacketizer({
      payloadType,
      onPacket: (packet) => this.handleIntercomPacket(packet),
    });

    this._intercomCapture = new MicrophoneCapture(
      { encoder },
      {
        onPacket: (packet: AudioPacket) => this._intercomPacketizer?.push(packet),
        onError: (error: CaptureError) => this.handleCaptureError(error),
      },
    );

    try {
      await this._intercomCapture.start();
      this._intercomActive = true;
      this.emit('intercom', { active: true, codec: encoder });
    } catch (cause) {
      this.cleanupIntercom();
      const message = cause instanceof Error ? cause.message : String(cause);
      throw new CheetahMediaError(6999, 'intercom', `Intercom start failed: ${message}`, { cause, recoverable: true });
    }
  }

  async stopIntercom(): Promise<void> {
    this.guardDestroyed();
    if (!this._intercomActive && !this._intercomCapture) {
      return;
    }
    await this.cleanupIntercom();
    this._intercomActive = false;
    this.emit('intercom', { active: false });
  }

  private async cleanupIntercom(): Promise<void> {
    const capture = this._intercomCapture;
    this._intercomCapture = undefined;
    this._intercomPacketizer = undefined;
    this._intercomSend = undefined;
    this._intercomError = undefined;
    if (capture) {
      try {
        await capture.stop();
      } catch {
        // stop() may fail if the capture was never running; ignore.
      }
    }
  }

  private handleIntercomPacket(packet: IntercomPacket): void {
    try {
      this._intercomSend?.(packet);
    } catch (error) {
      this.reportIntercomError(error instanceof Error ? error : new Error(String(error)), false);
    }
  }

  private handleCaptureError(error: CaptureError): void {
    this._intercomActive = false;
    this.reportIntercomError(error, false);
    void this.cleanupIntercom();
  }

  private reportIntercomError(error: Error, fatal: boolean): void {
    const err =
      error instanceof CheetahMediaError
        ? error
        : new CheetahMediaError(6999, 'intercom', error.message, { recoverable: !fatal });
    this.emit('intercom', { active: this._intercomActive, error: err.toJSON() });
    this._intercomError?.(error);
  }

  async switchVariant(variant: { bandwidth?: number; index?: number }): Promise<void> {
    this.guardDestroyed();
    if (variant.bandwidth === undefined && variant.index === undefined) {
      throw new CheetahMediaError(6002, 'sdk', 'Variant bandwidth or index required', { recoverable: true });
    }
    try {
      await this.runtime.request('switch-variant', variant, 10000);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'variant', 'Variant switch failed', { cause, recoverable: true });
    }
  }

  getStats(): PlayerStats {
    this.guardDestroyed();
    return (
      this.lastStats ?? {
        state: this._state,
        epoch: this.runtime.epoch,
        bufferedMs: undefined,
        decodedFrames: undefined,
        droppedFrames: undefined,
        networkBytes: undefined,
        latencyMs: undefined,
        timestamp: this.now(),
      }
    );
  }

  getMetrics(): MetricsSnapshot {
    this.guardDestroyed();
    return this.metricRegistry.snapshot();
  }

  exportDiagnostics(): DiagnosticsSnapshot {
    this.guardDestroyed();
    return {
      playerId: this.id,
      version: this.version,
      state: this._state,
      epoch: this.runtime.epoch,
      config: redactConfig(this.config),
      lastStats: this.lastStats,
      metrics: this.metricRegistry.snapshot(),
      recentEvents: this.config.diagnostics.includeEventHistory
        ? Object.freeze([...this.eventHistory])
        : Object.freeze([]),
    };
  }

  addEventListener<T extends CheetahPlayerEventType>(
    type: T,
    listener: EventListener<T>,
  ): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener as EventListener);
  }

  removeEventListener<T extends CheetahPlayerEventType>(
    type: T,
    listener: EventListener<T>,
  ): void {
    this.listeners.get(type)?.delete(listener as EventListener);
  }
}

/** Create a new player instance without starting network activity. */
export function createPlayer(config: PlayerConfig = {}): CheetahPlayer {
  return new CheetahPlayerImpl(config, (opts) => createRuntime(opts));
}
