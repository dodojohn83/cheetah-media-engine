/**
 * Browser capability report used to decide isolation level, codec strategy and
 * the initial playback route.
 *
 * `detectCapabilities()` performs cheap synchronous detection of globals and
 * can be used in workers or during startup.  `probeCapabilities()` performs
 * deeper async probes (`isConfigSupported`, `MediaSource.isTypeSupported`,
 * WebAssembly feature validation) and should be cached per environment.
 */

export type CapabilityConfidence = 'high' | 'medium' | 'low';

export interface CapabilityReport {
  /** Synchronous baseline flags. */
  readonly secureContext: boolean;
  readonly crossOriginIsolated: boolean;
  readonly sharedArrayBuffer: boolean;
  readonly atomics: boolean;
  readonly simd: boolean;
  readonly threads: boolean;
  readonly webCodecs: boolean;
  readonly mse: boolean;
  readonly webAudio: boolean;
  readonly offscreenCanvas: boolean;
  readonly webgpu: boolean;
  readonly webgl2: boolean;
  readonly canvas2d: boolean;
  readonly videoFrame: boolean;
  /** `true` when the `WebAssembly` global is present and a small Memory can be created. */
  readonly wasm: boolean;

  /** Environment fingerprint used to invalidate stale cache entries. */
  readonly fingerprint: string;
  readonly timestamp: number;
  readonly confidence: CapabilityConfidence;
  readonly reasons: readonly string[];
}

export interface ProbeDetails {
  readonly webCodecs: Readonly<Record<string, boolean>>;
  readonly mse: Readonly<Record<string, boolean>>;
  readonly wasm: {
    readonly simd: boolean;
    readonly threads: boolean;
    readonly sharedMemory: boolean;
    readonly memoryLimitPages: number;
  };
  readonly renderer: {
    readonly webgpu: boolean;
    readonly webgl2: boolean;
    readonly canvas2d: boolean;
    readonly videoFrame: boolean;
    readonly preferredPixelFormat: string | undefined;
  };
}

export interface ProbedCapabilityReport extends CapabilityReport {
  readonly details: ProbeDetails;
}

function hasGlobal(name: string): boolean {
  return typeof globalThis !== 'undefined' && name in globalThis;
}

function getGlobal<T>(name: string): T | undefined {
  return hasGlobal(name) ? (globalThis as unknown as Record<string, T>)[name] : undefined;
}

function hasSimdSupport(): boolean {
  if (!hasGlobal('WebAssembly')) return false;
  // Minimal SIMD module: v128.any_true (0xfd 0x0d).
  const bytes = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
    0x03, 0x02, 0x01, 0x00,
    0x0a, 0x0b, 0x01, 0x09, 0x00, 0x00, 0xfd, 0x0d, 0x00, 0x00, 0x0b,
  ]);
  return (WebAssembly as unknown as { validate?: (buffer: ArrayBuffer) => boolean }).validate?.(bytes.buffer) ?? false;
}

function hasThreadsSupport(): boolean {
  if (!hasGlobal('Worker')) return false;
  if (!hasGlobal('SharedArrayBuffer')) return false;
  if (!hasGlobal('Atomics')) return false;
  try {
    new SharedArrayBuffer(4);
    return true;
  } catch {
    return false;
  }
}

function hasWasmSupport(): boolean {
  if (!hasGlobal('WebAssembly')) return false;
  try {
    new WebAssembly.Memory({ initial: 1, maximum: 256 });
    return true;
  } catch {
    return false;
  }
}

function computeFingerprint(): string {
  const parts: (string | number | boolean | undefined)[] = [];
  parts.push(typeof navigator !== 'undefined' ? navigator.userAgent : '');
  parts.push(typeof navigator !== 'undefined' ? navigator.hardwareConcurrency : 0);
  parts.push(typeof navigator !== 'undefined' ? (navigator as unknown as { deviceMemory?: number }).deviceMemory : undefined);
  parts.push(typeof navigator !== 'undefined' ? navigator.platform : '');
  parts.push((globalThis as unknown as { isSecureContext?: boolean }).isSecureContext ?? false);
  return parts.join('|');
}

