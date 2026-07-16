/**
 * Route planner: turn a capability report and a stream request into a ranked
 * list of playback backend candidates.
 *
 * The planner is pure and deterministic. It does not start backends, allocate
 * hardware or perform I/O; it only produces a `PlaybackPlan` that the runtime
 * executes and falls back through.
 */

import type { CapabilityReport, ProbedCapabilityReport, ProbeDetails } from './capabilities';

export type Protocol =
  | 'http-flv'
  | 'ws-flv'
  | 'http-fmp4'
  | 'ws-fmp4'
  | 'http-annexb'
  | 'ws-annexb'
  | 'http-mpegps'
  | 'ws-mpegps'
  | 'webtransport'
  | 'webrtc'
  | 'hls'
  | 'll-hls';
export type Backend = 'webcodecs' | 'mse' | 'wasm-threads-simd' | 'wasm-simd' | 'wasm-baseline';
export type Renderer = 'webgpu' | 'webgl2' | 'canvas2d';
export type TransportMode = 'fetch' | 'websocket' | 'webtransport' | 'webrtc';
export type LatencyTarget = 'realtime' | 'low' | 'normal';

export interface TrackProfile {
  readonly kind: 'video' | 'audio';
  readonly codec: string;
  readonly width?: number;
  readonly height?: number;
  readonly sampleRate?: number;
  readonly channels?: number;
}

export interface PlanRequest {
  readonly protocol: Protocol;
  readonly tracks: readonly TrackProfile[];
  readonly latencyTarget: LatencyTarget;
  /** Whether the page is cross-origin isolated (COOP/COEP). */
  readonly isolation?: boolean;
  /** Backends the user explicitly disabled. */
  readonly disabled?: readonly Backend[];
  /** Optional resource budget. */
  readonly budget?: {
    readonly maxWasmMemoryMB?: number;
    readonly maxThreads?: number;
  };
}

export interface PlanCandidate {
  readonly rank: number;
  readonly videoBackend: Backend | undefined;
  readonly audioBackend: Backend | undefined;
  readonly renderer: Renderer | undefined;
  readonly transport: TransportMode;
  readonly reason: string;
}

export interface PlaybackPlan {
  readonly candidates: readonly PlanCandidate[];
  readonly primary: PlanCandidate;
  readonly fallback: readonly PlanCandidate[];
  /** Candidates that were considered but unsupported, with reasons. */
  readonly unsupported: readonly { readonly backend: Backend; readonly reason: string }[];
  /** `true` if no fully-preferred route exists and a lower-quality path was chosen. */
  readonly degraded: boolean;
  /** Human-readable reason chain explaining the selection. */
  readonly reasonChain: readonly string[];
}

const VIDEO_CODECS = new Set(['h264', 'h265', 'av1']);
const AUDIO_CODECS = new Set(['aac', 'g711a', 'g711u', 'mp3']);

const DEFAULT_VIDEO_ROUTE: Backend[] = ['webcodecs', 'mse', 'wasm-threads-simd', 'wasm-simd', 'wasm-baseline'];
const DEFAULT_AUDIO_ROUTE: Backend[] = ['webcodecs', 'mse', 'wasm-threads-simd', 'wasm-simd', 'wasm-baseline'];

const CODEC_ALIASES: Record<string, string> = {
  h264: 'avc1.42001e',
  h265: 'hvc1.1.6.l93.b0',
  hevc: 'hvc1.1.6.l93.b0',
  av1: 'av01.0.04m.10',
  aac: 'mp4a.40.2',
  g711a: 'alaw',
  alaw: 'alaw',
  g711u: 'ulaw',
  ulaw: 'ulaw',
  mp3: 'mp3',
};

function normalizeCodec(codec: string): string {
  return CODEC_ALIASES[codec.toLowerCase()] ?? codec.toLowerCase();
}

function isVideoCodec(codec: string): boolean {
  return VIDEO_CODECS.has(codec.toLowerCase());
}

function isAudioCodec(codec: string): boolean {
  return AUDIO_CODECS.has(codec.toLowerCase());
}

function getDetails(caps: CapabilityReport): ProbeDetails | undefined {
  return (caps as Partial<ProbedCapabilityReport>).details;
}

