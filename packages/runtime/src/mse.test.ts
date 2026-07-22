import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { MseBackend, mseBackendFactory, type MseError, type HTMLVideoElementLike, type MseBackendOptions } from './mse';
import type { BackendContext, MediaBackend } from './fallback';
import type { TrackProfile } from './planner';

const videoTrack: TrackProfile = { kind: 'video', codec: 'h264', width: 640, height: 360 };
const audioTrack: TrackProfile = { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 };

const ctx: BackendContext = {
  candidate: {
    rank: 1,
    videoBackend: 'mse',
    audioBackend: 'mse',
    renderer: undefined,
    transport: 'fetch',
    reason: 'mse fallback',
    isLive: true,
  },
  reason: 'test',
};

class MockEventTarget {
  private listeners = new Map<string, Set<(event?: unknown) => void>>();

  addEventListener(type: string, listener: (event?: unknown) => void): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener);
  }

  removeEventListener(type: string, listener: (event?: unknown) => void): void {
    this.listeners.get(type)?.delete(listener);
  }

  dispatchEvent(type: string, event?: unknown): void {
    const listeners = this.listeners.get(type);
    if (listeners) {
      for (const listener of listeners) {
        listener(event);
      }
    }
  }
}

class MockTimeRanges {
  private ranges: [number, number][];

  constructor(ranges: [number, number][]) {
    this.ranges = ranges;
  }

  get length(): number {
    return this.ranges.length;
  }

  start(index: number): number {
    if (index < 0 || index >= this.ranges.length) throw new RangeError('index out of range');
    return this.ranges[index]![0];
  }

  end(index: number): number {
    if (index < 0 || index >= this.ranges.length) throw new RangeError('index out of range');
    return this.ranges[index]![1];
  }
}

class MockSourceBuffer extends MockEventTarget {
  updating = false;
  timestampOffset = 0;
  appendWindowStart = 0;
  appendWindowEnd = Infinity;
  buffered: MockTimeRanges = new MockTimeRanges([]);
  appended: Uint8Array[] = [];
  removed: [number, number][] = [];
  changeTypeCalls: string[] = [];
  private throwOnce: Error | undefined = undefined;
  private throwQuotaCount = 0;

  setBuffered(ranges: [number, number][]): void {
    this.buffered = new MockTimeRanges(ranges);
  }

  setThrowOnce(error: Error): void {
    this.throwOnce = error;
  }

  setThrowQuotaOnce(count = 1): void {
    this.throwQuotaCount = count;
  }

  appendBuffer(data: ArrayBufferView): void {
    if (this.throwOnce) {
      const err = this.throwOnce;
      this.throwOnce = undefined;
      throw err;
    }
    if (this.throwQuotaCount > 0) {
      this.throwQuotaCount -= 1;
      const err = new Error('QuotaExceededError');
      err.name = 'QuotaExceededError';
      throw err;
    }
    this.updating = true;
    this.appended.push(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    queueMicrotask(() => {
      this.updating = false;
      this.dispatchEvent('updateend');
    });
  }

  remove(start: number, end: number): void {
    this.updating = true;
    this.removed.push([start, end]);
    queueMicrotask(() => {
      this.updating = false;
      this.dispatchEvent('updateend');
    });
  }

  abort(): void {
    this.updating = false;
  }

  changeType(type: string): void {
    this.changeTypeCalls.push(type);
  }
}

class MockMediaSource extends MockEventTarget {
  static isTypeSupported = vi.fn(() => true);
  static instances: MockMediaSource[] = [];
  static last: MockMediaSource | undefined = undefined;

  readyState = 'open';
  sourceBuffers: MockSourceBuffer[] = [];
  endOfStream = vi.fn(() => {
    this.readyState = 'ended';
  });

  constructor() {
    super();
    MockMediaSource.instances.push(this);
    MockMediaSource.last = this;
  }

  addSourceBuffer(_type: string): MockSourceBuffer {
    const sb = new MockSourceBuffer();
    this.sourceBuffers.push(sb);
    return sb;
  }

  removeSourceBuffer(sb: MockSourceBuffer): void {
    this.sourceBuffers = this.sourceBuffers.filter((s) => s !== sb);
  }

