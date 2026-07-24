/**
 * Main-thread playback session that wires transport → (fMP4 split / HLS) → MSE.
 *
 * This is the integration spine missing from the public SDK: the WASM WebEngine
 * shell only stores control state, while MSE/WebCodecs backends lived as
 * isolated unit-test islands. The session fills that gap for required Web v1
 * paths that MSE can consume natively (HTTP/WS-fMP4 and HLS fMP4).
 */

import { createTransport, type Transport, type TransportError } from './transport';
import { validateHeaders } from './transport-common';
import { MseBackend, type HTMLVideoElementLike, type MseMetrics } from './mse';
import type { Protocol, TrackProfile } from './planner';
import { Fmp4BoxAccumulator, Fmp4SegmentBuilder, splitFmp4 } from './fmp4';
import { FlvFmp4TransmuxerJs, type FlvTrackInfo } from './flv-transmux';
import { TsFmp4TransmuxerJs, type TsTrackInfo } from './ts-transmux';

export type PlaybackSessionEvent =
  | { type: 'state'; state: 'loading' | 'preroll' | 'playing' | 'paused' | 'rebuffering' | 'ended' | 'failed' }
  | { type: 'tracks'; tracks: readonly TrackProfile[] }
  | { type: 'backend'; backend: 'mse' }
  | { type: 'firstframe' }
  | { type: 'error'; code: number; stage: string; message: string; recoverable: boolean }
  | { type: 'stats'; metrics: MseMetrics; networkBytes: number };

export interface PlaybackSessionOptions {
  readonly videoElement: HTMLVideoElementLike;
  readonly url: string;
  readonly protocol: Protocol;
  readonly isLive?: boolean;
  readonly tracks?: readonly TrackProfile[];
  readonly headers?: Record<string, string>;
  readonly softLatencyMs?: number;
  readonly hardLatencyMs?: number;
  readonly maxPlaybackRate?: number;
  /** Network request timeout for playlist/segment fetches and transports (ms). */
  readonly timeoutMs?: number;
  readonly onEvent?: (event: PlaybackSessionEvent) => void;
}

const DEFAULT_TRACKS: readonly TrackProfile[] = [
  { kind: 'video', codec: 'h264' },
  { kind: 'audio', codec: 'aac' },
];

function resolveMediaUrl(url: string): string {
  try {
    return new URL(url, typeof location !== 'undefined' ? location.href : 'http://localhost/').href;
  } catch {
    return url;
  }
}

const VALID_PROTOCOLS: readonly string[] = [
  'http-flv',
  'ws-flv',
  'http-fmp4',
  'ws-fmp4',
  'http-annexb',
  'ws-annexb',
  'http-mpegps',
  'ws-mpegps',
  'webtransport',
  'webrtc',
  'hls',
  'll-hls',
];

export function detectProtocol(url: string, hint?: Protocol | 'auto'): Protocol {
  if (typeof url !== 'string') {
    throw new Error('detectProtocol url must be a string');
  }
  if (hint !== undefined && hint !== 'auto' && !VALID_PROTOCOLS.includes(hint)) {
    throw new Error(`detectProtocol hint must be a valid protocol or 'auto'`);
  }
  if (hint && hint !== 'auto') {
    return hint as Protocol;
  }
  let parsed: URL | undefined;
  try {
    parsed = new URL(url, typeof location !== 'undefined' ? location.href : 'http://localhost/');
  } catch {
    parsed = undefined;
  }
  const path = (parsed?.pathname ?? url).toLowerCase();
  const full = url.toLowerCase();
  const isWs = parsed?.protocol === 'ws:' || parsed?.protocol === 'wss:' || full.startsWith('ws://') || full.startsWith('wss://');

  if (path.endsWith('.m3u8') || full.includes('.m3u8')) {
    return full.includes('ll-') || full.includes('lowlatency') ? 'll-hls' : 'hls';
  }
  if (path.endsWith('.flv') || full.includes('.flv')) {
    return isWs ? 'ws-flv' : 'http-flv';
  }
  if (path.endsWith('.ts') || path.endsWith('.m2ts')) {
    return isWs ? 'ws-fmp4' : 'http-fmp4'; // not true fmp4; caller may reject
  }
  if (
    path.endsWith('.mp4') ||
    path.endsWith('.m4s') ||
    path.endsWith('.m4v') ||
    path.endsWith('.m4a') ||
    full.includes('fmp4') ||
    full.includes('fragmented')
  ) {
    return isWs ? 'ws-fmp4' : 'http-fmp4';
  }
  // Default live IP-camera style URLs to HTTP-FLV when no extension is present.
  if (isWs) return 'ws-flv';
  return 'http-flv';
}

