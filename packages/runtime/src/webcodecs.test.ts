import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { WebCodecsBackend, webcodecsBackendFactory, type CloseableVideoFrame, type CloseableAudioData } from './webcodecs';
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
    queueMicrotask(() => {
      if (!this.closed && this.state !== 'error') {
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
    queueMicrotask(() => {
      if (!this.closed && this.state !== 'error') {
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

  it('forwards decoder errors to onError callback', async () => {
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
    decoders[decoders.length - 1]?.simulateError('decode failed');

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: 'decode failed' }));
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
});
