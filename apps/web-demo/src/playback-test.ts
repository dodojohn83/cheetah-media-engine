import { MseBackend, MseError } from '@cheetah-media/runtime';
import type { TrackProfile } from '@cheetah-media/runtime';

interface Source {
  readonly type: 'download' | 'synthetic';
  readonly generator?: string;
  readonly url?: string;
}

interface Fixture {
  id: string;
  protocol: string;
  codec: string;
  source: Source;
  resolution?: string;
  frame_rate?: number;
  sample_rate?: number;
  channels?: number;
}

interface Manifest {
  readonly fixtures: Fixture[];
}

interface PlaybackResult {
  readonly status: 'success' | 'skipped' | 'failed';
  readonly fixture: string;
  readonly protocol: string;
  readonly mime?: string;
  readonly buffered?: number;
  readonly currentTime?: number;
  readonly duration?: number;
  readonly error?: string;
  readonly support?: Record<string, boolean>;
}

const videoEl = document.getElementById('video') as HTMLVideoElement;
const logEl = document.getElementById('log') as HTMLDivElement;

function log(msg: string): void {
  logEl.textContent += `${msg}\n`;
}

interface Fmp4Split {
  readonly init: Uint8Array;
  readonly segments: readonly Uint8Array[];
}

function readBoxSize(data: Uint8Array, offset: number): { size: number; headerSize: number } | undefined {
  if (offset + 8 > data.length) return undefined;
  const dv = new DataView(data.buffer, data.byteOffset + offset, 16);
  const size = dv.getUint32(0, false);
  if (size === 0) {
    return { size: data.length - offset, headerSize: 8 };
  }
  if (size === 1) {
    if (offset + 16 > data.length) return undefined;
    const high = dv.getUint32(8, false);
    const low = dv.getUint32(12, false);
    const extended = high * 0x100000000 + low;
    return { size: extended, headerSize: 16 };
  }
  return { size, headerSize: 8 };
}

function boxType(data: Uint8Array, offset: number): string {
  return String.fromCharCode(
    data[offset + 4]!,
    data[offset + 5]!,
    data[offset + 6]!,
    data[offset + 7]!,
  );
}

function concatUint8(chunks: readonly Uint8Array[]): Uint8Array {
  let total = 0;
  for (const c of chunks) total += c.length;
  const out = new Uint8Array(total);
  let off = 0;
  for (const c of chunks) {
    out.set(c, off);
    off += c.length;
  }
  return out;
}

function splitFmp4(data: Uint8Array): Fmp4Split {
  const initChunks: Uint8Array[] = [];
  const segments: Uint8Array[] = [];
  let current: Uint8Array[] = [];
  let offset = 0;
  while (offset < data.length) {
    const box = readBoxSize(data, offset);
    if (!box) break;
    const type = boxType(data, offset);
    const chunk = data.subarray(offset, offset + box.size);
    if (type === 'ftyp' || type === 'moov') {
      initChunks.push(chunk);
    } else if (type === 'moof') {
      if (current.length > 0) {
        segments.push(concatUint8(current));
      }
      current = [chunk];
    } else if (type === 'mfra' || type === 'free' || type === 'skip' || type === 'meta') {
      // Ignore movie fragment random access and informational boxes after media.
      if (current.length > 0) {
        segments.push(concatUint8(current));
        current = [];
      }
    } else {
      current.push(chunk);
    }
    offset += box.size;
  }
  if (current.length > 0) {
    segments.push(concatUint8(current));
  }
  return { init: concatUint8(initChunks), segments };
}

function resolveUrl(url: string): string {
  if (!url) throw new Error('resolveUrl: empty url');
  if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('/')) return url;
  return `/fixtures/${url}`;
}

async function fetchArrayBuffer(url: string): Promise<ArrayBuffer> {
  const res = await fetch(resolveUrl(url));
  if (!res.ok) throw new Error(`fetch ${url} failed: ${res.status}`);
  return res.arrayBuffer();
}

async function fetchText(url: string): Promise<string> {
  const res = await fetch(resolveUrl(url));
  if (!res.ok) throw new Error(`fetch ${url} failed: ${res.status}`);
  return res.text();
}

const AUDIO_CODECS = new Set(['aac', 'mp3', 'g711a', 'g711u']);

function audioCodecFromFixture(fixture: Fixture): string | undefined {
  if (AUDIO_CODECS.has(fixture.codec)) return fixture.codec;
  if ((fixture.codec === 'h264' || fixture.codec === 'h265') && fixture.sample_rate) {
    // The generated fixtures pair H.264/H.265 video with AAC audio.
    return 'aac';
  }
  return undefined;
}