export function protocolSupportedByMseSession(protocol: Protocol): boolean {
  switch (protocol) {
    case 'http-fmp4':
    case 'ws-fmp4':
    case 'hls':
    case 'll-hls':
    case 'http-flv':
    case 'ws-flv':
      return true;
    default:
      return false;
  }
}

export class PlaybackSession {
  private readonly options: PlaybackSessionOptions;
  private readonly tracks: readonly TrackProfile[];
  private readonly url: string;
  private transport: Transport | undefined;
  private mse: MseBackend | undefined;
  private stopped = false;
  private started = false;
  private starting = false;
  private networkBytes = 0;
  private firstFrameEmitted = false;
  private wantPlay = false;
  private generation = 0;
  private statsTimer: ReturnType<typeof setInterval> | undefined;
  private stopController: AbortController | undefined;
  private videoEventController: AbortController | undefined;

  constructor(options: PlaybackSessionOptions) {
    if (!options || typeof options !== 'object') {
      throw new Error('PlaybackSession options must be an object');
    }
    if (typeof options.videoElement !== 'object' || options.videoElement === null) {
      throw new Error('PlaybackSession requires an object videoElement');
    }
    if (typeof options.url !== 'string' || options.url.length === 0) {
      throw new Error('PlaybackSession requires a non-empty url');
    }
    if (options.isLive !== undefined && typeof options.isLive !== 'boolean') {
      throw new Error('PlaybackSession isLive must be a boolean');
    }
    if (options.headers !== undefined) {
      const headersError = validateHeaders(options.headers);
      if (headersError) {
        throw new Error(`PlaybackSession headers: ${headersError}`);
      }
    }
    if (options.onEvent !== undefined && typeof options.onEvent !== 'function') {
      throw new Error('PlaybackSession onEvent must be a function');
    }
    if (options.timeoutMs !== undefined) {
      if (
        (options.timeoutMs !== Infinity && !Number.isFinite(options.timeoutMs)) ||
        options.timeoutMs <= 0
      ) {
        throw new Error('PlaybackSession timeoutMs must be Infinity or a finite positive number');
      }
    }
    if (options.softLatencyMs !== undefined) {
      if (!Number.isFinite(options.softLatencyMs) || options.softLatencyMs <= 0) {
        throw new Error('PlaybackSession softLatencyMs must be a finite positive number');
      }
    }
    if (options.hardLatencyMs !== undefined) {
      if (!Number.isFinite(options.hardLatencyMs) || options.hardLatencyMs <= 0) {
        throw new Error('PlaybackSession hardLatencyMs must be a finite positive number');
      }
    }
    if (options.maxPlaybackRate !== undefined) {
      if (!Number.isFinite(options.maxPlaybackRate) || options.maxPlaybackRate <= 0) {
        throw new Error('PlaybackSession maxPlaybackRate must be a finite positive number');
      }
    }
    this.options = options;
    this.url = resolveMediaUrl(options.url);
    this.tracks = options.tracks && options.tracks.length > 0 ? options.tracks : DEFAULT_TRACKS;
  }

  get active(): boolean {
    return this.started && !this.stopped;
  }

  get metrics(): MseMetrics | undefined {
    return this.mse?.metrics;
  }

  get networkByteCount(): number {
    return this.networkBytes;
  }