  static reset(): void {
    this.instances = [];
    this.last = undefined;
    this.isTypeSupported.mockReset();
    this.isTypeSupported.mockReturnValue(true);
  }
}

let urlCounter = 0;
class MockURL {
  static createObjectURL = vi.fn((obj: unknown) => {
    urlCounter += 1;
    return `blob:mock:${urlCounter}:${typeof obj}`;
  });
  static revokeObjectURL = vi.fn();
}

class MockHTMLVideoElement extends MockEventTarget implements HTMLVideoElementLike {
  src = '';
  srcObject: unknown = undefined;
  currentTime = 0;
  playbackRate = 1;
  paused = true;
  readyState = 0;
  error: { code: number; message: string } | null = null;
  play = vi.fn(async () => undefined);
  pause = vi.fn();
  load = vi.fn();
}

describe('MseBackend', () => {
  beforeEach(() => {
    MockMediaSource.reset();
    vi.stubGlobal('MediaSource', MockMediaSource);
    vi.stubGlobal('URL', MockURL);
    urlCounter = 0;
    MockURL.createObjectURL.mockClear();
    MockURL.revokeObjectURL.mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  function makeOptions(overrides?: Partial<MseBackendOptions>): MseBackendOptions {
    return {
      videoElement: new MockHTMLVideoElement(),
      tracks: [videoTrack, audioTrack],
      liveControlIntervalMs: 50,
      sourceOpenTimeoutMs: 100,
      ...overrides,
    };
  }

  function getMediaSource(): MockMediaSource {
    const last = MockMediaSource.last;
    if (!last) throw new Error('No MockMediaSource instance');
    return last;
  }

  function getSourceBuffer(): MockSourceBuffer {
    const ms = getMediaSource();
    const sb = ms.sourceBuffers[0];
    if (!sb) throw new Error('No MockSourceBuffer instance');
    return sb;
  }

  it('configures when MediaSource and MIME are supported', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video }));
    await backend.configure();
    expect(backend.identity).toBe('mse');
    expect(video.src).toMatch(/^blob:mock:/);
    expect(backend.metrics.mediaSourceReadyState).toBe('open');
    expect(backend.metrics.appendQueueDepth).toBe(0);
  });

  it('rejects configure when MediaSource API is missing', async () => {
    vi.unstubAllGlobals();
    const backend = new MseBackend(ctx, makeOptions());
    await expect(backend.configure()).rejects.toThrow('MediaSource API not available');
  });

  it('rejects configure when MIME is not supported', async () => {
    MockMediaSource.isTypeSupported.mockReturnValueOnce(false);
    const backend = new MseBackend(ctx, makeOptions());
    await expect(backend.configure()).rejects.toThrow('MIME type not supported');
  });

  it('rejects construction when videoElement is not an object', () => {
    expect(
      () => new MseBackend(ctx, makeOptions({ videoElement: 42 as unknown as HTMLVideoElementLike })),
    ).toThrow('videoElement is required');
  });

  it('rejects construction when isLive is not a boolean', () => {
    expect(() => new MseBackend(ctx, makeOptions({ isLive: 'true' as unknown as boolean }))).toThrow('isLive must be a boolean');
  });

  it('appends segments to the SourceBuffer', async () => {
    const backend = new MseBackend(ctx, makeOptions());
    await backend.configure();
    const data = new Uint8Array([1, 2, 3]);
    backend.pushSegment(data, { isInit: true });
    await flushMicrotasks(5);
    const sb = getSourceBuffer();
    expect(sb.appended.length).toBe(1);
    expect([...sb.appended[0]!]).toEqual([1, 2, 3]);
    expect(sb.changeTypeCalls.length).toBe(1);
  });

  it('enqueues appends and processes them serially', async () => {
    const backend = new MseBackend(ctx, makeOptions());
    await backend.configure();
    backend.pushSegment(new Uint8Array([1]), { isInit: false });
    backend.pushSegment(new Uint8Array([2]), { isInit: false });
    backend.pushSegment(new Uint8Array([3]), { isInit: false });
    expect(backend.metrics.appendQueueDepth).toBe(3);
    await flushMicrotasks(10);
    const sb = getSourceBuffer();
    expect(sb.appended.length).toBe(3);
    expect(backend.metrics.appendQueueDepth).toBe(0);
  });

  it('drops segments when the append queue is full', async () => {
    const backend = new MseBackend(ctx, makeOptions({ maxAppendQueue: 2 }));
    await backend.configure();
    backend.pushSegment(new Uint8Array([1]));
    backend.pushSegment(new Uint8Array([2]));
    backend.pushSegment(new Uint8Array([3]));
    backend.pushSegment(new Uint8Array([4]));
    expect(backend.metrics.droppedSegments).toBe(2);
    await flushMicrotasks(10);
    const sb = getSourceBuffer();
    expect(sb.appended.length).toBe(2);
  });

