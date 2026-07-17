import { describe, expect, it, vi, afterEach } from 'vitest';
import { MicrophoneCapture, CaptureError, type AudioPacket, type AudioContextLike, type AudioWorkletNodeLike, type AudioWorkletNodeConstructor } from './capture';

type PortLike = AudioWorkletNodeLike['port'];
type GetUserMedia = (constraints: MediaStreamConstraints) => Promise<MediaStream>;

function createFakeAudioWorkletNode(): AudioWorkletNodeLike {
  let onmessage: PortLike['onmessage'] = null;
  const port: PortLike = {
    get onmessage() {
      return onmessage;
    },
    set onmessage(fn) {
      onmessage = fn;
    },
    postMessage: (message: unknown) => {
      if (onmessage) {
        onmessage({ data: message });
      }
    },
  };
  return {
    port,
    connect: vi.fn(),
    disconnect: vi.fn(),
  };
}

interface TestEnv {
  sampleRate: number;
  getUserMedia: GetUserMedia;
  audioContext: AudioContextLike;
  workletNodeCtor: AudioWorkletNodeConstructor;
  fakeWorkletNode: AudioWorkletNodeLike;
  fakeSource: { connect: ReturnType<typeof vi.fn>; disconnect: ReturnType<typeof vi.fn> };
  fakeStream: { getAudioTracks: ReturnType<typeof vi.fn> };
  fakeTrack: { stop: ReturnType<typeof vi.fn> };
}

function createTestEnv(sampleRate = 8000): TestEnv {
  const fakeWorkletNode = createFakeAudioWorkletNode();
  const fakeSource = {
    connect: vi.fn(),
    disconnect: vi.fn(),
  };
  const fakeTrack = { stop: vi.fn() };
  const fakeStream = {
    getAudioTracks: vi.fn().mockReturnValue([fakeTrack]),
  };
  const getUserMedia = vi.fn().mockResolvedValue(fakeStream as unknown as MediaStream) as unknown as GetUserMedia;

  const audioContext: AudioContextLike = {
    sampleRate,
    state: 'running',
    destination: { maxChannelCount: 2 },
    resume: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    audioWorklet: {
      addModule: vi.fn().mockResolvedValue(undefined),
    },
    createMediaStreamSource: vi.fn().mockReturnValue(fakeSource),
  };

  const workletNodeCtor = vi.fn(function () {
    return fakeWorkletNode;
  }) as unknown as AudioWorkletNodeConstructor;

  return {
    sampleRate,
    getUserMedia,
    audioContext,
    workletNodeCtor,
    fakeWorkletNode,
    fakeSource,
    fakeStream,
    fakeTrack,
  };
}