  async start(): Promise<void> {
    if (this.started || this.starting) return;
    if (this.stopped) {
      throw new Error('PlaybackSession already stopped');
    }
    this.starting = true;
    const protocol = this.options.protocol;
    if (!protocolSupportedByMseSession(protocol)) {
      this.starting = false;
      const message = `Protocol ${protocol} is not supported by the main-thread MSE session`;
      this.emit({
        type: 'error',
        code: 6003,
        stage: 'source',
        message,
        recoverable: false,
      });
      this.emit({ type: 'state', state: 'failed' });
      throw new Error(message);
    }

    this.generation += 1;
    this.stopController = new AbortController();
    const gen = this.generation;
    this.emit({ type: 'state', state: 'loading' });

    // FLV starts with provisional tracks; tracks are refined after sequence headers.
    let sessionTracks = this.tracks;
    if (protocol === 'http-flv' || protocol === 'ws-flv') {
      sessionTracks = [{ kind: 'video', codec: 'h264' }, { kind: 'audio', codec: 'aac' }];
    }

    try {
      this.mse = new MseBackend(
        { candidate: { rank: 0, videoBackend: 'mse', audioBackend: 'mse', renderer: undefined, transport: 'fetch', reason: 'playback-session', isLive: this.options.isLive ?? true }, reason: 'session-start' },
        {
          videoElement: this.options.videoElement,
          tracks: sessionTracks,
          isLive: this.options.isLive ?? true,
          liveLatencyTargetMs: this.options.softLatencyMs ?? 1000,
          maxPlaybackRate: this.options.maxPlaybackRate ?? 1.05,
          callbacks: {
            onError: (err) => {
              if (this.generation !== gen || this.stopped) return;
              this.emit({
                type: 'error',
                code: 6200,
                stage: 'mse',
                message: err.message,
                recoverable: true,
              });
              this.emit({ type: 'state', state: 'failed' });
            },
          },
        },
      );
      await this.mse.configure();
    } catch (err) {
      this.starting = false;
      const message = err instanceof Error ? err.message : String(err);
      this.emit({ type: 'error', code: 6200, stage: 'mse', message, recoverable: false });
      this.emit({ type: 'state', state: 'failed' });
      throw err;
    }
    this.started = true;
    this.starting = false;

    if (this.generation !== gen || this.stopped) return;

    this.emit({ type: 'tracks', tracks: sessionTracks });
    this.emit({ type: 'backend', backend: 'mse' });
    this.emit({ type: 'state', state: 'preroll' });
    this.bindVideoEvents(gen);
    this.startStats(gen);

    if (protocol === 'hls' || protocol === 'll-hls') {
      try {
        await this.runHls(gen);
      } catch (err) {
        if (this.stopped) return;
        const message = err instanceof Error ? err.message : String(err);
        this.emit({ type: 'error', code: 6100, stage: 'hls', message, recoverable: false });
        this.emit({ type: 'state', state: 'failed' });
        throw err instanceof Error ? err : new Error(message);
      }
    } else if (protocol === 'http-flv' || protocol === 'ws-flv') {
      await this.runFlvStream(gen);
    } else {
      await this.runFmp4Stream(gen);
    }
  }

  play(): void {
    this.wantPlay = true;
    const video = this.options.videoElement;
    try {
      const p = video.play?.();
      if (p && typeof (p as Promise<void>).catch === 'function') {
        void (p as Promise<void>).catch(() => {
          // Autoplay policies may reject; UI can retry after a user gesture.
        });
      }
    } catch {
      // ignore
    }
    if (!this.stopped) {
      this.emit({ type: 'state', state: 'playing' });
    }
  }

  pause(): void {
    this.wantPlay = false;
    try {
      this.options.videoElement.pause?.();
    } catch {
      // ignore
    }
    if (!this.stopped) {
      this.emit({ type: 'state', state: 'paused' });
    }
  }

  async seek(timeMs: number): Promise<void> {
    if (!this.mse) throw new Error('Session not started');
    if (!Number.isFinite(timeMs) || timeMs < 0) {
      throw new Error('seek timeMs must be a finite non-negative number');
    }
    await this.mse.seek(timeMs);
  }