/**
 * Detect runtime capabilities without async work. Safe to call in node/vitest
 * because it only probes globals and catches failures.
 */
export function detectCapabilities(): CapabilityReport {
  const secureContext = (globalThis as unknown as { isSecureContext?: boolean }).isSecureContext ?? false;
  const crossOriginIsolated = (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated ?? false;
  const sharedArrayBuffer = hasGlobal('SharedArrayBuffer');
  const atomics = hasGlobal('Atomics');
  const simd = hasSimdSupport();
  const threads = hasThreadsSupport();
  const webCodecs = hasGlobal('VideoDecoder') && hasGlobal('AudioDecoder');
  const mse = hasGlobal('MediaSource');
  const webAudio = hasGlobal('AudioContext') || hasGlobal('webkitAudioContext');
  const offscreenCanvas = hasGlobal('OffscreenCanvas');
  const webgpu = hasGlobal('GPU');
  const webgl2 = hasGlobal('WebGL2RenderingContext');
  const canvas2d = hasGlobal('HTMLCanvasElement');
  const videoFrame = hasGlobal('VideoFrame');
  const wasm = hasWasmSupport();

  const reasons: string[] = [];
  if (secureContext) reasons.push('secure-context');
  if (crossOriginIsolated) reasons.push('cross-origin-isolated');
  if (sharedArrayBuffer) reasons.push('shared-array-buffer');
  if (atomics) reasons.push('atomics');
  if (simd) reasons.push('simd');
  if (threads) reasons.push('threads');
  if (webCodecs) reasons.push('webcodecs-api');
  if (mse) reasons.push('mse-api');
  if (webgpu) reasons.push('webgpu');
  if (webgl2) reasons.push('webgl2');
  if (canvas2d) reasons.push('canvas2d');

  return {
    secureContext,
    crossOriginIsolated,
    sharedArrayBuffer,
    atomics,
    simd,
    threads,
    webCodecs,
    mse,
    webAudio,
    offscreenCanvas,
    webgpu,
    webgl2,
    canvas2d,
    videoFrame,
    wasm,
    fingerprint: computeFingerprint(),
    timestamp: performance.now(),
    confidence: 'low',
    reasons,
  };
}

interface VideoConfig {
  readonly codec: string;
  readonly width: number;
  readonly height: number;
}

interface AudioConfig {
  readonly codec: string;
  readonly sampleRate: number;
  readonly numberOfChannels: number;
}

const VIDEO_PROBE_CONFIGS: VideoConfig[] = [
  { codec: 'avc1.42001E', width: 640, height: 360 },
  { codec: 'hvc1.1.6.L93.B0', width: 640, height: 360 },
];

const AUDIO_PROBE_CONFIGS: AudioConfig[] = [
  { codec: 'mp4a.40.2', sampleRate: 48000, numberOfChannels: 2 },
  { codec: 'alaw', sampleRate: 8000, numberOfChannels: 1 },
  { codec: 'ulaw', sampleRate: 8000, numberOfChannels: 1 },
  { codec: 'mp3', sampleRate: 48000, numberOfChannels: 2 },
];

async function probeWebCodecsVideo(): Promise<Record<string, boolean>> {
  const VideoDecoder = getGlobal<typeof globalThis.VideoDecoder>('VideoDecoder');
  const result: Record<string, boolean> = {};
  if (!VideoDecoder || typeof VideoDecoder.isConfigSupported !== 'function') return result;

  for (const cfg of VIDEO_PROBE_CONFIGS) {
    try {
      const support = await VideoDecoder.isConfigSupported(cfg);
      result[cfg.codec.toLowerCase()] = support.supported ?? false;
    } catch {
      result[cfg.codec.toLowerCase()] = false;
    }
  }
  return result;
}

async function probeWebCodecsAudio(): Promise<Record<string, boolean>> {
  const AudioDecoder = getGlobal<typeof globalThis.AudioDecoder>('AudioDecoder');
  const result: Record<string, boolean> = {};
  if (!AudioDecoder || typeof AudioDecoder.isConfigSupported !== 'function') return result;

  for (const cfg of AUDIO_PROBE_CONFIGS) {
    try {
      const support = await AudioDecoder.isConfigSupported(cfg);
      result[cfg.codec.toLowerCase()] = support.supported ?? false;
    } catch {
      result[cfg.codec.toLowerCase()] = false;
    }
  }
  return result;
}

function makeMime(type: 'video' | 'audio', codec: string): string {
  if (codec === 'mp3') return 'audio/mpeg';
  if (codec === 'alaw' || codec === 'ulaw') return `audio/${codec}`;
  return `${type === 'video' ? 'video' : 'audio'}/mp4;codecs="${codec}"`;
}

function probeMse(): Record<string, boolean> {
  const MediaSource = getGlobal<typeof globalThis.MediaSource>('MediaSource');
  const result: Record<string, boolean> = {};
  if (!MediaSource || typeof MediaSource.isTypeSupported !== 'function') return result;

  const codecs = [...VIDEO_PROBE_CONFIGS.map((c) => c.codec), ...AUDIO_PROBE_CONFIGS.map((c) => c.codec)];
  for (const codec of codecs) {
    const type = VIDEO_PROBE_CONFIGS.some((c) => c.codec === codec) ? 'video' : 'audio';
    try {
      result[codec.toLowerCase()] = MediaSource.isTypeSupported(makeMime(type, codec));
    } catch {
      result[codec.toLowerCase()] = false;
    }
  }
  return result;
}

function probeWasmMemoryLimit(): number {
  if (!hasGlobal('WebAssembly')) return 0;
  // Try to create a memory with the maximum allowed pages to detect the
  // practical per-process limit without allocating much actual RAM.
  const candidates = [32767, 16384, 8192, 4096, 2048, 1024, 512, 256];
  for (const pages of candidates) {
    try {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const memory = new WebAssembly.Memory({ initial: 1, maximum: pages, shared: false });
      // Touch a small amount of memory so the runtime commits at least one page.
      const view = new Uint8Array(memory.buffer);
      view[0] = 0;
      return pages;
    } catch {
      // try smaller
    }
  }
  return 0;
}

function probeWasmThreads(): boolean {
  if (!hasGlobal('WebAssembly') || !hasGlobal('SharedArrayBuffer')) return false;
  // Minimal atomic.wait/notify (threads) feature module.
  const bytes = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f,
    0x03, 0x02, 0x01, 0x00,
    0x07, 0x0b, 0x01, 0x07, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00, 0x02,
    0x0a, 0x09, 0x01, 0x07, 0x00, 0x00, 0xfd, 0x1f, 0x00, 0x00, 0x0b,
  ]);
  return (WebAssembly as unknown as { validate?: (buffer: ArrayBuffer) => boolean }).validate?.(bytes.buffer) ?? false;
}