describe('MicrophoneCapture', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('starts and captures a G.711 encoded frame', async () => {
    const env = createTestEnv();
    const packets: AudioPacket[] = [];

    const capture = new MicrophoneCapture(
      {
        audioContext: env.audioContext,
        workletNodeCtor: env.workletNodeCtor,
        getUserMedia: env.getUserMedia,
        sampleRate: 8000,
        frameDurationMs: 20,
      },
      {
        onPacket: (packet) => packets.push(packet),
      },
    );

    await capture.start();

    expect(env.getUserMedia).toHaveBeenCalledWith({ audio: true });
    expect(env.audioContext.audioWorklet.addModule).toHaveBeenCalled();
    expect(env.workletNodeCtor).toHaveBeenCalled();
    expect(env.fakeSource.connect).toHaveBeenCalledWith(env.fakeWorkletNode);
    expect(env.fakeWorkletNode.connect).toHaveBeenCalledWith(env.audioContext.destination);
    expect(capture.isRunning).toBe(true);

    // Simulate two frames of silence from the AudioWorklet processor.
    // The resampler needs one sample of lookahead, so the first frame only
    // produces 159 output samples; the second frame yields a full encoded packet.
    const frame = new Float32Array(160).fill(0);
    env.fakeWorkletNode.port.postMessage({ type: 'frame', samples: frame });
    env.fakeWorkletNode.port.postMessage({ type: 'frame', samples: frame });

    expect(packets.length).toBe(1);
    const packet = packets[0]!;
    expect(packet.kind).toBe('mulaw');
    expect(packet.payload.length).toBe(160);
    expect(packet.sampleRate).toBe(8000);
    expect(packet.channels).toBe(1);
    expect(packet.payload[0]).toBe(0xff); // mu-law silence

    await capture.stop();
    expect(capture.isRunning).toBe(false);
  });

  it('classifies permission denial', async () => {
    const getUserMedia = vi.fn().mockRejectedValue({ name: 'NotAllowedError', message: 'denied' }) as unknown as GetUserMedia;
    const onError = vi.fn();

    const capture = new MicrophoneCapture(
      { getUserMedia },
      { onError },
    );

    await expect(capture.start()).rejects.toBeInstanceOf(CaptureError);
    const error = onError.mock.calls[0]?.[0] as CaptureError;
    expect(error?.code).toBe('permission-denied');
  });

  it('reports not-supported when AudioWorkletNode is missing', async () => {
    const env = createTestEnv();
    const onError = vi.fn();

    const capture = new MicrophoneCapture(
      {
        audioContext: env.audioContext,
        getUserMedia: env.getUserMedia,
      },
      { onError },
    );

    await expect(capture.start()).rejects.toBeInstanceOf(CaptureError);
    const error = onError.mock.calls[0]?.[0] as CaptureError;
    expect(error?.code).toBe('not-supported');
  });

  it('releases the microphone stream when start fails after getUserMedia', async () => {
    const env = createTestEnv();
    env.audioContext.audioWorklet.addModule = vi.fn().mockRejectedValue(new Error('addModule failed'));

    const capture = new MicrophoneCapture({
      audioContext: env.audioContext,
      workletNodeCtor: env.workletNodeCtor,
      getUserMedia: env.getUserMedia,
    });

    await expect(capture.start()).rejects.toBeInstanceOf(CaptureError);
    expect(env.fakeTrack.stop).toHaveBeenCalled();
    // Injected audioContext must not be closed by the capture.
    expect(env.audioContext.close).not.toHaveBeenCalled();
  });

  it('emits A-law packets when configured', async () => {
    const env = createTestEnv();
    const packets: AudioPacket[] = [];

    const capture = new MicrophoneCapture(
      {
        audioContext: env.audioContext,
        workletNodeCtor: env.workletNodeCtor,
        getUserMedia: env.getUserMedia,
        encoder: 'alaw',
      },
      { onPacket: (packet) => packets.push(packet) },
    );

    await capture.start();
    const frame = new Float32Array(160).fill(0);
    env.fakeWorkletNode.port.postMessage({ type: 'frame', samples: frame });
    env.fakeWorkletNode.port.postMessage({ type: 'frame', samples: frame });

    expect(packets[0]?.kind).toBe('alaw');
    expect(packets[0]?.payload[0]).toBe(0xd5); // A-law silence

    await capture.stop();
  });

  it('caps a burst to maxBufferedFrames + 1 packets and drops the excess', async () => {
    const env = createTestEnv();
    const packets: AudioPacket[] = [];

    const capture = new MicrophoneCapture(
      {
        audioContext: env.audioContext,
        workletNodeCtor: env.workletNodeCtor,
        getUserMedia: env.getUserMedia,
        maxBufferedFrames: 1,
        sampleRate: 8000,
      },
      { onPacket: (packet) => packets.push(packet) },
    );

    await capture.start();
    // A single huge worklet frame produces a large resampled block; only the
    // first maxBufferedFrames+1 encoded frames are emitted, the rest is dropped.
    env.fakeWorkletNode.port.postMessage({
      type: 'frame',
      samples: new Float32Array(10000).fill(0),
    });

    expect(packets.length).toBeLessThanOrEqual(2);
    expect(capture.getMetrics().droppedFrames).toBeGreaterThan(0);
    expect(capture.getMetrics().bufferedFrames).toBeLessThanOrEqual(1);

    await capture.stop();
  });
});