  async setPlaybackRate(rate: number): Promise<void> {
    if (!this.mse) throw new Error('Session not started');
    if (!Number.isFinite(rate) || rate < 0.1 || rate > 16) {
      throw new Error('playback rate must be between 0.1 and 16');
    }
    await this.mse.setPlaybackRate?.(rate);
  }

  async frameStep(direction: 'forward' | 'backward', keyframeOnly = false): Promise<void> {
    if (!this.mse) throw new Error('Session not started');
    if (direction !== 'forward' && direction !== 'backward') {
      throw new Error('frameStep direction must be forward or backward');
    }
    if (typeof keyframeOnly !== 'boolean') {
      throw new Error('frameStep keyframeOnly must be a boolean');
    }
    await this.mse.frameStep?.(direction, keyframeOnly);
  }

  async pauseDisplay(keepConnection = true): Promise<void> {
    if (!this.mse) throw new Error('Session not started');
    if (typeof keepConnection !== 'boolean') {
      throw new Error('pauseDisplay keepConnection must be a boolean');
    }
    await this.mse.pauseDisplay?.(keepConnection);
  }

  async stop(): Promise<void> {
    if (this.stopped) return;
    this.stopped = true;
    this.generation += 1;
    this.stopStats();
    this.stopController?.abort();
    this.videoEventController?.abort();
    this.transport?.stop();
    this.transport = undefined;
    if (this.mse) {
      await this.mse.stop();
      this.mse = undefined;
    }
  }

  private emit(event: PlaybackSessionEvent): void {
    try {
      this.options.onEvent?.(event);
    } catch {
      // User handlers must not break the session.
    }
  }

  private bindVideoEvents(gen: number): void {
    const video = this.options.videoElement;
    this.videoEventController?.abort();
    this.videoEventController = new AbortController();
    const signal = this.videoEventController.signal;
    const onPlaying = () => {
      if (this.generation !== gen || this.stopped) return;
      if (!this.firstFrameEmitted) {
        this.firstFrameEmitted = true;
        this.emit({ type: 'firstframe' });
      }
      this.emit({ type: 'state', state: 'playing' });
    };
    const onWaiting = () => {
      if (this.generation !== gen || this.stopped) return;
      this.emit({ type: 'state', state: 'rebuffering' });
    };
    const onPause = () => {
      if (this.generation !== gen || this.stopped || this.wantPlay) return;
      this.emit({ type: 'state', state: 'paused' });
    };
    const onEnded = () => {
      if (this.generation !== gen || this.stopped) return;
      this.emit({ type: 'state', state: 'ended' });
    };
    video.addEventListener('playing', onPlaying, { signal });
    video.addEventListener('waiting', onWaiting, { signal });
    video.addEventListener('pause', onPause, { signal });
    video.addEventListener('ended', onEnded, { signal });
  }

  private startStats(gen: number): void {
    this.stopStats();
    this.statsTimer = setInterval(() => {
      if (this.generation !== gen || this.stopped || !this.mse) return;
      this.emit({
        type: 'stats',
        metrics: this.mse.metrics,
        networkBytes: this.networkBytes,
      });
    }, 500);
  }

  private stopStats(): void {
    if (this.statsTimer !== undefined) {
      clearInterval(this.statsTimer);
      this.statsTimer = undefined;
    }
  }