function codecHasWebCodecsSupport(caps: CapabilityReport, codec: string): boolean {
  if (!caps.webCodecs) return false;
  const details = getDetails(caps);
  if (!details) return true; // no per-codec probe, allow optimistically
  return !!details.webCodecs[codec.toLowerCase()];
}

function codecHasMseSupport(caps: CapabilityReport, codec: string): boolean {
  if (!caps.mse) return false;
  const details = getDetails(caps);
  if (!details) return true;
  return !!details.mse[codec.toLowerCase()];
}

function canMseContainer(request: PlanRequest): boolean {
  // MSE directly supports fMP4/MP4 containers. HLS with fMP4 segments is fine.
  // FLV, raw Annex-B and MPEG-PS require demux/remux before MSE in this engine.
  switch (request.protocol) {
    case 'http-fmp4':
    case 'ws-fmp4':
    case 'hls':
    case 'll-hls':
      return true;
    case 'http-flv':
    case 'ws-flv':
    case 'http-annexb':
    case 'ws-annexb':
    case 'http-mpegps':
    case 'ws-mpegps':
    case 'webtransport':
    case 'webrtc':
      return false;
    default:
      return false;
  }
}

function protocolRequiresDemux(protocol: Protocol): boolean {
  // MPEG-PS is a container and must be demuxed before any browser decoder.
  // Annex-B is already an elementary stream, so it can be fed to a decoder
  // after NAL boundaries are identified.
  // WebRTC data channel in this skeleton carries raw Annex-B bytes.
  return protocol === 'http-mpegps' || protocol === 'ws-mpegps';
}

function wasmMemoryOk(caps: CapabilityReport, budget: PlanRequest['budget']): boolean {
  if (!caps.wasm) return false;
  const details = getDetails(caps);
  const memoryLimitPages = details?.wasm.memoryLimitPages ?? 0;
  if (memoryLimitPages === 0) return true; // unknown/uncached
  const maxWasmMemoryMB = budget?.maxWasmMemoryMB;
  if (maxWasmMemoryMB !== undefined) {
    const maxPages = Math.floor((maxWasmMemoryMB * 1024 * 1024) / 65536);
    return memoryLimitPages >= maxPages;
  }
  return true;
}

function wasmThreadsAvailable(caps: CapabilityReport, request: PlanRequest): boolean {
  if (!caps.threads || !caps.sharedArrayBuffer || !caps.atomics) return false;
  if (!request.isolation) return false;
  const details = getDetails(caps);
  if (details && !details.wasm.threads) return false;
  const maxThreads = request.budget?.maxThreads ?? Number.MAX_SAFE_INTEGER;
  return maxThreads >= 2;
}

function wasmSimdAvailable(caps: CapabilityReport): boolean {
  return caps.simd;
}

function codecSupportedByWasm(_caps: CapabilityReport, _codec: string): boolean {
  // Software WASM decoders can decode any codec the engine ships.  The
  // planner assumes all listed codecs are available once the WASM baseline is.
  return true;
}

