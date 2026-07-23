import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  WebCodecsBackend,
  webcodecsBackendFactory,
  type CloseableVideoFrame,
  type CloseableAudioData,
  type WebCodecsBackendOptions,
} from './webcodecs';
import type { BackendContext, MediaBackend } from './fallback';
import type { TrackProfile } from './planner';

const videoTrack: TrackProfile = { kind: 'video', codec: 'h264', width: 640, height: 360 };
const audioTrack: TrackProfile = { kind: 'audio', codec: 'aac', sampleRate: 48000, channels: 2 };

const ctx: BackendContext = {
  candidate: {
    rank: 1,
    videoBackend: 'webcodecs',
    audioBackend: 'webcodecs',
    renderer: undefined,
    transport: 'fetch',
    reason: 'test',
    isLive: true,
  },
  reason: 'initial',
};

function createMockFrame(timestamp: number): CloseableVideoFrame {
  return {
    timestamp,
    codedWidth: 640,
    codedHeight: 360,
    format: 'I420',
    close: vi.fn(),
  };
}

function createMockAudio(timestamp: number): CloseableAudioData {
  return {
    timestamp,
    numberOfFrames: 1024,
    sampleRate: 48000,
    numberOfChannels: 2,
    close: vi.fn(),
  };
}

class MockVideoDecoder {
  static isConfigSupported = vi.fn(async () => ({ supported: true }));
  state = 'unconfigured';
  output: (frame: CloseableVideoFrame) => void = () => undefined;
  error: (err: Error) => void = () => undefined;
  decodeCalls: unknown[] = [];
  flushed = false;
  closed = false;
  private issueToken = 0;
  private resetToken = 0;

  constructor(init: { output: (frame: CloseableVideoFrame) => void; error: (err: Error) => void }) {
    this.output = init.output;
    this.error = init.error;
  }

  configure(_config: object): void {
    this.state = 'configured';
  }

  decode(chunk: object): void {
    this.decodeCalls.push(chunk);
    const frame = createMockFrame(0);
    const token = ++this.issueToken;
    queueMicrotask(() => {
      if (!this.closed && this.state !== 'error' && token > this.resetToken) {
        this.output(frame);
      }
    });
  }

  flush(): Promise<void> {
    this.flushed = true;
    return new Promise((resolve) => queueMicrotask(resolve));
  }

  reset(): void {
    this.state = 'configured';
    this.resetToken = this.issueToken;
  }

  close(): void {
    this.closed = true;
    this.state = 'closed';
  }

  simulateError(message: string): void {
    this.state = 'error';
    this.error(new Error(message));
  }
}

class MockAudioDecoder {
  static isConfigSupported = vi.fn(async () => ({ supported: true }));
  state = 'unconfigured';
  output: (data: CloseableAudioData) => void = () => undefined;
  error: (err: Error) => void = () => undefined;
  decodeCalls: unknown[] = [];
  flushed = false;
  closed = false;
  private issueToken = 0;
  private resetToken = 0;

  constructor(init: { output: (data: CloseableAudioData) => void; error: (err: Error) => void }) {
    this.output = init.output;
    this.error = init.error;
  }

  configure(_config: object): void {
    this.state = 'configured';
  }

  decode(chunk: object): void {
    this.decodeCalls.push(chunk);
    const data = createMockAudio(0);
    const token = ++this.issueToken;
    queueMicrotask(() => {
      if (!this.closed && this.state !== 'error' && token > this.resetToken) {
        this.output(data);
      }
    });
  }

  flush(): Promise<void> {
    this.flushed = true;
    return new Promise((resolve) => queueMicrotask(resolve));
  }

  reset(): void {
    this.state = 'configured';
    this.resetToken = this.issueToken;
  }

  close(): void {
    this.closed = true;
    this.state = 'closed';
  }
}

class MockEncodedVideoChunk {
  readonly type: 'key' | 'delta';
  readonly timestamp: number;
  readonly data: ArrayBufferView;
  constructor(init: { type: 'key' | 'delta'; timestamp: number; data: ArrayBufferView }) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.data = init.data;
  }
}