  private async runFlvStream(gen: number): Promise<void> {
    const mode = this.options.protocol === 'ws-flv' ? 'websocket' : 'fetch';
    const transmuxer = new FlvFmp4TransmuxerJs();
    let tracksEmitted = false;

    const pushSegments = (): void => {
      if (!this.mse || this.generation !== gen || this.stopped) return;
      const segs = transmuxer.poll();
      if (!tracksEmitted && transmuxer.getTracks().length > 0) {
        tracksEmitted = true;
        this.emit({ type: 'tracks', tracks: flvTracksToProfiles(transmuxer.getTracks()) });
      }
      for (const seg of segs) {
        if (seg.init && seg.init.length > 0) {
          this.mse.pushSegment(seg.init, { isInit: true });
        }
        if (seg.media && seg.media.length > 0) {
          this.mse.pushSegment(seg.media, { isInit: false });
        }
      }
      if (this.wantPlay) this.play();
    };

    await new Promise<void>((resolve, reject) => {
      if (this.generation !== gen || this.stopped) {
        resolve();
        return;
      }
      this.transport = createTransport(
        {
          url: this.url,
          maxRetries: this.options.isLive ? 3 : 0,
          timeoutMs: this.options.timeoutMs ?? 30_000,
          ...(this.options.headers ? { headers: this.options.headers } : {}),
        },
        mode,
      );

      this.transport.start(
        (chunk) => {
          if (this.generation !== gen || this.stopped) return;
          this.networkBytes += chunk.bytes.length;
          try {
            transmuxer.push(chunk.bytes);
            pushSegments();
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            this.emit({ type: 'error', code: 6100, stage: 'demux', message, recoverable: false });
            this.emit({ type: 'state', state: 'failed' });
            reject(err instanceof Error ? err : new Error(message));
          }
        },
        (error: TransportError) => {
          if (this.generation !== gen || this.stopped) return;
          this.emit({
            type: 'error',
            code: error.code,
            stage: 'transport',
            message: error.message,
            recoverable: error.retryable,
          });
          if (!error.retryable) {
            this.emit({ type: 'state', state: 'failed' });
            reject(new Error(error.message));
          }
        },
        () => {
          if (this.generation !== gen || this.stopped) {
            resolve();
            return;
          }
          try {
            transmuxer.finish();
            pushSegments();
          } catch {
            // ignore flush errors on teardown
          }
          resolve();
        },
      );
    });
  }

  private async runFmp4Stream(gen: number): Promise<void> {
    const mode =
      this.options.protocol === 'ws-fmp4' ? 'websocket' : 'fetch';
    const accumulator = new Fmp4BoxAccumulator();
    const builder = new Fmp4SegmentBuilder();

    await new Promise<void>((resolve, reject) => {
      if (this.generation !== gen || this.stopped) {
        resolve();
        return;
      }
      this.transport = createTransport(
        {
          url: this.url,
          maxRetries: this.options.isLive ? 3 : 0,
          timeoutMs: this.options.timeoutMs ?? 30_000,
          ...(this.options.headers ? { headers: this.options.headers } : {}),
        },
        mode,
      );

      this.transport.start(
        (chunk) => {
          if (this.generation !== gen || this.stopped || !this.mse) return;
          this.networkBytes += chunk.bytes.length;
          try {
            accumulator.push(chunk.bytes);
            const boxes = accumulator.takeCompleteBoxes();
            for (const box of boxes) {
              // Copy box data; subarrays may alias the accumulator buffer.
              const owned = box.slice();
              const segs = builder.feed(owned);
              for (const seg of segs) {
                this.mse.pushSegment(seg.data, { isInit: seg.isInit });
              }
            }
            if (builder.hasInit && this.wantPlay) {
              this.play();
            }
          } catch (err) {
            const message = err instanceof Error ? err.message : String(err);
            this.emit({ type: 'error', code: 6100, stage: 'demux', message, recoverable: false });
            this.emit({ type: 'state', state: 'failed' });
            reject(err instanceof Error ? err : new Error(message));
          }
        },
        (error: TransportError) => {
          if (this.generation !== gen || this.stopped) return;
          this.emit({
            type: 'error',
            code: error.code,
            stage: 'transport',
            message: error.message,
            recoverable: error.retryable,
          });
          if (!error.retryable) {
            this.emit({ type: 'state', state: 'failed' });
            reject(new Error(error.message));
          }
        },
        () => {
          if (this.generation !== gen || this.stopped || !this.mse) {
            resolve();
            return;
          }
          try {
            for (const seg of builder.flush()) {
              this.mse.pushSegment(seg.data, { isInit: seg.isInit });
            }
            if (this.wantPlay) this.play();
          } catch {
            // ignore flush errors on teardown
          }
          resolve();
        },
      );
    });
  }

