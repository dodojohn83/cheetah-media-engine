import { describe, expect, it } from 'vitest';
import { detectProtocol, PlaybackSession, protocolSupportedByMseSession } from './playback-session';

describe('playback session protocol detection', () => {
  it('detects HLS playlists', () => {
    expect(detectProtocol('https://cdn.example/live/index.m3u8')).toBe('hls');
    expect(detectProtocol('https://cdn.example/ll-hls/index.m3u8')).toBe('ll-hls');
  });

  it('detects fMP4 and FLV by extension and scheme', () => {
    expect(detectProtocol('https://cdn.example/a.mp4')).toBe('http-fmp4');
    expect(detectProtocol('wss://cdn.example/a.mp4')).toBe('ws-fmp4');
    expect(detectProtocol('https://cdn.example/a.flv')).toBe('http-flv');
    expect(detectProtocol('ws://cdn.example/a.flv')).toBe('ws-flv');
  });

  it('honors explicit protocol hints', () => {
    expect(detectProtocol('https://cdn.example/stream', 'http-fmp4')).toBe('http-fmp4');
  });

  it('rejects invalid detectProtocol inputs', () => {
    expect(() => detectProtocol(null as unknown as string)).toThrow('detectProtocol url must be a string');
    expect(() => detectProtocol('https://cdn.example/stream', 'rtmp' as unknown as 'http-fmp4')).toThrow(
      "detectProtocol hint must be a valid protocol or 'auto'",
    );
  });

  it('marks MSE-native and FLV-transmux protocols as supported by the session', () => {
    expect(protocolSupportedByMseSession('http-fmp4')).toBe(true);
    expect(protocolSupportedByMseSession('hls')).toBe(true);
    expect(protocolSupportedByMseSession('http-flv')).toBe(true);
    expect(protocolSupportedByMseSession('ws-flv')).toBe(true);
    expect(protocolSupportedByMseSession('ws-annexb')).toBe(false);
  });
});

describe('PlaybackSession constructor validation', () => {
  function makeElement(): HTMLVideoElement {
    return {
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      play: () => undefined,
      pause: () => undefined,
      load: () => undefined,
      srcObject: null,
      src: '',
      currentTime: 0,
      playbackRate: 1,
      paused: true,
      ended: false,
      readyState: 0,
      buffered: { length: 0 },
      duration: NaN,
      error: null,
      videoWidth: 0,
      videoHeight: 0,
    } as unknown as HTMLVideoElement;
  }

  it('rejects non-object options', () => {
    expect(() => new PlaybackSession(null as unknown as { videoElement: HTMLVideoElement; url: string; protocol: 'http-fmp4' })).toThrow('must be an object');
  });

  it('rejects non-object videoElement', () => {
    expect(() => new PlaybackSession({ videoElement: 42, url: 'https://x/a.mp4', protocol: 'http-fmp4' } as any)).toThrow('object videoElement');
  });

  it('rejects non-boolean isLive', () => {
    expect(() => new PlaybackSession({ videoElement: makeElement(), url: 'https://x/a.mp4', protocol: 'http-fmp4', isLive: 'false' } as any)).toThrow('isLive must be a boolean');
  });

  it('rejects invalid headers', () => {
    expect(() => new PlaybackSession({ videoElement: makeElement(), url: 'https://x/a.mp4', protocol: 'http-fmp4', headers: { auth: 1 } } as any)).toThrow('header value');
  });

  it('rejects non-function onEvent', () => {
    expect(() => new PlaybackSession({ videoElement: makeElement(), url: 'https://x/a.mp4', protocol: 'http-fmp4', onEvent: 'noop' } as any)).toThrow('onEvent');
  });

  it('rejects invalid numeric latency and playback rate options', () => {
    const base = { videoElement: makeElement(), url: 'https://x/a.mp4', protocol: 'http-fmp4' } as any;
    expect(() => new PlaybackSession({ ...base, softLatencyMs: NaN })).toThrow('softLatencyMs');
    expect(() => new PlaybackSession({ ...base, hardLatencyMs: -1 })).toThrow('hardLatencyMs');
    expect(() => new PlaybackSession({ ...base, maxPlaybackRate: 0 })).toThrow('maxPlaybackRate');
    expect(() => new PlaybackSession({ ...base, maxPlaybackRate: Infinity })).toThrow('maxPlaybackRate');
  });
});

describe('PlaybackSession method validation', () => {
  function makeElement(): HTMLVideoElement {
    return {
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      play: () => undefined,
      pause: () => undefined,
      load: () => undefined,
      srcObject: null,
      src: '',
      currentTime: 0,
      playbackRate: 1,
      paused: true,
      ended: false,
      readyState: 0,
      buffered: { length: 0 },
      duration: NaN,
      error: null,
      videoWidth: 0,
      videoHeight: 0,
    } as unknown as HTMLVideoElement;
  }

  function makeSession(): PlaybackSession {
    return new PlaybackSession({
      videoElement: makeElement(),
      url: 'https://x/a.mp4',
      protocol: 'http-fmp4',
    });
  }

  it('rejects invalid seek timeMs', async () => {
    const session = makeSession();
    (session as any).mse = { seek: () => {} };
    await expect(session.seek(NaN)).rejects.toThrow('seek timeMs');
    await expect(session.seek(-1)).rejects.toThrow('seek timeMs');
    await expect(session.seek(Infinity)).rejects.toThrow('seek timeMs');
  });

  it('rejects invalid playback rate', async () => {
    const session = makeSession();
    (session as any).mse = { setPlaybackRate: () => {} };
    await expect(session.setPlaybackRate(0)).rejects.toThrow('playback rate');
    await expect(session.setPlaybackRate(20)).rejects.toThrow('playback rate');
    await expect(session.setPlaybackRate(NaN)).rejects.toThrow('playback rate');
  });

  it('rejects invalid frameStep arguments', async () => {
    const session = makeSession();
    (session as any).mse = { frameStep: () => {} };
    await expect(session.frameStep('up' as 'forward')).rejects.toThrow('frameStep direction');
    await expect(session.frameStep('forward', 'yes' as unknown as boolean)).rejects.toThrow('frameStep keyframeOnly');
  });

  it('rejects invalid pauseDisplay keepConnection', async () => {
    const session = makeSession();
    (session as any).mse = { pauseDisplay: () => {} };
    await expect(session.pauseDisplay('yes' as unknown as boolean)).rejects.toThrow('pauseDisplay keepConnection');
  });
});