function probeWasmSharedMemory(): boolean {
  if (!hasGlobal('WebAssembly')) return false;
  try {
    const memory = new WebAssembly.Memory({ initial: 1, maximum: 256, shared: true });
    return memory.buffer instanceof SharedArrayBuffer;
  } catch {
    return false;
  }
}

function probeRenderer(): { webgpu: boolean; webgl2: boolean; canvas2d: boolean; videoFrame: boolean; preferredPixelFormat: string | undefined } {
  const webgpu = hasGlobal('GPU');
  const webgl2 = hasGlobal('WebGL2RenderingContext');
  const canvas2d = hasGlobal('HTMLCanvasElement');
  const videoFrame = hasGlobal('VideoFrame');
  let preferredPixelFormat: string | undefined;

  if (videoFrame && typeof (globalThis as unknown as { VideoFrame?: { prototype: { format?: string } } }).VideoFrame?.prototype?.format === 'string') {
    const VideoFrame = (globalThis as unknown as { VideoFrame: { prototype: { format: string } } }).VideoFrame;
    preferredPixelFormat = VideoFrame.prototype.format;
  }
  return { webgpu, webgl2, canvas2d, videoFrame, preferredPixelFormat };
}

/**
 * Perform async capability probes and merge them with the sync report.
 *
 * The returned report has `confidence: 'high'` only when the actual
 * `isConfigSupported` / `MediaSource.isTypeSupported` calls succeed.
 */