  private async runHls(gen: number): Promise<void> {
    const playlistText = await this.fetchText(this.url);
    if (this.generation !== gen || this.stopped) return;

    if (/#EXT-X-KEY/i.test(playlistText)) {
      const message = 'Encrypted HLS (#EXT-X-KEY) is not supported in Web v1 MSE session';
      this.emit({ type: 'error', code: 6003, stage: 'hls', message, recoverable: false });
      this.emit({ type: 'state', state: 'failed' });
      throw new Error(message);
    }

    // Master playlist: pick the first media variant.
    if (/#EXT-X-STREAM-INF/i.test(playlistText)) {
      const variant = pickFirstVariant(playlistText, this.url);
      if (!variant) {
        throw new Error('HLS master playlist has no variants');
      }
      const mediaText = await this.fetchText(variant);
      if (this.generation !== gen || this.stopped) return;
      await this.playHlsMediaPlaylist(gen, mediaText, variant);
      return;
    }

    await this.playHlsMediaPlaylist(gen, playlistText, this.url);
  }

  private async playHlsMediaPlaylist(gen: number, text: string, baseUrl: string): Promise<void> {
    const { mapUri, segments } = parseMediaPlaylist(text, baseUrl);
    if (segments.length === 0) {
      throw new Error('HLS media playlist has no segments');
    }

    // Detect TS vs fMP4 by map or extension.
    const sample = mapUri ?? segments[0]!.uri;
    const isTs = /\.ts($|\?)/i.test(sample) || /\.m2ts($|\?)/i.test(sample);

    if (isTs) {
      await this.playHlsTsSegments(gen, segments);
      return;
    }

    if (mapUri) {
      const init = await this.fetchBytes(mapUri);
      if (this.generation !== gen || this.stopped || !this.mse) return;
      this.networkBytes += init.length;
      this.mse.pushSegment(init, { isInit: true });
    }

    for (const segment of segments) {
      if (this.generation !== gen || this.stopped || !this.mse) return;
      const data = await this.fetchBytes(segment.uri);
      if (this.generation !== gen || this.stopped || !this.mse) return;
      this.networkBytes += data.length;
      // If no MAP, first segment may include init boxes.
      if (!mapUri && segment === segments[0]) {
        const { init, segments: frags } = splitFmp4(data);
        if (init.length > 0) {
          this.mse.pushSegment(init, { isInit: true });
        }
        for (const frag of frags) {
          this.mse.pushSegment(frag, { isInit: false });
        }
      } else {
        this.mse.pushSegment(data, { isInit: false });
      }
      if (this.wantPlay) this.play();
    }
  }

  private async playHlsTsSegments(
    gen: number,
    segments: readonly HlsSegment[],
  ): Promise<void> {
    // Keep one transmuxer across segments so init/config is stable for live HLS.
    const transmuxer = new TsFmp4TransmuxerJs();
    let tracksEmitted = false;

    for (const segment of segments) {
      if (this.generation !== gen || this.stopped || !this.mse) return;
      const data = await this.fetchBytes(segment.uri);
      if (this.generation !== gen || this.stopped || !this.mse) return;
      this.networkBytes += data.length;
      try {
        transmuxer.push(data);
        // End-of-segment: finalize open PES assemblies for this fragment.
        transmuxer.finish();
        if (!tracksEmitted && transmuxer.getTracks().length > 0) {
          tracksEmitted = true;
          this.emit({ type: 'tracks', tracks: tsTracksToProfiles(transmuxer.getTracks()) });
        }
        for (const seg of transmuxer.poll()) {
          if (seg.init && seg.init.length > 0) {
            this.mse.pushSegment(seg.init, { isInit: true });
          }
          if (seg.media && seg.media.length > 0) {
            this.mse.pushSegment(seg.media, { isInit: false });
          }
        }
        if (this.wantPlay) this.play();
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        this.emit({ type: 'error', code: 6100, stage: 'demux', message, recoverable: false });
        this.emit({ type: 'state', state: 'failed' });
        throw err instanceof Error ? err : new Error(message);
      }
    }
  }