function backendSupportsCodec(
  backend: Backend,
  track: TrackProfile,
  caps: CapabilityReport,
  request: PlanRequest,
): { supported: boolean; reason?: string } {
  const codec = normalizeCodec(track.codec);

  if (track.kind === 'video' && !isVideoCodec(track.codec)) {
    return { supported: false, reason: `unknown video codec ${track.codec}` };
  }
  if (track.kind === 'audio' && !isAudioCodec(track.codec)) {
    return { supported: false, reason: `unknown audio codec ${track.codec}` };
  }

  switch (backend) {
    case 'webcodecs':
      if (!caps.webCodecs) return { supported: false, reason: 'WebCodecs API unavailable' };
      if (protocolRequiresDemux(request.protocol)) {
        return { supported: false, reason: `WebCodecs cannot demux ${request.protocol}` };
      }
      if (!codecHasWebCodecsSupport(caps, codec)) {
        return { supported: false, reason: `WebCodecs does not support ${track.codec}` };
      }
      return { supported: true };

    case 'mse':
      if (!caps.mse) return { supported: false, reason: 'MSE API unavailable' };
      if (!canMseContainer(request)) {
        return { supported: false, reason: `MSE cannot play ${request.protocol}` };
      }
      if (track.kind === 'video' && !isVideoCodec(track.codec)) {
        return { supported: false, reason: `MSE does not support video codec ${track.codec}` };
      }
      if (!codecHasMseSupport(caps, codec)) {
        return { supported: false, reason: `MSE MIME/codec not supported for ${track.codec}` };
      }
      return { supported: true };

    case 'wasm-threads-simd':
      if (!wasmThreadsAvailable(caps, request)) {
        return { supported: false, reason: 'WASM threads require COOP/COEP, SharedArrayBuffer and Atomics' };
      }
      if (!wasmSimdAvailable(caps)) return { supported: false, reason: 'WASM SIMD not available' };
      if (!wasmMemoryOk(caps, request.budget)) return { supported: false, reason: 'WASM memory budget too small' };
      if (!codecSupportedByWasm(caps, codec)) return { supported: false, reason: `WASM codec pack missing ${track.codec}` };
      return { supported: true };

    case 'wasm-simd':
      if (!wasmSimdAvailable(caps)) return { supported: false, reason: 'WASM SIMD not available' };
      if (!wasmMemoryOk(caps, request.budget)) return { supported: false, reason: 'WASM memory budget too small' };
      if (!codecSupportedByWasm(caps, codec)) return { supported: false, reason: `WASM codec pack missing ${track.codec}` };
      return { supported: true };

    case 'wasm-baseline':
      if (!caps.wasm) return { supported: false, reason: 'WebAssembly unavailable' };
      if (!wasmMemoryOk(caps, request.budget)) return { supported: false, reason: 'WASM memory budget too small' };
      if (!codecSupportedByWasm(caps, codec)) return { supported: false, reason: `WASM codec pack missing ${track.codec}` };
      return { supported: true };
  }
}

function selectBestBackend(
  kind: 'video' | 'audio',
  request: PlanRequest,
  caps: CapabilityReport,
  route: Backend[],
): { backend: Backend | undefined; reason: string; unsupported: { backend: Backend; reason: string }[] } {
  const unsupported: { backend: Backend; reason: string }[] = [];
  const track = request.tracks.find((t) => t.kind === kind);
  if (!track) {
    return { backend: undefined, reason: `no ${kind} track`, unsupported };
  }

  for (const backend of route) {
    if (request.disabled?.includes(backend)) {
      unsupported.push({ backend, reason: 'disabled by caller' });
      continue;
    }
    const result = backendSupportsCodec(backend, track, caps, request);
    if (result.supported) {
      return { backend, reason: result.reason ?? `${backend} supports ${track.codec}`, unsupported };
    }
    unsupported.push({ backend, reason: result.reason ?? `${backend} cannot handle ${track.codec}` });
  }

  return { backend: undefined, reason: `no backend supports ${track.codec}`, unsupported };
}

function chooseRenderer(caps: CapabilityReport): Renderer | undefined {
  if (caps.webgpu && caps.videoFrame) return 'webgpu';
  if (caps.webgl2) return 'webgl2';
  if (caps.canvas2d) return 'canvas2d';
  return undefined;
}

function chooseTransport(request: PlanRequest): TransportMode {
  switch (request.protocol) {
    case 'ws-flv':
    case 'ws-fmp4':
    case 'ws-annexb':
    case 'ws-mpegps':
      return 'websocket';
    case 'webtransport':
      return 'webtransport';
    case 'webrtc':
      return 'webrtc';
    case 'http-flv':
    case 'http-fmp4':
    case 'http-annexb':
    case 'http-mpegps':
    case 'hls':
    case 'll-hls':
      return 'fetch';
    default:
      return 'fetch';
  }
}

function latencyPenalty(transport: TransportMode, latency: LatencyTarget, backend: Backend): number {
  let penalty = 0;
  if (transport === 'fetch' && latency === 'realtime') penalty += 1;
  if (backend === 'wasm-baseline' && (latency === 'realtime' || latency === 'low')) penalty += 1;
  if (backend === 'wasm-simd' && latency === 'realtime') penalty += 0;
  return penalty;
}