  it('retries append on QuotaExceededError and cleans buffer', async () => {
    const onError = vi.fn((err: MseError) => err);
    const backend = new MseBackend(ctx, makeOptions({ callbacks: { onError }, maxQuotaRetries: 1 }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setBuffered([[0, 10]]);
    sb.setThrowQuotaOnce();
    backend.pushSegment(new Uint8Array([1, 2, 3]));
    await flushMicrotasks(10);
    expect(sb.removed.length).toBeGreaterThanOrEqual(1);
    expect(backend.metrics.quotaCleanupCount).toBe(1);
    expect(sb.appended.length).toBe(1);
    expect(onError).not.toHaveBeenCalled();
  });

  it('resets quota retry budget after a successful append', async () => {
    const onError = vi.fn((err: MseError) => err);
    const backend = new MseBackend(ctx, makeOptions({ callbacks: { onError }, maxQuotaRetries: 1 }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setBuffered([[0, 10]]);

    sb.setThrowQuotaOnce();
    backend.pushSegment(new Uint8Array([1, 2, 3]));
    await flushMicrotasks(10);
    expect(sb.appended.length).toBe(1);
    expect(backend.metrics.quotaCleanupCount).toBe(1);

    sb.setThrowQuotaOnce();
    backend.pushSegment(new Uint8Array([4, 5, 6]));
    await flushMicrotasks(10);
    expect(sb.appended.length).toBe(2);
    expect(backend.metrics.quotaCleanupCount).toBe(2);
    expect(onError).not.toHaveBeenCalled();
  });

  it('fails immediately when quota cleanup has no buffer to remove', async () => {
    const onError = vi.fn((err: MseError) => err);
    const backend = new MseBackend(ctx, makeOptions({ callbacks: { onError }, maxQuotaRetries: 1 }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setBuffered([]);
    sb.setThrowQuotaOnce();
    backend.pushSegment(new Uint8Array([1, 2, 3]));
    await flushMicrotasks(5);
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ code: 'quota-exceeded' }));
    expect(sb.appended.length).toBe(0);
  });

  it('forwards source buffer errors to onError callback', async () => {
    const onError = vi.fn((err: MseError) => err);
    const backend = new MseBackend(ctx, makeOptions({ callbacks: { onError } }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setThrowOnce(new Error('append failed'));
    backend.pushSegment(new Uint8Array([1]));
    await flushMicrotasks(5);
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ code: 'append-error' }));
  });

  it('stops and detaches the media source', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video }));
    await backend.configure();
    const objectUrl = video.src;
    await backend.stop();
    expect(video.src).toBe('');
    expect(MockURL.revokeObjectURL).toHaveBeenCalledWith(objectUrl);
    expect(backend.metrics.mediaSourceReadyState).toBe('closed');
  });

  it('adjusts playback rate and removes stale buffer during live control', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'setTimeout', 'clearTimeout'] });
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(
      ctx,
      makeOptions({
        videoElement: video,
        liveControlIntervalMs: 10,
        maxBufferBehindMs: 1000,
        maxBufferAheadMs: 3000,
        liveLatencyTargetMs: 500,
        liveDriftSmallMs: 100,
        liveDriftLargeMs: 500,
      }),
    );
    await backend.configure();
    const sb = getSourceBuffer();
    video.currentTime = 10;
    video.playbackRate = 1.2;
    sb.setBuffered([[0, 20]]);
    // buffer ahead = (20-10)*1000 = 10000ms > target+large -> seek
    await vi.advanceTimersByTimeAsync(20);
    expect(backend.metrics.seekCount).toBe(1);
    expect(video.currentTime).toBeLessThan(20);
    expect(video.playbackRate).toBe(1);

    // small drift -> speed up (buffer ahead ~900ms, between small and large drift)
    video.currentTime = 19.1;
    sb.setBuffered([[15, 20]]);
    await vi.advanceTimersByTimeAsync(20);
    expect(video.playbackRate).toBeGreaterThan(1);

    vi.useRealTimers();
    await backend.stop();
  });

  it('factory creates a backend implementing MediaBackend', () => {
    const factory = mseBackendFactory(makeOptions());
    const backend = factory(ctx) as MediaBackend;
    expect(backend.identity).toBe('mse');
    expect(typeof backend.configure).toBe('function');
    expect(typeof backend.stop).toBe('function');
  });

  it('rejects configure when MediaSource sourceopen times out', async () => {
    class SlowMediaSource extends MockMediaSource {
      readyState = 'closed';
    }
    vi.stubGlobal('MediaSource', SlowMediaSource);
    const backend = new MseBackend(ctx, makeOptions({ sourceOpenTimeoutMs: 50 }));
    await expect(backend.configure()).rejects.toThrow('MediaSource did not open');
  });

  it('cleans up resources when configure fails', async () => {
    class SlowMediaSource extends MockMediaSource {
      readyState = 'closed';
    }
    vi.stubGlobal('MediaSource', SlowMediaSource);
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, sourceOpenTimeoutMs: 50 }));
    await expect(backend.configure()).rejects.toThrow('MediaSource did not open');
    expect(video.src).toBe('');
    expect(MockURL.revokeObjectURL).toHaveBeenCalled();
  });

  it('seek clears buffer, updates currentTime and resolves on seeked', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, isLive: false }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setBuffered([[0, 10]]);

    const seekPromise = backend.seek(5000);
    await flushMicrotasks(2);
    expect(sb.removed.length).toBeGreaterThanOrEqual(1);
    expect(video.currentTime).toBe(5);

    video.dispatchEvent('seeked');
    await expect(seekPromise).resolves.toBeUndefined();
    expect(backend.metrics.seekCount).toBe(1);
  });

  it('seek rejects before configure', async () => {
    const backend = new MseBackend(ctx, makeOptions());
    await expect(backend.seek(1000)).rejects.toThrow('Cannot seek before configure');
  });

  it('setPlaybackRate sets the video playback rate', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video }));
    await backend.configure();
    await backend.setPlaybackRate(2);
    expect(video.playbackRate).toBe(2);
  });

  it('setPlaybackRate rejects out of range values', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video }));
    await backend.configure();
    await expect(backend.setPlaybackRate(0.05)).rejects.toThrow('playback rate');
    await expect(backend.setPlaybackRate(20)).rejects.toThrow('playback rate');
  });

  it('VOD mode does not adjust playback rate for live catch-up', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, isLive: false, liveControlIntervalMs: 10 }));
    await backend.configure();
    const sb = getSourceBuffer();
    video.currentTime = 5;
    sb.setBuffered([[0, 20]]);
    video.playbackRate = 1;

    vi.advanceTimersByTime(50);
    expect(video.playbackRate).toBe(1);
    vi.useRealTimers();
  });

  it('stopping during seek does not restart the live control timer', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, liveControlIntervalMs: 10 }));
    await backend.configure();
    const sb = getSourceBuffer();
    sb.setBuffered([[0, 10]]);

    const seekPromise = backend.seek(5000);
    await flushMicrotasks(2);
    await backend.stop();

    // Let the pending seek timeout fire.
    vi.advanceTimersByTime(6000);
    await expect(seekPromise).rejects.toThrow('Seeked event timeout');

    // Advance far enough that a restarted live control timer would have fired.
    vi.advanceTimersByTime(1000);
    expect(video.playbackRate).toBe(1);
    vi.useRealTimers();
  });

  it('pauseDisplay freezes live latency correction but keeps appending when keepConnection is true', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, liveControlIntervalMs: 10, maxBufferAheadMs: 1000 }));
    await backend.configure();
    const sb = getSourceBuffer();

    backend.pushSegment(new Uint8Array([0, 1, 2]), { isInit: true });
    await flushMicrotasks(2);
    expect(sb.appended.length).toBe(1);

    await backend.pauseDisplay(true);
    expect(video.pause).toHaveBeenCalled();
    backend.pushSegment(new Uint8Array([3, 4, 5]));
    await flushMicrotasks(2);
    expect(sb.appended.length).toBe(2);

    vi.useRealTimers();
    await backend.stop();
  });

  it('pauseDisplay with keepConnection false drops new segments', async () => {
    const backend = new MseBackend(ctx, makeOptions());
    await backend.configure();
    await backend.pauseDisplay(false);
    backend.pushSegment(new Uint8Array([0, 1, 2]));
    expect(backend.metrics.droppedSegments).toBe(1);
    await backend.stop();
  });

  it('frameStep rejects keyframe-only mode in MSE', async () => {
    const backend = new MseBackend(ctx, makeOptions());
    await backend.configure();
    await expect(backend.frameStep('forward', true)).rejects.toThrow('Keyframe-only');
    await backend.stop();
  });

  it('frameStep moves currentTime by one frame and resolves on seeked', async () => {
    const video = new MockHTMLVideoElement();
    const backend = new MseBackend(ctx, makeOptions({ videoElement: video, videoFrameRate: 30 }));
    await backend.configure();
    video.currentTime = 1.0;

    const stepPromise = backend.frameStep('forward');
    expect(video.currentTime).toBeCloseTo(1.0 + 1 / 30, 6);
    video.dispatchEvent('seeked');
    await expect(stepPromise).resolves.toBeUndefined();
    await backend.stop();
  });
});

async function flushMicrotasks(count = 10): Promise<void> {
  for (let i = 0; i < count; i++) {
    await Promise.resolve();
  }
}