  private async fetchWithTimeout(url: string, init: RequestInit): Promise<Response> {
    const timeoutMs = this.options.timeoutMs ?? 30_000;
    const controller = new AbortController();
    const timer =
      timeoutMs > 0 && timeoutMs !== Infinity
        ? setTimeout(() => controller.abort(), timeoutMs)
        : undefined;

    let onStop: (() => void) | undefined;
    if (this.stopController) {
      onStop = () => controller.abort();
      this.stopController.signal.addEventListener('abort', onStop, { once: true });
    }

    try {
      return await fetch(url, {
        ...init,
        signal: controller.signal,
      });
    } finally {
      if (timer) clearTimeout(timer);
      if (onStop && this.stopController) {
        this.stopController.signal.removeEventListener('abort', onStop);
      }
    }
  }

  private async fetchText(url: string): Promise<string> {
    const res = await this.fetchWithTimeout(url, {
      credentials: 'same-origin',
      ...(this.options.headers ? { headers: this.options.headers } : {}),
    });
    if (!res.ok) {
      throw new Error(`HLS fetch failed ${res.status} for ${url}`);
    }
    return res.text();
  }

  private async fetchBytes(url: string): Promise<Uint8Array> {
    const res = await this.fetchWithTimeout(url, {
      credentials: 'same-origin',
      ...(this.options.headers ? { headers: this.options.headers } : {}),
    });
    if (!res.ok) {
      throw new Error(`Segment fetch failed ${res.status} for ${url}`);
    }
    return new Uint8Array(await res.arrayBuffer());
  }
}

function flvTracksToProfiles(tracks: readonly FlvTrackInfo[]): TrackProfile[] {
  return tracks.map((t) => {
    if (t.kind === 'video') {
      return {
        kind: 'video' as const,
        codec: t.codec === 'h265' ? 'h265' : 'h264',
        ...(t.width ? { width: t.width } : {}),
        ...(t.height ? { height: t.height } : {}),
      };
    }
    return {
      kind: 'audio' as const,
      codec: 'aac',
      ...(t.sampleRate ? { sampleRate: t.sampleRate } : {}),
      ...(t.channels ? { channels: t.channels } : {}),
    };
  });
}

function tsTracksToProfiles(tracks: readonly TsTrackInfo[]): TrackProfile[] {
  return tracks.map((t) => {
    if (t.kind === 'video') {
      return {
        kind: 'video' as const,
        codec: 'h264',
        ...(t.width ? { width: t.width } : {}),
        ...(t.height ? { height: t.height } : {}),
      };
    }
    return {
      kind: 'audio' as const,
      codec: 'aac',
      ...(t.sampleRate ? { sampleRate: t.sampleRate } : {}),
      ...(t.channels ? { channels: t.channels } : {}),
    };
  });
}

function resolveRelative(base: string, ref: string): string {
  try {
    return new URL(ref, base).href;
  } catch {
    return ref;
  }
}

function pickFirstVariant(master: string, baseUrl: string): string | undefined {
  const lines = master.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!.trim();
    if (line.startsWith('#EXT-X-STREAM-INF')) {
      const next = lines[i + 1]?.trim();
      if (next && !next.startsWith('#')) {
        return resolveRelative(baseUrl, next);
      }
    }
  }
  return undefined;
}

interface HlsSegment {
  readonly uri: string;
  readonly durationSec: number;
}

function parseMediaPlaylist(
  text: string,
  baseUrl: string,
): { mapUri?: string; segments: HlsSegment[] } {
  const lines = text.split(/\r?\n/);
  let mapUri: string | undefined;
  const segments: HlsSegment[] = [];
  let nextDuration = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!.trim();
    if (line.startsWith('#EXT-X-MAP:')) {
      const m = /URI="([^"]+)"/i.exec(line);
      if (m?.[1]) {
        mapUri = resolveRelative(baseUrl, m[1]);
      }
      continue;
    }
    if (line.startsWith('#EXTINF:')) {
      const n = Number.parseFloat(line.slice('#EXTINF:'.length));
      nextDuration = Number.isFinite(n) ? n : 0;
      continue;
    }
    if (!line || line.startsWith('#')) continue;
    segments.push({ uri: resolveRelative(baseUrl, line), durationSec: nextDuration });
    nextDuration = 0;
  }
  return mapUri !== undefined ? { mapUri, segments } : { segments };
}