/**
 * Build a playback plan from a request and a capability report.
 *
 * The primary candidate is the first valid combination of video and audio
 * backends; fallback candidates are generated by downgrading each decoder path
 * independently while keeping the result valid.
 */
export function plan(request: PlanRequest, caps: CapabilityReport): PlaybackPlan {
  const unsupported: { backend: Backend; reason: string }[] = [];
  const reasonChain: string[] = [];

  const videoResult = selectBestBackend('video', request, caps, DEFAULT_VIDEO_ROUTE);
  const audioResult = selectBestBackend('audio', request, caps, DEFAULT_AUDIO_ROUTE);

  unsupported.push(...videoResult.unsupported, ...audioResult.unsupported);

  const renderer = chooseRenderer(caps);
  const transport = chooseTransport(request);

  reasonChain.push(`transport=${transport}`);
  if (renderer) reasonChain.push(`renderer=${renderer}`);

  const candidates: PlanCandidate[] = [];

  // Generate fallback candidates by taking the next-best backend for video
  // and/or audio.  We avoid combining every permutation explicitly to keep the
  // plan small and deterministic.
  const videoOptions = DEFAULT_VIDEO_ROUTE;
  const audioOptions = DEFAULT_AUDIO_ROUTE;

  const seen = new Set<string>();

  for (const vBackend of videoOptions) {
    if (request.disabled?.includes(vBackend)) continue;
    const vTrack = request.tracks.find((t) => t.kind === 'video');
    if (vTrack && !backendSupportsCodec(vBackend, vTrack, caps, request).supported) continue;

    for (const aBackend of audioOptions) {
      if (request.disabled?.includes(aBackend)) continue;
      const aTrack = request.tracks.find((t) => t.kind === 'audio');
      if (aTrack && !backendSupportsCodec(aBackend, aTrack, caps, request).supported) continue;

      // MSE renders through the browser element; do not pair it with a custom renderer.
      const candidateRenderer = vBackend === 'mse' ? undefined : renderer;

      const key = `${vBackend}:${aBackend}:${candidateRenderer ?? 'none'}`;
      if (seen.has(key)) continue;
      seen.add(key);

      const reasons: string[] = [];
      if (vTrack) reasons.push(`video=${vBackend}`);
      if (aTrack) reasons.push(`audio=${aBackend}`);
      if (candidateRenderer) reasons.push(`renderer=${candidateRenderer}`);

      const penalty = latencyPenalty(transport, request.latencyTarget, vBackend) + latencyPenalty(transport, request.latencyTarget, aBackend);

      const candidate: PlanCandidate = {
        rank: candidates.length + 1 + penalty,
        videoBackend: vTrack ? vBackend : undefined,
        audioBackend: aTrack ? aBackend : undefined,
        renderer: candidateRenderer,
        transport,
        reason: reasons.join(', '),
      };
      candidates.push(candidate);
    }
  }

  // Sort by rank; lower is better.
  candidates.sort((a, b) => a.rank - b.rank);

  const primary = candidates[0];

  if (candidates.length === 0 || !primary) {
    return {
      candidates,
      primary: {
        rank: 0,
        videoBackend: undefined,
        audioBackend: undefined,
        renderer: undefined,
        transport,
        reason: 'no supported playback route',
      },
      fallback: [],
      unsupported,
      degraded: true,
      reasonChain: [...reasonChain, 'no supported route'],
    };
  }

  return {
    candidates,
    primary,
    fallback: candidates.slice(1),
    unsupported,
    degraded: primary.videoBackend !== 'webcodecs' && primary.videoBackend !== undefined,
    reasonChain: [...reasonChain, `primary=${primary.reason}`],
  };
}

/**
 * Explain why a candidate was chosen or excluded.
 */
export function explain(plan: PlaybackPlan): string {
  const lines: string[] = [];
  lines.push(`primary: ${plan.primary.reason}`);
  for (const c of plan.candidates.slice(1)) {
    lines.push(`fallback[${c.rank}]: ${c.reason}`);
  }
  for (const u of plan.unsupported) {
    lines.push(`unsupported ${u.backend}: ${u.reason}`);
  }
  return lines.join('\n');
}