class MockEncodedAudioChunk {
  readonly type: 'key' | 'delta';
  readonly timestamp: number;
  readonly data: ArrayBufferView;
  constructor(init: { type: 'key' | 'delta'; timestamp: number; data: ArrayBufferView }) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.data = init.data;
  }
}

describe('WebCodecsBackend', () => {
  beforeEach(() => {
    vi.stubGlobal('VideoDecoder', MockVideoDecoder);
    vi.stubGlobal('AudioDecoder', MockAudioDecoder);
    vi.stubGlobal('EncodedVideoChunk', MockEncodedVideoChunk);
    vi.stubGlobal('EncodedAudioChunk', MockEncodedAudioChunk);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('configures when video and audio decoders are supported', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack, audioTrack], callbacks: {} });
    await expect(backend.configure()).resolves.toBeUndefined();
    expect(backend.identity).toBe('webcodecs');
  });

  it('rejects configure when the video codec is unsupported', async () => {
    MockVideoDecoder.isConfigSupported.mockResolvedValueOnce({ supported: false });
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await expect(backend.configure()).rejects.toThrow('VideoDecoder does not support');
  });

  it('rejects configure when WebCodecs API is missing', async () => {
    vi.unstubAllGlobals();
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await expect(backend.configure()).rejects.toThrow('WebCodecs API not available');
  });

  it('configures video-only when AudioDecoder is missing', async () => {
    vi.unstubAllGlobals();
    vi.stubGlobal('VideoDecoder', MockVideoDecoder);
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await expect(backend.configure()).resolves.toBeUndefined();
  });

  it('emits decoded video and audio frames', async () => {
    const onVideoFrame = vi.fn();
    const onAudioData = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack, audioTrack],
      callbacks: { onVideoFrame, onAudioData },
    });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    backend.pushAudio(new Uint8Array([1, 2, 3]), 2000);

    await Promise.resolve();
    await Promise.resolve();

    expect(onVideoFrame).toHaveBeenCalledTimes(1);
    expect(onAudioData).toHaveBeenCalledTimes(1);
    expect(backend.metrics.decodedVideoFrames).toBe(1);
    expect(backend.metrics.decodedAudioFrames).toBe(1);
  });

  it('stops previous outputs and closes decoders on stop', async () => {
    const onVideoFrame = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: { onVideoFrame },
    });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    await backend.stop();

    // Frame delivered during flush before close is still emitted; late frames
    // after stop are closed by the backend.
    expect(onVideoFrame).toHaveBeenCalled();
    // No further outputs accepted after stop.
    const before = onVideoFrame.mock.calls.length;
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 2000, { isKeyFrame: true });
    await Promise.resolve();
    expect(onVideoFrame).toHaveBeenCalledTimes(before);
  });

  it('forwards decoder errors to onError callback and resets pending count', async () => {
    const onError = vi.fn();
    const decoders: MockVideoDecoder[] = [];
    vi.stubGlobal('VideoDecoder', class extends MockVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        decoders.push(this);
      }
    });

    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: { onError },
    });
    await backend.configure();
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    expect(backend.metrics.pendingDecodes).toBe(1);

    decoders[decoders.length - 1]?.simulateError('decode failed');

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'decode failed' }));
    expect(backend.metrics.pendingDecodes).toBe(0);
    // Further pushes are ignored after error.
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 2000, { isKeyFrame: true });
    expect(backend.metrics.pendingDecodes).toBe(0);
  });

  it('drops video chunks when the decode queue is full', async () => {
    const onVideoFrame = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: { onVideoFrame },
      maxPendingDecodes: 2,
    });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 2000, { isKeyFrame: false });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 3000, { isKeyFrame: false });

    expect(backend.metrics.pendingDecodes).toBe(2);
    expect(backend.metrics.droppedChunks).toBe(1);

    await Promise.resolve();
    expect(onVideoFrame).toHaveBeenCalledTimes(2);
  });

  it('detects Annex-B keyframes in short buffers', async () => {
    class CaptureVideoDecoder extends MockVideoDecoder {
      chunks: MockEncodedVideoChunk[] = [];
      decode(chunk: object): void {
        this.chunks.push(chunk as MockEncodedVideoChunk);
        super.decode(chunk);
      }
    }

    const decoders: CaptureVideoDecoder[] = [];
    vi.stubGlobal('VideoDecoder', class extends CaptureVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        decoders.push(this);
      }
    });

    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 1, 0x65]), 1000); // 3-byte start code + IDR
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 2000); // 4-byte start code + non-IDR
    backend.pushVideo(new Uint8Array([0, 1, 2, 3, 0x65]), 3000); // no start code

    const last = decoders[decoders.length - 1]!;
    expect(last.chunks[0]?.type).toBe('key');
    expect(last.chunks[1]?.type).toBe('delta');
    expect(last.chunks[2]?.type).toBe('delta');
  });

  it('detects H.264 keyframes with leading SPS/PPS NAL units', async () => {
    class CaptureVideoDecoder extends MockVideoDecoder {
      chunks: MockEncodedVideoChunk[] = [];
      decode(chunk: object): void {
        this.chunks.push(chunk as MockEncodedVideoChunk);
        super.decode(chunk);
      }
    }

    const decoders: CaptureVideoDecoder[] = [];
    vi.stubGlobal('VideoDecoder', class extends CaptureVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        decoders.push(this);
      }
    });

    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();

    // SPS (type 7), PPS (type 8), then IDR (type 5) access unit.
    const buffer = new Uint8Array([
      0, 0, 0, 1, 0x67, // SPS
      0, 0, 0, 1, 0x68, // PPS
      0, 0, 0, 1, 0x65, // IDR
    ]);
    backend.pushVideo(buffer, 1000);

    const last = decoders[decoders.length - 1]!;
    expect(last.chunks[0]?.type).toBe('key');
  });

  it('throws when configure is called twice', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();
    await expect(backend.configure()).rejects.toThrow('already configured');
  });

  it('factory creates a backend implementing MediaBackend', () => {
    const factory = webcodecsBackendFactory({ tracks: [videoTrack], callbacks: {} });
    const backend = factory(ctx) as MediaBackend;
    expect(backend.identity).toBe('webcodecs');
    expect(typeof backend.configure).toBe('function');
    expect(typeof backend.stop).toBe('function');
  });

  it('closes video decoder when audio decoder creation fails', async () => {
    const videoInstances: MockVideoDecoder[] = [];
    class ThrowingAudioDecoder extends MockAudioDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        throw new Error('audio decoder boom');
      }
    }

    vi.stubGlobal('VideoDecoder', class extends MockVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        videoInstances.push(this);
      }
    });
    vi.stubGlobal('AudioDecoder', ThrowingAudioDecoder);

    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack, audioTrack],
      callbacks: {},
    });
    await expect(backend.configure()).rejects.toThrow('audio decoder boom');
    expect(videoInstances[0]?.closed).toBe(true);
  });

  it('completes stop even when decoder close throws', async () => {
    class CloseThrowingVideoDecoder extends MockVideoDecoder {
      close(): void {
        throw new Error('already closed');
      }
    }
    vi.stubGlobal('VideoDecoder', CloseThrowingVideoDecoder);
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();
    await expect(backend.stop()).resolves.toBeUndefined();
  });

  it('resets pendingDecodes when reconfiguring on a keyframe', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {}, maxPendingDecodes: 5 });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 1000);
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 2000);
    expect(backend.metrics.pendingDecodes).toBe(2);

    backend.markVideoConfigChanged();
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 3000);
    // reset() discarded the two earlier deltas; only the keyframe remains pending.
    expect(backend.metrics.pendingDecodes).toBe(1);
  });

  it('closes frame and reports error when onVideoFrame callback throws', async () => {
    let frame: CloseableVideoFrame | undefined;
    const onError = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: {
        onVideoFrame: (f) => {
          frame = f;
          throw new Error('callback boom');
        },
        onError,
      },
    });
    await backend.configure();
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    await Promise.resolve();
    expect(frame).toBeDefined();
    expect(frame?.close).toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'callback boom' }));
  });

  it('closes audio data and reports error when onAudioData callback throws', async () => {
    let audio: CloseableAudioData | undefined;
    const onError = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [audioTrack],
      callbacks: {
        onAudioData: (d) => {
          audio = d;
          throw new Error('audio callback boom');
        },
        onError,
      },
    });
    await backend.configure();
    backend.pushAudio(new Uint8Array([1, 2, 3]), 1000);
    await Promise.resolve();
    expect(audio).toBeDefined();
    expect(audio?.close).toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'audio callback boom' }));
  });

  it('pauseDisplay keeps connection and queues incoming video frames', async () => {
    const onVideoFrame = vi.fn();
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: { onVideoFrame },
    });
    await backend.configure();
    await backend.pauseDisplay(true);

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    await Promise.resolve();
    // Queued, not decoded yet.
    expect(onVideoFrame).not.toHaveBeenCalled();

    const stepPromise = backend.frameStep('forward');
    await Promise.resolve();
    await expect(stepPromise).resolves.toBeUndefined();
    expect(onVideoFrame).toHaveBeenCalledTimes(1);
  });

  it('frameStep decodes exactly one chunk even when multiple chunks arrive during the step', async () => {
    class CaptureVideoDecoder extends MockVideoDecoder {
      chunks: MockEncodedVideoChunk[] = [];
      decode(chunk: object): void {
        this.chunks.push(chunk as MockEncodedVideoChunk);
        super.decode(chunk);
      }
    }
    const decoders: CaptureVideoDecoder[] = [];
    vi.stubGlobal('VideoDecoder', class extends CaptureVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        decoders.push(this);
      }
    });

    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: {},
    });
    await backend.configure();
    await backend.pauseDisplay(true);

    const stepPromise = backend.frameStep('forward');
    // Multiple chunks arrive before the first output is produced.
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 2000, { isKeyFrame: false });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 3000, { isKeyFrame: false });

    await stepPromise;
    await Promise.resolve();
    const last = decoders[decoders.length - 1]!;
    expect(last.chunks.map((c) => c.timestamp)).toEqual([1000]);
    await backend.stop();
  });

  it('pausing with connection preserves the oldest queued chunks on overflow', async () => {
    const onVideoFrame = vi.fn();
    class CaptureVideoDecoder extends MockVideoDecoder {
      chunks: MockEncodedVideoChunk[] = [];
      decode(chunk: object): void {
        this.chunks.push(chunk as MockEncodedVideoChunk);
        super.decode(chunk);
      }
    }
    const decoders: CaptureVideoDecoder[] = [];
    vi.stubGlobal('VideoDecoder', class extends CaptureVideoDecoder {
      constructor(init: { output: () => void; error: (err: Error) => void }) {
        super(init);
        decoders.push(this);
      }
    });

    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack],
      callbacks: { onVideoFrame },
      maxVideoQueue: 2,
    });
    await backend.configure();
    await backend.pauseDisplay(true);

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 2000, { isKeyFrame: false });
    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x41]), 3000, { isKeyFrame: false });
    expect(backend.metrics.droppedChunks).toBe(1);

    await backend.frameStep('forward');
    await backend.frameStep('forward');
    await Promise.resolve();
    expect(onVideoFrame).toHaveBeenCalledTimes(2);
    expect(decoders[0]?.chunks.map((c) => c.timestamp)).toEqual([1000, 2000]);
    await backend.stop();
  });

  it('frameStep rejects backward direction', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();
    await backend.pauseDisplay(true);
    await expect(backend.frameStep('backward')).rejects.toThrow('Backward');
    await backend.stop();
  });

  it('frameStep requires pauseDisplay to be active', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();
    await expect(backend.frameStep('forward')).rejects.toThrow('pauseDisplay');
    await backend.stop();
  });

  it('frameStep rejects when stop is called before the next frame arrives', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();
    await backend.pauseDisplay(true);

    const stepPromise = backend.frameStep('forward');
    await backend.stop();

    await expect(stepPromise).rejects.toThrow('stopped during frame step');
  });

  it('pauseDisplay(false) clears in-flight decode counters', async () => {
    const backend = new WebCodecsBackend(ctx, {
      tracks: [videoTrack, audioTrack],
      callbacks: {},
      maxPendingDecodes: 5,
    });
    await backend.configure();

    backend.pushVideo(new Uint8Array([0, 0, 0, 1, 0x65]), 1000, { isKeyFrame: true });
    backend.pushAudio(new Uint8Array([1, 2, 3]), 2000);
    expect(backend.metrics.pendingDecodes).toBe(2);

    await backend.pauseDisplay(false);
    expect(backend.metrics.pendingDecodes).toBe(0);
    await backend.stop();
  });

  it('throws for invalid constructor options', () => {
    expect(() => new WebCodecsBackend(ctx, undefined as unknown as WebCodecsBackendOptions)).toThrow(
      'WebCodecsBackendOptions is required',
    );
    expect(() => new WebCodecsBackend(ctx, {} as unknown as WebCodecsBackendOptions)).toThrow(
      'tracks must be a non-empty array',
    );
    expect(
      () => new WebCodecsBackend(ctx, { tracks: [], callbacks: {} } as unknown as WebCodecsBackendOptions),
    ).toThrow('tracks must be a non-empty array');
    expect(
      () =>
        new WebCodecsBackend(
          ctx,
          { tracks: [{ kind: 'video' }], callbacks: {} } as unknown as WebCodecsBackendOptions,
        ),
    ).toThrow('codec');
    expect(
      () =>
        new WebCodecsBackend(
          ctx,
          { tracks: [videoTrack], callbacks: { onError: 'not fn' } } as unknown as WebCodecsBackendOptions,
        ),
    ).toThrow('onError must be a function');
    expect(
      () =>
        new WebCodecsBackend(
          ctx,
          { tracks: [videoTrack], callbacks: {}, maxPendingDecodes: 0 } as unknown as WebCodecsBackendOptions,
        ),
    ).toThrow('maxPendingDecodes');
  });

  it('throws when pushVideo receives invalid arguments', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [videoTrack], callbacks: {} });
    await backend.configure();

    expect(() => backend.pushVideo('not bytes' as unknown as Uint8Array, 1000, { isKeyFrame: true })).toThrow(
      'pushVideo data',
    );
    expect(() => backend.pushVideo(new Uint8Array([1]), NaN, { isKeyFrame: true })).toThrow(
      'pushVideo timestamp',
    );
    expect(() =>
      backend.pushVideo(new Uint8Array([1]), 1000, { isKeyFrame: 'yes' as unknown as boolean }),
    ).toThrow('pushVideo isKeyFrame');
    expect(() =>
      backend.pushVideo(new Uint8Array([1]), 1000, { isKeyFrame: true, duration: -1 }),
    ).toThrow('pushVideo duration');
    expect(() =>
      backend.pushVideo(new Uint8Array([1]), 1000, 'opts' as unknown as { isKeyFrame: boolean }),
    ).toThrow('pushVideo opts');
  });

  it('throws when pushAudio receives invalid arguments', async () => {
    const backend = new WebCodecsBackend(ctx, { tracks: [audioTrack], callbacks: {} });
    await backend.configure();

    expect(() => backend.pushAudio('not bytes' as unknown as Uint8Array, 1000)).toThrow('pushAudio data');
    expect(() => backend.pushAudio(new Uint8Array([1]), NaN)).toThrow('pushAudio timestamp');
    expect(() => backend.pushAudio(new Uint8Array([1]), 1000, { duration: -1 })).toThrow(
      'pushAudio duration',
    );
    expect(() =>
      backend.pushAudio(new Uint8Array([1]), 1000, 'opts' as unknown as { duration: number }),
    ).toThrow('pushAudio opts');
  });
});