export async function probeCapabilities(): Promise<ProbedCapabilityReport> {
  const base = detectCapabilities();

  const [webCodecsVideo, webCodecsAudio, mse, memoryLimitPages] = await Promise.all([
    probeWebCodecsVideo(),
    probeWebCodecsAudio(),
    Promise.resolve(probeMse()),
    Promise.resolve(probeWasmMemoryLimit()),
  ]);

  const wasmThreads = probeWasmThreads();
  const wasmSharedMemory = probeWasmSharedMemory();
  const renderer = probeRenderer();

  const webCodecsMap: Record<string, boolean> = {};
  for (const [codec, supported] of Object.entries(webCodecsVideo)) {
    webCodecsMap[codec] = supported;
  }
  for (const [codec, supported] of Object.entries(webCodecsAudio)) {
    webCodecsMap[codec] = supported;
  }

  const confidence: CapabilityConfidence = base.webCodecs || base.mse ? 'high' : 'medium';
  const reasons = [...base.reasons];
  if (Object.values(webCodecsMap).some(Boolean)) reasons.push('webcodecs-config-supported');
  if (Object.values(mse).some(Boolean)) reasons.push('mse-codec-supported');

  return {
    ...base,
    confidence,
    reasons,
    details: {
      webCodecs: webCodecsMap,
      mse,
      wasm: {
        simd: base.simd,
        threads: wasmThreads,
        sharedMemory: wasmSharedMemory,
        memoryLimitPages,
      },
      renderer,
    },
  };
}

interface CachedEntry {
  report: ProbedCapabilityReport;
  version: string;
}

/**
 * Simple capability cache keyed by environment fingerprint. The cache is
 * invalidated when the fingerprint, browser version or codec pack version
 * changes.
 */
export class CapabilityCache {
  private readonly ttlMs: number;
  private entry: CachedEntry | undefined = undefined;
  private browserVersion = '';
  private codecPackVersion = '';

  constructor(ttlMs = 30000) {
    this.ttlMs = ttlMs;
  }

  setEnvironment(browserVersion: string, codecPackVersion: string): void {
    this.browserVersion = browserVersion;
    this.codecPackVersion = codecPackVersion;
    if (this.entry && (this.entry.version !== `${browserVersion}|${codecPackVersion}`)) {
      this.entry = undefined;
    }
  }

  /**
   * Return a cached report if the environment fingerprint and versions match.
   */
  get(fingerprint: string): ProbedCapabilityReport | undefined {
    if (!this.entry) return undefined;
    if (this.entry.report.fingerprint !== fingerprint) return undefined;
    if (performance.now() - this.entry.report.timestamp > this.ttlMs) return undefined;
    return this.entry.report;
  }

  /**
   * Store a fresh report keyed by the current environment versions.
   */
  put(report: ProbedCapabilityReport): void {
    this.entry = {
      report,
      version: `${this.browserVersion}|${this.codecPackVersion}`,
    };
  }

  /**
   * Refresh the cache by calling `probeCapabilities()` when no valid entry
   * exists for the current environment fingerprint and versions.
   */
  async probe(): Promise<ProbedCapabilityReport> {
    const fresh = await probeCapabilities();
    const cached = this.get(fresh.fingerprint);
    if (cached) return cached;
    this.put(fresh);
    return fresh;
  }
}
