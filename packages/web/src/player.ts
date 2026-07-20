import {
  createRuntime,
  RUNTIME_VERSION,
  MetricRegistry,
  MicrophoneCapture,
  IntercomPacketizer,
  CaptureError,
  BlobSink,
  StreamDownloader,
  CompositeRecorder,
  PlaybackSession,
  detectProtocol,
  protocolSupportedByMseSession,
  type EngineRuntime,
  type MetricsSnapshot,
  type WorkerErrorPayload,
  type AudioPacket,
  type DownloadResult,
  type DownloadProgress,
  type DownloadSink,
  type DownloadOptions as RuntimeDownloadOptions,
  type TransportError,
  type CompositeRecordingOptions as CompositeRecordingOptionsType,
  type CompositeRecordingResult as CompositeRecordingResultType,
  type HTMLVideoElementLike,
  type PlaybackSessionEvent,
  type Protocol,
} from '@cheetah-media/runtime';
import { createGb28181PtzCmd, type PtzCommand } from './ptz';
import { NoopVrRenderer, NoopAiFrameProcessor, type VrRenderer, type AiFrameProcessor } from './vr';
import type { IntercomPacket } from '@cheetah-media/runtime';

export type { DownloadResult, DownloadProgress, DownloadSink } from '@cheetah-media/runtime';
export type { CompositeRecordingOptions, CompositeRecordingResult, CompositeRecordingProgress, CompositeWatermark } from '@cheetah-media/runtime';

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
  | 'compositeRecording'
  | 'ptz'
  | 'metadata'
  | 'intercom'
  | 'download';

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

export interface DownloadOptions {
  /** URL to download. Only http(s) URLs are supported. */
  readonly url: string;
  /** Optional request headers. */
  readonly headers?: Record<string, string>;
  /** Fetch credentials mode. */
  readonly credentials?: RequestCredentials;
  /** Optional custom sink. Defaults to an in-memory BlobSink. */
  readonly sink?: DownloadSink;
  /** Optional per-chunk transform applied before writing to the sink. */
  readonly transform?: (chunk: Uint8Array) => Uint8Array | undefined;
  /** Optional filename hint when saving via File System Access API. */
  readonly filename?: string;
}

export interface CheetahPlayer {
  readonly id: string;
  readonly version: string;
  readonly state: PlayerState;
  readonly intercomActive: boolean;
  readonly vrActive: boolean;
  readonly aiActive: boolean;

  /**
   * Attach a video element used by the main-thread MSE playback session.
   * Without a media element, load/play only drive the WASM control shell
   * (useful for unit tests); real browser playback requires attach + load.
   */
  attachMediaElement(element: HTMLVideoElementLike | null): void;
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

  startDownload(options: DownloadOptions): Promise<import('@cheetah-media/runtime').DownloadResult>;
  pauseDownload(): void;
  resumeDownload(options?: DownloadOptions): Promise<import('@cheetah-media/runtime').DownloadResult>;
  stopDownload(): Promise<void>;
  readonly downloadActive: boolean;
  readonly downloadProgress: import('@cheetah-media/runtime').DownloadProgress | undefined;
  readonly downloadBlob: Blob | undefined;

  startCompositeRecording(options: CompositeRecordingOptionsType): Promise<void>;
  pauseCompositeRecording(): void;
  resumeCompositeRecording(): void;
  stopCompositeRecording(): Promise<CompositeRecordingResultType>;
  readonly compositeRecordingActive: boolean;