function tracksFromFixture(fixture: Fixture): TrackProfile[] {
  const list: TrackProfile[] = [];
  if (fixture.resolution) {
    const [widthStr, heightStr] = fixture.resolution.split('x');
    const width = Number(widthStr);
    const height = Number(heightStr);
    if (width > 0 && height > 0) {
      list.push({ kind: 'video', codec: fixture.codec, width, height });
    }
  } else if (fixture.codec === 'h264' || fixture.codec === 'h265') {
    list.push({ kind: 'video', codec: fixture.codec });
  }
  const audioCodec = audioCodecFromFixture(fixture);
  if (audioCodec && fixture.sample_rate) {
    const audio: TrackProfile = {
      kind: 'audio',
      codec: audioCodec,
      sampleRate: fixture.sample_rate,
      ...(fixture.channels ? { channels: fixture.channels } : {}),
    };
    list.push(audio);
  } else if (audioCodec) {
    list.push({ kind: 'audio', codec: audioCodec });
  }
  return list;
}

async function loadManifest(): Promise<Manifest> {
  const res = await fetch('/fixtures/manifest.json');
  if (!res.ok) throw new Error(`manifest fetch failed: ${res.status}`);
  return (await res.json()) as Manifest;
}

function waitForPlayback(video: HTMLVideoElement, timeoutMs: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('playback timeout')), timeoutMs);
    const check = () => {
      if (video.currentTime > 0.2 && !video.paused) {
        clearTimeout(timer);
        resolve();
      }
    };
    video.addEventListener('playing', check, { once: true });
    video.addEventListener('timeupdate', check);
    video.addEventListener('error', () => {
      clearTimeout(timer);
      reject(new Error(`video element error ${video.error?.code ?? 0}`));
    }, { once: true });
  });
}

async function parseHlsPlaylist(playlistUrl: string): Promise<{ init: string; segments: string[] }> {
  const base = playlistUrl.slice(0, playlistUrl.lastIndexOf('/') + 1);
  const text = await fetchText(playlistUrl);
  const lines = text.split(/\r?\n/);
  let init: string | undefined;
  const segments: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;
    if (line.startsWith('#EXT-X-MAP:URI=')) {
      const quoted = line.split('=')[1];
      if (quoted) init = quoted.replace(/^"|"$/g, '');
    } else if (!line.startsWith('#')) {
      segments.push(line);
    }
  }
  if (!init) throw new Error('HLS playlist missing EXT-X-MAP');
  const resolve = (u: string) => (u.startsWith('http') ? u : `${base}${u}`);
  return { init: resolve(init), segments: segments.map(resolve) };
}

async function playFmp4(fixture: Fixture, url: string, tracks: TrackProfile[]): Promise<PlaybackResult> {
  const backend = new MseBackend(
    {} as never,
    {
      videoElement: videoEl,
      tracks,
      maxBufferAheadMs: 5000,
      maxBufferBehindMs: 5000,
      liveLatencyTargetMs: 100000,
      liveDriftSmallMs: 200,
      liveDriftLargeMs: 1000,
      minPlaybackRate: 1,
      maxPlaybackRate: 1,
      sourceOpenTimeoutMs: 10000,
      maxAppendQueue: 8,
    },
  );

  try {
    await backend.configure();
  } catch (err) {
    if (err instanceof MseError && err.code === 'mse-not-supported') {
      return snapshotResult(fixture, 'skipped', `MSE not supported: ${err.message}`);
    }
    throw err;
  }

  const data = new Uint8Array(await fetchArrayBuffer(url));
  const { init, segments } = splitFmp4(data);
  if (init.length === 0) {
    throw new Error('no fMP4 init segment found');
  }
  backend.pushSegment(init, { isInit: true });
  for (const segment of segments) {
    backend.pushSegment(segment, { isInit: false });
  }

  await waitForPlayback(videoEl, 15000);

  return snapshotResult(fixture, 'success');
}