  setVrRenderer(renderer: VrRenderer): void;
  setAiProcessor(processor: AiFrameProcessor): void;

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

function errorMessageFromCause(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  if (cause && typeof cause === 'object' && 'message' in cause) {
    return String((cause as { message: unknown }).message);
  }
  return String(cause);
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
  if (!Number.isFinite(config.latency.softMs) || !Number.isFinite(config.latency.hardMs) || config.latency.softMs < 0 || config.latency.hardMs < 0) {
    throw new CheetahMediaError(6001, 'config', 'Latency targets must be finite and non-negative');
  }
  if (config.latency.hardMs <= config.latency.softMs) {
    throw new CheetahMediaError(6001, 'config', 'Latency hard target must be greater than soft target');
  }
  if (!Number.isFinite(config.latency.maxPlaybackRate) || config.latency.maxPlaybackRate < 1 || config.latency.maxPlaybackRate > 2) {
    throw new CheetahMediaError(6001, 'config', 'maxPlaybackRate must be a finite number between 1 and 2');
  }
  if (!Number.isFinite(config.memory.maxWasmMemoryMB) || config.memory.maxWasmMemoryMB < 16) {
    throw new CheetahMediaError(6001, 'config', 'maxWasmMemoryMB must be a finite number at least 16');
  }
  if (!Number.isFinite(config.memory.maxThreads) || config.memory.maxThreads < 1) {
    throw new CheetahMediaError(6001, 'config', 'maxThreads must be a finite positive integer');
  }
  if (!Number.isFinite(config.diagnostics.maxEventHistory) || config.diagnostics.maxEventHistory < 0) {
    throw new CheetahMediaError(6001, 'config', 'maxEventHistory must be a finite non-negative number');
  }
  if (!Number.isFinite(config.diagnostics.statsIntervalMs) || config.diagnostics.statsIntervalMs < 16) {
    throw new CheetahMediaError(6001, 'config', 'statsIntervalMs must be a finite number at least 16');
  }
  if (!Number.isFinite(config.audio.volume) || config.audio.volume < 0 || config.audio.volume > 1) {
    throw new CheetahMediaError(6001, 'config', 'volume must be a finite number between 0 and 1');
  }
  if (config.render.maxResolution !== undefined) {
    if (!Number.isFinite(config.render.maxResolution.width) || config.render.maxResolution.width <= 0 || !Number.isFinite(config.render.maxResolution.height) || config.render.maxResolution.height <= 0) {
      throw new CheetahMediaError(6001, 'config', 'maxResolution width and height must be finite positive numbers');
    }
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
  private _intercomStarting = false;
  private _intercomCapture: MicrophoneCapture | undefined;
  private _intercomPacketizer: IntercomPacketizer | undefined;
  private _intercomSend: ((packet: IntercomPacket) => void) | undefined;
  private _intercomError: ((error: Error) => void) | undefined;
  private _downloadController: StreamDownloader | undefined;
  private _lastDownloadOptions: DownloadOptions | undefined;
  private _downloadSink: DownloadSink | undefined;
  private _compositeRecorder: CompositeRecorder | undefined;
  private _lastCompositeOptions: CompositeRecordingOptionsType | undefined;
  private _vrRenderer: VrRenderer = new NoopVrRenderer();
  private _aiProcessor: AiFrameProcessor = new NoopAiFrameProcessor();
  private mediaElement: HTMLVideoElementLike | null = null;
  private session: PlaybackSession | undefined;
  private mediaRecorder: MediaRecorder | undefined;
  private mediaRecorderChunks: Blob[] = [];
  private mediaRecorderMime: string | undefined;
  private mediaRecorderFilename: string | undefined;

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

  attachMediaElement(element: HTMLVideoElementLike | null): void {
    this.guardDestroyed();
    if (element !== null && element !== undefined) {
      if (typeof element !== 'object') {
        throw new CheetahMediaError(6002, 'sdk', 'Media element must be an object or null', { recoverable: true });
      }
      const el = element as unknown as Record<string, unknown>;
      if (typeof el.addEventListener !== 'function' || typeof el.removeEventListener !== 'function') {
        throw new CheetahMediaError(6002, 'sdk', 'Media element must expose addEventListener and removeEventListener', { recoverable: true });
      }
    }
    this.mediaElement = element;
  }

  get state(): PlayerState {
    return this._state;
  }

  get intercomActive(): boolean {
    return this._intercomActive;
  }

  get downloadActive(): boolean {
    return this._downloadController ? this._downloadController.progress.state === 'running' : false;
  }

  get downloadProgress(): DownloadProgress | undefined {
    return this._downloadController?.progress;
  }

  get downloadBlob(): Blob | undefined {
    return this._downloadSink instanceof BlobSink ? this._downloadSink.getBlob() : undefined;
  }

  get compositeRecordingActive(): boolean {
    return this._compositeRecorder ? this._compositeRecorder.recordingActive : false;
  }

  get vrActive(): boolean {
    return this._vrRenderer.active;
  }

  get aiActive(): boolean {
    return this._aiProcessor.active;
  }

  private nextSequence(): number {
    this.sequence += 1;
    return this.sequence;
  }

  private now(): number {
    return typeof performance !== 'undefined' ? performance.now() : Date.now();
  }

  private failedStateTimer: ReturnType<typeof setTimeout> | undefined;
  private pendingFailed = false;

  private setState(to: PlayerState): void {
    if (this._state === to) return;
    // A pending failed transition from preroll should only be cancelled when the
    // lifecycle really moves on (new load/preroll/playing, stop, destroy). Other
    // transient states like rebuffering/paused must not silently drop the fatal error.
    const isTerminal = to === 'idle' || to === 'destroyed';
    const isFailed = to === 'failed';
    const cancelsPendingFailed = isTerminal || isFailed || to === 'loading' || to === 'preroll' || to === 'playing';
    if (isFailed && this._state === 'preroll' && this.pendingFailed) return;
    if (this.failedStateTimer && (!this.pendingFailed || cancelsPendingFailed)) {
      clearTimeout(this.failedStateTimer);
      this.failedStateTimer = undefined;
    }
    if (cancelsPendingFailed) {
      this.pendingFailed = false;
    }
    // Briefly hold the preroll state so tests and observers can see it before
    // an immediate post-preroll demux/network error flips to failed.
    if (isFailed && this._state === 'preroll') {
      this.pendingFailed = true;
      this.failedStateTimer = setTimeout(() => {
        this.failedStateTimer = undefined;
        this.pendingFailed = false;
        if (this._state === 'failed') return;
        const from = this._state;
        this._state = 'failed';
        this.emit('statechange', { from, to: 'failed' });
      }, 1500);
      return;
    }
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
    if (typeof url !== 'string' || url.length === 0) {
      throw new CheetahMediaError(7001, 'load', 'URL must be a non-empty string', { recoverable: false });
    }
    if (options.isLive !== undefined && typeof options.isLive !== 'boolean') {
      throw new CheetahMediaError(6002, 'sdk', 'isLive must be a boolean', { recoverable: true });
    }
    this.setState('loading');
    await this.teardownSession();
    const isLive = options.isLive ?? false;

    // Prefer main-thread MSE session when a media element is attached and the
    // protocol is natively MSE-compatible. Otherwise fall back to the WASM
    // control shell so unit tests and Future demux paths keep working.
    const protocolHint = this.config.transport.protocol;
    const protocol = detectProtocol(
      url,
      protocolHint === 'auto' ? 'auto' : (protocolHint as Protocol),
    );

    if (this.mediaElement && protocolSupportedByMseSession(protocol)) {
      try {
        // Best-effort worker bootstrap; ignore missing worker in pure-MSE mode.
        let bootstrapTimer: ReturnType<typeof setTimeout> | undefined;
        const bootstrapTimeout = new Promise<void>((_, reject) => {
          bootstrapTimer = setTimeout(
            () => reject(new Error('Runtime bootstrap timed out')),
            3000,
          );
        });
        try {
          await Promise.race([this.runtime.load(url, { isLive }), bootstrapTimeout]);
        } catch {
          // Worker/wasm may be unavailable in lightweight demos.
        } finally {
          clearTimeout(bootstrapTimer);
        }
        this.session = new PlaybackSession({
          videoElement: this.mediaElement,
          url,
          protocol,
          isLive,
          headers: this.config.transport.headers,
          softLatencyMs: this.config.latency.softMs,
          hardLatencyMs: this.config.latency.hardMs,
          maxPlaybackRate: this.config.latency.maxPlaybackRate,
          onEvent: (event) => this.handleSessionEvent(event),
        });
        await this.session.start();
        if (this._state !== 'failed' && this._state !== 'destroyed') {
          this.setState('preroll');
        }
        return;
      } catch (cause) {
        this.setState('failed');
        await this.teardownSession();
        throw new CheetahMediaError(6100, 'load', cause instanceof Error ? cause.message : 'Load failed', {
          cause,
          recoverable: true,
        });
      }
    }

    if (this.mediaElement && !protocolSupportedByMseSession(protocol)) {
      // Clearer than a fake preroll for Future/raw protocols (Annex-B, MPEG-PS, …).
      const message = `Protocol ${protocol} is not supported by the attached media element session`;
      this.setState('failed');
      const err = new CheetahMediaError(6003, 'source', message, { recoverable: false });
      this.emit('error', { error: err.toJSON() });
      throw err;
    }

    try {
      await this.runtime.load(url, { isLive });
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
    // Ignore play requests while the player is still bootstrapping or has already
    // failed/stopped; idle is allowed for the no-media / mock-runtime control path.
    if (this._state === 'loading' || this._state === 'failed' || this._state === 'stopping') {
      return;
    }
    if (this.session) {
      this.session.play();
      return;
    }
    this.runtime.play();
    this.setState('playing');
  }

  pause(): void {
    this.guardDestroyed();
    // Ignore pause requests unless the player is actually playing or rebuffering;
    // pausing before/during load or after failure is a no-op.
    if (this._state !== 'playing' && this._state !== 'rebuffering') {
      return;
    }
    if (this.session) {
      this.session.pause();
      return;
    }
    this.runtime.pause();
    this.setState('paused');
  }

  async seek(timeMs: number): Promise<void> {
    this.guardDestroyed();
    if (!Number.isFinite(timeMs) || timeMs < 0) {
      throw new CheetahMediaError(6002, 'sdk', 'seek timeMs must be a finite non-negative number', { recoverable: true });
    }
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Seek requires an active stream', { recoverable: true });
    }
    try {
      if (this.session) {
        await this.session.seek(timeMs);
        return;
      }
      await this.runtime.seek(timeMs);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'seek', 'Seek failed', { cause, recoverable: true });
    }
  }

  async setPlaybackRate(rate: number): Promise<void> {
    this.guardDestroyed();
    if (!Number.isFinite(rate) || rate < 0.1 || rate > 16) {
      throw new CheetahMediaError(6002, 'sdk', 'playback rate must be between 0.1 and 16', { recoverable: true });
    }
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Set playback rate requires an active stream', { recoverable: true });
    }
    try {
      if (this.session) {
        await this.session.setPlaybackRate(rate);
        return;
      }
      await this.runtime.setPlaybackRate(rate);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'playback-rate', 'Set playback rate failed', { cause, recoverable: true });
    }
  }

  async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
    this.guardDestroyed();
    if (direction !== 'forward' && direction !== 'backward') {
      throw new CheetahMediaError(6002, 'sdk', 'frameStep direction must be forward or backward', { recoverable: true });
    }
    if (typeof keyframeOnly !== 'boolean') {
      throw new CheetahMediaError(6002, 'sdk', 'frameStep keyframeOnly must be a boolean', { recoverable: true });
    }
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Frame step requires an active stream', { recoverable: true });
    }
    try {
      if (this.session) {
        await this.session.frameStep(direction, keyframeOnly);
        return;
      }
      await this.runtime.frameStep(direction, keyframeOnly);
    } catch (cause) {
      throw new CheetahMediaError(6999, 'frame-step', 'Frame step failed', { cause, recoverable: true });
    }
  }

  async pauseDisplay(keepConnection = true): Promise<void> {
    this.guardDestroyed();
    if (typeof keepConnection !== 'boolean') {
      throw new CheetahMediaError(6002, 'sdk', 'pauseDisplay keepConnection must be a boolean', { recoverable: true });
    }
    if (this._state !== 'playing' && this._state !== 'paused' && this._state !== 'preroll') {
      throw new CheetahMediaError(6002, 'sdk', 'Pause display requires an active stream', { recoverable: true });
    }
    try {
      if (this.session) {
        await this.session.pauseDisplay(keepConnection);
        return;
      }
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
      if (this.mediaRecorder && this.mediaRecorder.state !== 'inactive') {
        await this.stopRecording().catch(() => undefined);
      }
      await this.teardownSession();
      try {
        await this.runtime.stop();
      } catch {
        // Worker may be absent when only the MSE session was used.
      }
    } catch (cause) {
      this.setState('failed');
      throw new CheetahMediaError(6100, 'stop', 'Stop failed', { cause, recoverable: true });
    }
    this.setState('idle');
  }

  async destroy(): Promise<void> {
    if (this.destroyed) return;
    if (this.failedStateTimer) {
      clearTimeout(this.failedStateTimer);
      this.failedStateTimer = undefined;
    }
    this.pendingFailed = false;
    this.destroyed = true;
    this._intercomActive = false;
    this._intercomStarting = false;
    await this.cleanupDownload().catch(() => undefined);
    await this.cleanupIntercom().catch(() => undefined);
    await this.cleanupCompositeRecording().catch(() => undefined);
    await this.teardownSession();
    try {
      this._vrRenderer.destroy();
    } catch {
      // Extension destroy exceptions must not break shutdown.
    }
    try {
      this._aiProcessor.destroy();
    } catch {
      // Extension destroy exceptions must not break shutdown.
    }
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
    this.validateSnapshotOptions(options);
    // Prefer capturing from the attached video element (real MSE path).
    if (this.mediaElement && typeof document !== 'undefined') {
      try {
        return this.snapshotFromMediaElement(options);
      } catch (cause) {
        if (cause instanceof CheetahMediaError) throw cause;
        // Fall through to worker path.
      }
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
    this.validateRecordingOptions(options);
    if (this._state !== 'playing' && this._state !== 'paused') {
      throw new CheetahMediaError(6002, 'sdk', 'Recording requires an active stream', { recoverable: true });
    }
    if (this.mediaRecorder && this.mediaRecorder.state !== 'inactive') {
      throw new CheetahMediaError(6002, 'recording', 'Recording already active', { recoverable: true });
    }

    // Main-thread MediaRecorder from the video element when available.
    const video = this.mediaElement as HTMLVideoElement | null;
    if (
      video &&
      typeof MediaRecorder !== 'undefined' &&
      typeof (video as HTMLVideoElement & { captureStream?: (fps?: number) => MediaStream }).captureStream === 'function'
    ) {
      try {
        const stream = (
          video as HTMLVideoElement & { captureStream: (fps?: number) => MediaStream }
        ).captureStream(30);
        const preferred = options.mimeType ?? this.config.recording.mimeType;
        const mimeType =
          preferred && MediaRecorder.isTypeSupported(preferred)
            ? preferred
            : ['video/webm;codecs=vp9', 'video/webm', 'video/mp4'].find((m) => MediaRecorder.isTypeSupported(m)) ??
              '';
        this.mediaRecorderChunks = [];
        this.mediaRecorderMime = mimeType || undefined;
        this.mediaRecorderFilename = options.filename ?? this.config.recording.filename;
        this.mediaRecorder = mimeType
          ? new MediaRecorder(stream, { mimeType })
          : new MediaRecorder(stream);
        this.mediaRecorder.ondataavailable = (event) => {
          if (event.data && event.data.size > 0) {
            this.mediaRecorderChunks.push(event.data);
          }
        };
        this.mediaRecorder.start(1000);
        this.emit('recording', { active: true, mimeType: this.mediaRecorder.mimeType });
        return;
      } catch (cause) {
        throw new CheetahMediaError(6999, 'recording', 'Recording failed', { cause, recoverable: false });
      }
    }

    try {
      await this.runtime.request('start-recording', options, 5000);
      this.emit('recording', { active: true, ...options });
    } catch (cause) {
      throw new CheetahMediaError(6999, 'recording', 'Recording failed', { cause, recoverable: false });
    }
  }

  async stopRecording(): Promise<void> {
    this.guardDestroyed();
    if (this.mediaRecorder) {
      const recorder = this.mediaRecorder;
      this.mediaRecorder = undefined;
      await new Promise<void>((resolve) => {
        if (recorder.state === 'inactive') {
          resolve();
          return;
        }
        recorder.onstop = () => resolve();
        try {
          recorder.stop();
        } catch {
          resolve();
        }
      });
      const blob = new Blob(this.mediaRecorderChunks, {
        type: this.mediaRecorderMime ?? recorder.mimeType ?? 'video/webm',
      });
      this.mediaRecorderChunks = [];
      this.emit('recording', {
        active: false,
        blob,
        filename: this.mediaRecorderFilename,
        mimeType: blob.type,
        size: blob.size,
      });
      return;
    }
    try {
      await this.runtime.request('stop-recording', undefined, 5000);
      this.emit('recording', { active: false });
    } catch (cause) {
      throw new CheetahMediaError(6999, 'recording', 'Stop recording failed', { cause, recoverable: false });
    }
  }

  private async teardownSession(): Promise<void> {
    const session = this.session;
    this.session = undefined;
    if (session) {
      await session.stop().catch(() => undefined);
    }
  }

  private handleSessionEvent(event: PlaybackSessionEvent): void {
    switch (event.type) {
      case 'state': {
        if (event.state === 'ended') {
          this.setState('idle');
          return;
        }
        if (event.state === 'failed') {
          this.setState('failed');
          return;
        }
        if (event.state === 'loading') {
          this.setState('loading');
          return;
        }
        if (event.state === 'preroll') {
          this.setState('preroll');
          return;
        }
        if (event.state === 'playing') {
          this.setState('playing');
          return;
        }
        if (event.state === 'paused') {
          this.setState('paused');
          return;
        }
        if (event.state === 'rebuffering') {
          this.setState('rebuffering');
          this.emit('buffering', { reason: 'mse-waiting' });
          return;
        }
        return;
      }
      case 'tracks':
        this.emit('tracks', { tracks: event.tracks });
        return;
      case 'backend':
        this.emit('backendchange', { to: event.backend, reason: 'playback-session' });
        return;
      case 'firstframe':
        this.emit('firstframe', {});
        return;
      case 'error': {
        this.setState('failed');
        const err = new CheetahMediaError(event.code, event.stage, event.message, {
          recoverable: event.recoverable,
        });
        this.emit('error', { error: err.toJSON() });
        return;
      }
      case 'stats': {
        const bufferedMs =
          Number.isFinite(event.metrics.bufferedEnd) && Number.isFinite(event.metrics.bufferedStart)
            ? Math.max(0, (event.metrics.bufferedEnd - event.metrics.bufferedStart) * 1000)
            : undefined;
        this.emit('stats', {
          bufferedMs,
          networkBytes: event.networkBytes,
          droppedFrames: event.metrics.droppedSegments,
          stallCount: event.metrics.stallCount,
        });
        return;
      }
      default:
        return;
    }
  }

  private validateSnapshotOptions(options: { maxWidth?: number; maxHeight?: number }): void {
    if (options.maxWidth !== undefined) {
      if (!Number.isFinite(options.maxWidth) || options.maxWidth < 0 || options.maxWidth % 1 !== 0) {
        throw new CheetahMediaError(6002, 'sdk', 'maxWidth must be a non-negative integer', { recoverable: true });
      }
    }
    if (options.maxHeight !== undefined) {
      if (!Number.isFinite(options.maxHeight) || options.maxHeight < 0 || options.maxHeight % 1 !== 0) {
        throw new CheetahMediaError(6002, 'sdk', 'maxHeight must be a non-negative integer', { recoverable: true });
      }
    }
  }

  private validateSwitchVariant(variant: { bandwidth?: number; index?: number }): void {
    if (!variant || typeof variant !== 'object') {
      throw new CheetahMediaError(6002, 'sdk', 'Variant must be an object', { recoverable: true });
    }
    const hasBandwidth = variant.bandwidth !== undefined;
    const hasIndex = variant.index !== undefined;
    if (!hasBandwidth && !hasIndex) {
      throw new CheetahMediaError(6002, 'sdk', 'Variant bandwidth or index required', { recoverable: true });
    }
    const isValidInteger = (value: unknown): value is number =>
      typeof value === 'number' && Number.isFinite(value) && value >= 0 && value % 1 === 0;
    if (hasBandwidth && !isValidInteger(variant.bandwidth)) {
      throw new CheetahMediaError(6002, 'sdk', 'Variant bandwidth must be a non-negative integer', { recoverable: true });
    }
    if (hasIndex && !isValidInteger(variant.index)) {
      throw new CheetahMediaError(6002, 'sdk', 'Variant index must be a non-negative integer', { recoverable: true });
    }
  }

  private validateIntercomOptions(options: IntercomOptions): void {
    if (!options || typeof options !== 'object') {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom options must be an object', { recoverable: true });
    }
    if (options.codec !== undefined && options.codec !== 'g711a' && options.codec !== 'g711u' && options.codec !== 'opus') {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom codec must be g711a, g711u or opus', { recoverable: true });
    }
    if (typeof options.sendPacket !== 'function') {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom sendPacket must be a function', { recoverable: true });
    }
    if (options.onError !== undefined && typeof options.onError !== 'function') {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom onError must be a function', { recoverable: true });
    }
    if (options.payloadType !== undefined) {
      if (
        typeof options.payloadType !== 'number' ||
        !Number.isFinite(options.payloadType) ||
        options.payloadType < 0 ||
        options.payloadType > 127 ||
        options.payloadType % 1 !== 0
      ) {
        throw new CheetahMediaError(6002, 'sdk', 'Intercom payloadType must be an integer between 0 and 127', { recoverable: true });
      }
    }
  }

  private validateRecordingOptions(options: { mimeType?: string; filename?: string }): void {
    if (!options || typeof options !== 'object') {
      throw new CheetahMediaError(6002, 'sdk', 'Recording options must be an object', { recoverable: true });
    }
    if (options.mimeType !== undefined && typeof options.mimeType !== 'string') {
      throw new CheetahMediaError(6002, 'sdk', 'Recording mimeType must be a string', { recoverable: true });
    }
    if (options.filename !== undefined && typeof options.filename !== 'string') {
      throw new CheetahMediaError(6002, 'sdk', 'Recording filename must be a string', { recoverable: true });
    }
  }

  private validateCompositeRecordingOptions(options: unknown): void {
    if (!options || typeof options !== 'object') {
      throw new CheetahMediaError(6002, 'sdk', 'Composite recording options must be an object', { recoverable: true });
    }
    const opts = options as Record<string, unknown>;
    if (opts.watermarks !== undefined && opts.watermarks !== null && !Array.isArray(opts.watermarks)) {
      throw new CheetahMediaError(6002, 'sdk', 'Composite recording watermarks must be an array', { recoverable: true });
    }
    const watermarkList =
      Array.isArray(opts.watermarks) && opts.watermarks.length > 0
        ? opts.watermarks
        : opts.watermark !== undefined && opts.watermark !== null
          ? [opts.watermark]
          : undefined;
    if (Array.isArray(watermarkList)) {
      for (const wm of watermarkList) {
        if (!wm || typeof wm !== 'object') {
          throw new CheetahMediaError(6002, 'sdk', 'Composite recording watermark must be an object', { recoverable: true });
        }
        const mark = wm as Record<string, unknown>;
        const type = mark.type;
        if (type !== 'text' && type !== 'image' && type !== 'html') {
          throw new CheetahMediaError(6002, 'sdk', 'Composite recording watermark type must be text, image or html', { recoverable: true });
        }
        if (typeof mark.x !== 'number' || !Number.isFinite(mark.x) || typeof mark.y !== 'number' || !Number.isFinite(mark.y)) {
          throw new CheetahMediaError(6002, 'sdk', 'Composite recording watermark x and y must be finite numbers', { recoverable: true });
        }
        if (type === 'text' && typeof mark.text !== 'string') {
          throw new CheetahMediaError(6002, 'sdk', 'Composite recording text watermark must have a string text field', { recoverable: true });
        }
        if (type === 'image' && (!mark.image || typeof mark.image !== 'object')) {
          throw new CheetahMediaError(6002, 'sdk', 'Composite recording image watermark must have an image object', { recoverable: true });
        }
      }
    }
  }

  private snapshotFromMediaElement(options: { maxWidth?: number; maxHeight?: number }): ImageData {
    const video = this.mediaElement as HTMLVideoElement;
    const srcW =
      (video as HTMLVideoElement).videoWidth ||
      (video as HTMLVideoElement).clientWidth ||
      0;
    const srcH =
      (video as HTMLVideoElement).videoHeight ||
      (video as HTMLVideoElement).clientHeight ||
      0;
    if (srcW <= 0 || srcH <= 0) {
      throw new CheetahMediaError(6999, 'snapshot', 'Video frame not available', { recoverable: true });
    }
    let width = srcW;
    let height = srcH;
    if (options.maxWidth && width > options.maxWidth) {
      height = Math.max(1, Math.round((height * options.maxWidth) / width));
      width = options.maxWidth;
    }
    if (options.maxHeight && height > options.maxHeight) {
      width = Math.max(1, Math.round((width * options.maxHeight) / height));
      height = options.maxHeight;
    }
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      throw new CheetahMediaError(6999, 'snapshot', 'Canvas 2D unavailable', { recoverable: false });
    }
    ctx.drawImage(video as CanvasImageSource, 0, 0, width, height);
    return ctx.getImageData(0, 0, width, height);
  }

  async startIntercom(options: IntercomOptions): Promise<void> {
    this.guardDestroyed();
    this.validateIntercomOptions(options);
    if (this._intercomActive || this._intercomStarting) {
      throw new CheetahMediaError(6002, 'sdk', 'Intercom already active', { recoverable: true });
    }
    if (options.codec === 'opus') {
      throw new CheetahMediaError(6003, 'sdk', 'Opus intercom is not supported in this version', { recoverable: true });
    }

    this._intercomStarting = true;
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
      this._intercomStarting = false;
      await this.cleanupIntercom();
      const message = cause instanceof Error ? cause.message : String(cause);
      throw new CheetahMediaError(6999, 'intercom', `Intercom start failed: ${message}`, { cause, recoverable: true });
    } finally {
      this._intercomStarting = false;
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

  async startDownload(options: DownloadOptions): Promise<DownloadResult> {
    this.guardDestroyed();
    if (this._downloadController) {
      throw new CheetahMediaError(6002, 'sdk', 'Download already active', { recoverable: true });
    }
    let url: URL;
    try {
      url = new URL(options.url);
    } catch {
      throw new CheetahMediaError(7001, 'download', 'Invalid download URL', { recoverable: false });
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      throw new CheetahMediaError(7001, 'download', 'Only http/https downloads are supported', { recoverable: false });
    }

    const sink = options.sink ?? new BlobSink();
    this._downloadSink = sink;
    this._lastDownloadOptions = { ...options, sink };
    const controller = new StreamDownloader();
    this._downloadController = controller;
    const runtimeOptions: RuntimeDownloadOptions = {
      url: options.url,
      sink,
      onProgress: (progress: DownloadProgress) => this.emit('download', { active: true, progress }),
      onError: (error: TransportError) => this.emit('download', { active: false, error }),
      onComplete: () => this.emit('download', { active: false, completed: true }),
      ...(options.headers !== undefined ? { headers: options.headers } : {}),
      ...(options.credentials !== undefined ? { credentials: options.credentials } : {}),
      ...(options.transform !== undefined ? { transform: options.transform } : {}),
    };

    try {
      const result = await controller.start(runtimeOptions);
      if (controller.progress.state === 'completed' && sink instanceof BlobSink && options.filename) {
        this.saveBlob(sink.getBlob(), options.filename);
      }
      return result;
    } catch (cause) {
      const message = errorMessageFromCause(cause);
      throw new CheetahMediaError(6999, 'download', `Download failed: ${message}`, { cause, recoverable: true });
    } finally {
      if (this._downloadController === controller) {
        const state = this._downloadController.progress.state;
        if (state === 'completed' || state === 'error' || state === 'idle') {
          this._downloadController = undefined;
        }
      }
    }
  }

  pauseDownload(): void {
    this.guardDestroyed();
    this._downloadController?.pause();
  }

  async resumeDownload(options?: DownloadOptions): Promise<DownloadResult> {
    this.guardDestroyed();
    if (!this._downloadController) {
      throw new CheetahMediaError(6002, 'sdk', 'No paused download to resume', { recoverable: true });
    }
    const base = this._lastDownloadOptions;
    if (!base) {
      throw new CheetahMediaError(6002, 'sdk', 'No paused download to resume', { recoverable: true });
    }
    const url = options?.url ?? base.url;
    const sink = options?.sink ?? this._downloadSink ?? new BlobSink();
    this._downloadSink = sink;
    const runtimeOptions: RuntimeDownloadOptions = {
      url,
      sink,
      onProgress: (progress: DownloadProgress) => this.emit('download', { active: true, progress }),
      onError: (error: TransportError) => this.emit('download', { active: false, error }),
      onComplete: () => this.emit('download', { active: false, completed: true }),
      ...(options?.headers !== undefined ? { headers: options.headers } : base.headers !== undefined ? { headers: base.headers } : {}),
      ...(options?.credentials !== undefined
        ? { credentials: options.credentials }
        : base.credentials !== undefined
          ? { credentials: base.credentials }
          : {}),
      ...(options?.transform !== undefined
        ? { transform: options.transform }
        : base.transform !== undefined
          ? { transform: base.transform }
          : {}),
    };

    try {
      const result = await this._downloadController.resume(runtimeOptions);
      if (
        this._downloadController.progress.state === 'completed' &&
        sink instanceof BlobSink &&
        (options?.filename ?? base.filename)
      ) {
        this.saveBlob(sink.getBlob(), options?.filename ?? base.filename!);
      }
      return result;
    } catch (cause) {
      const message = errorMessageFromCause(cause);
      throw new CheetahMediaError(6999, 'download', `Download resume failed: ${message}`, { cause, recoverable: true });
    } finally {
      if (this._downloadController) {
        const state = this._downloadController.progress.state;
        if (state === 'completed' || state === 'error' || state === 'idle') {
          this._downloadController = undefined;
        }
      }
    }
  }

  async stopDownload(): Promise<void> {
    this.guardDestroyed();
    await this._downloadController?.stop();
    this._downloadController = undefined;
    this._downloadSink = undefined;
    this._lastDownloadOptions = undefined;
  }

  async startCompositeRecording(options: CompositeRecordingOptionsType): Promise<void> {
    this.guardDestroyed();
    this.validateCompositeRecordingOptions(options);
    if (this._compositeRecorder) {
      throw new CheetahMediaError(6002, 'sdk', 'Composite recording already active', { recoverable: true });
    }
    const recorder = new CompositeRecorder();
    this._compositeRecorder = recorder;
    this._lastCompositeOptions = options;
    const onComplete = (result: CompositeRecordingResultType): void => {
      this._compositeRecorder = undefined;
      this._lastCompositeOptions = undefined;
      if (options.filename && result.blob && result.blob.size > 0) {
        this.saveBlob(result.blob, options.filename);
      }
      this.emit('compositeRecording', { active: false, result });
    };
    const onError = (error: Error): void => {
      this._compositeRecorder = undefined;
      this._lastCompositeOptions = undefined;
      this.emit('compositeRecording', { active: false, error: errorMessageFromCause(error) });
    };
    const recorderOptions: CompositeRecordingOptionsType = {
      ...options,
      onComplete,
      onError,
    };
    try {
      await recorder.start(recorderOptions);
      if (this.state === 'destroyed' || this._compositeRecorder !== recorder) {
        await recorder.stop().catch(() => undefined);
        this._compositeRecorder = undefined;
        this._lastCompositeOptions = undefined;
        return;
      }
      this.emit('compositeRecording', { active: true, progress: recorder.progress });
    } catch (cause) {
      this._compositeRecorder = undefined;
      this._lastCompositeOptions = undefined;
      const message = errorMessageFromCause(cause);
      throw new CheetahMediaError(6999, 'composite-recording', `Composite recording failed to start: ${message}`, { cause, recoverable: true });
    }
  }

  pauseCompositeRecording(): void {
    this.guardDestroyed();
    if (!this._compositeRecorder) return;
    this._compositeRecorder.pause();
    this.emit('compositeRecording', { active: true, progress: this._compositeRecorder.progress, paused: true });
  }

  resumeCompositeRecording(): void {
    this.guardDestroyed();
    if (!this._compositeRecorder) return;
    this._compositeRecorder.resume();
    this.emit('compositeRecording', { active: true, progress: this._compositeRecorder.progress, paused: false });
  }

  async stopCompositeRecording(): Promise<CompositeRecordingResultType> {
    this.guardDestroyed();
    if (!this._compositeRecorder) {
      throw new CheetahMediaError(6002, 'sdk', 'No composite recording to stop', { recoverable: true });
    }
    const recorder = this._compositeRecorder;
    const options = this._lastCompositeOptions;
    this._compositeRecorder = undefined;
    this._lastCompositeOptions = undefined;
    try {
      const result = await recorder.stop();
      if (options?.filename && result.blob && result.blob.size > 0) {
        this.saveBlob(result.blob, options.filename);
      }
      this.emit('compositeRecording', { active: false, result });
      return result;
    } catch (cause) {
      this.emit('compositeRecording', { active: false, error: errorMessageFromCause(cause) });
      const message = errorMessageFromCause(cause);
      throw new CheetahMediaError(6999, 'composite-recording', `Composite recording failed: ${message}`, { cause, recoverable: true });
    }
  }

  setVrRenderer(renderer: VrRenderer): void {
    this.guardDestroyed();
    if (renderer !== undefined && renderer !== null) {
      if (
        typeof renderer !== 'object' ||
        typeof renderer.initialize !== 'function' ||
        typeof renderer.render !== 'function' ||
        typeof renderer.destroy !== 'function'
      ) {
        throw new CheetahMediaError(6002, 'sdk', 'VR renderer must expose initialize, render and destroy methods', { recoverable: true });
      }
    }
    try {
      this._vrRenderer.destroy();
    } catch {
      // Previous renderer destroy exceptions must not block the swap.
    }
    this._vrRenderer = renderer ?? new NoopVrRenderer();
  }

  setAiProcessor(processor: AiFrameProcessor): void {
    this.guardDestroyed();
    if (processor !== undefined && processor !== null) {
      if (
        typeof processor !== 'object' ||
        typeof processor.initialize !== 'function' ||
        typeof processor.process !== 'function' ||
        typeof processor.destroy !== 'function'
      ) {
        throw new CheetahMediaError(6002, 'sdk', 'AI processor must expose initialize, process and destroy methods', { recoverable: true });
      }
    }
    try {
      this._aiProcessor.destroy();
    } catch {
      // Previous processor destroy exceptions must not block the swap.
    }
    this._aiProcessor = processor ?? new NoopAiFrameProcessor();
  }

  private async cleanupDownload(): Promise<void> {
    const controller = this._downloadController;
    this._downloadController = undefined;
    this._downloadSink = undefined;
    this._lastDownloadOptions = undefined;
    if (controller) {
      try {
        await controller.stop();
      } catch {
        // stop() may fail if the download already completed; ignore.
      }
    }
  }

  private async cleanupCompositeRecording(): Promise<void> {
    const recorder = this._compositeRecorder;
    this._compositeRecorder = undefined;
    this._lastCompositeOptions = undefined;
    if (recorder) {
      try {
        await recorder.stop();
      } catch {
        // stop() may fail if the recording already completed; ignore.
      }
    }
  }

  private saveBlob(blob: Blob, filename: string): void {
    if (typeof document === 'undefined') return;
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }

  private async cleanupIntercom(): Promise<void> {
    const capture = this._intercomCapture;
    this._intercomCapture = undefined;
    if (capture) {
      try {
        await capture.stop();
      } catch {
        // stop() may fail if the capture was never running; ignore.
      }
    }
    this._intercomPacketizer = undefined;
    this._intercomSend = undefined;
    this._intercomError = undefined;
  }

  private handleIntercomPacket(packet: IntercomPacket): void {
    try {
      this._intercomSend?.(packet);
    } catch (error) {
      this.reportIntercomError(error instanceof Error ? error : new Error(String(error)), false);
    }
  }

  private handleCaptureError(error: CaptureError): void {
    if (!this._intercomActive) {
      void this.cleanupIntercom();
      return;
    }
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
    this.validateSwitchVariant(variant);
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
    if (typeof type !== 'string') {
      throw new CheetahMediaError(6002, 'sdk', 'Event type must be a string', { recoverable: true });
    }
    if (typeof listener !== 'function') {
      throw new CheetahMediaError(6002, 'sdk', 'Event listener must be a function', { recoverable: true });
    }
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