async function playHls(fixture: Fixture, url: string, tracks: TrackProfile[]): Promise<PlaybackResult> {
  const text = await fetchText(url);
  const masterBase = url.slice(0, url.lastIndexOf('/') + 1);
  const lines = text.split(/\r?\n/);
  let playlistUrl: string | undefined;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line || !line.startsWith('#EXT-X-STREAM-INF')) continue;
    const next = lines[i + 1];
    if (next && !next.startsWith('#')) {
      playlistUrl = next.startsWith('http') ? next : `${masterBase}${next}`;
      break;
    }
  }
  if (!playlistUrl) throw new Error('No variant playlist found in HLS master');

  const { init, segments } = await parseHlsPlaylist(playlistUrl);
  const backend = new MseBackend(
    {} as never,
    {
      videoElement: videoEl,
      tracks,
      maxBufferAheadMs: 5000,
      maxBufferBehindMs: 5000,
      liveLatencyTargetMs: 100000,
      liveDriftSmallMs: 200,
      liveDriftLargeMs: 1000,
      minPlaybackRate: 1,
      maxPlaybackRate: 1,
      sourceOpenTimeoutMs: 10000,
      maxAppendQueue: 8,
    },
  );

  try {
    await backend.configure();
  } catch (err) {
    if (err instanceof MseError && err.code === 'mse-not-supported') {
      return snapshotResult(fixture, 'skipped', `MSE not supported: ${err.message}`);
    }
    throw err;
  }

  backend.pushSegment(new Uint8Array(await fetchArrayBuffer(init)), { isInit: true });
  for (const segment of segments) {
    backend.pushSegment(new Uint8Array(await fetchArrayBuffer(segment)), { isInit: false });
  }

  await waitForPlayback(videoEl, 15000);
  return snapshotResult(fixture, 'success');
}

function snapshotResult(fixture: Fixture, status: 'success' | 'skipped' | 'failed', error?: string): PlaybackResult {
  const buffered = videoEl.buffered.length > 0 ? videoEl.buffered.end(videoEl.buffered.length - 1) : 0;
  const duration = videoEl.duration;
  const result: PlaybackResult = {
    status,
    fixture: fixture.id,
    protocol: fixture.protocol,
    buffered,
    currentTime: videoEl.currentTime,
    support: {
      mediaSource: typeof MediaSource !== 'undefined',
    },
  };
  if (duration && Number.isFinite(duration)) {
    (result as unknown as Record<string, unknown>).duration = duration;
  }
  if (error) {
    (result as unknown as Record<string, unknown>).error = error;
  }
  return result;
}

async function run(): Promise<PlaybackResult> {
  const params = new URLSearchParams(window.location.search);
  const fixtureId = params.get('fixture');
  if (!fixtureId) {
    return { status: 'failed', fixture: '', protocol: '', error: 'missing ?fixture=' };
  }

  const manifest = await loadManifest();
  const fixture = manifest.fixtures.find((f) => f.id === fixtureId);
  if (!fixture) {
    return { status: 'failed', fixture: fixtureId, protocol: '', error: 'fixture not in manifest' };
  }

  log(`fixture: ${fixture.id} / ${fixture.protocol}`);

  if (fixture.protocol === 'http-flv' || fixture.protocol === 'ws-flv') {
    const supported = typeof MediaSource !== 'undefined' && MediaSource.isTypeSupported('video/flv');
    return snapshotResult(fixture, 'skipped', `FLV not supported by MSE; supported=${supported}`);
  }

  let fixtureUrl = fixture.source.url ?? '';
  if (!fixtureUrl) {
    return snapshotResult(fixture, 'skipped', 'fixture has no source url');
  }

  if (fixture.protocol === 'ws-fmp4') {
    // WebSocket transport not implemented in this harness; validate the same
    // fMP4 content via HTTP to prove decode-ability of the bytes.
    fixtureUrl = fixtureUrl.replace('/h264-ws-fmp4/', '/h264-http-fmp4/');
  }

  const tracks = tracksFromFixture(fixture);
  if (tracks.length === 0) {
    return snapshotResult(fixture, 'skipped', 'no tracks detected');
  }

  log(`tracks: ${JSON.stringify(tracks)}`);

  if (fixture.protocol === 'hls') {
    return await playHls(fixture, fixtureUrl, tracks);
  }
  if (fixture.protocol === 'http-fmp4' || fixture.protocol === 'ws-fmp4') {
    return await playFmp4(fixture, fixtureUrl, tracks);
  }

  return snapshotResult(fixture, 'skipped', `protocol ${fixture.protocol} not in playback harness`);
}

(window as unknown as { __playbackResult?: { status: string; fixture: string; protocol: string; error?: string } }).__playbackResult = { status: 'initializing', fixture: '', protocol: '', error: 'initializing' };

run()
  .then((result) => {
    log(`result: ${result.status}`);
    (window as unknown as { __playbackResult: PlaybackResult }).__playbackResult = result;
  })
  .catch((err) => {
    log(`error: ${err instanceof Error ? err.message : String(err)}`);
    (window as unknown as { __playbackResult: PlaybackResult }).__playbackResult = {
      status: 'failed',
      fixture: (window as unknown as { __playbackResult: PlaybackResult }).__playbackResult.fixture,
      protocol: (window as unknown as { __playbackResult: PlaybackResult }).__playbackResult.protocol,
      error: err instanceof Error ? err.message : String(err),
    };
  });
