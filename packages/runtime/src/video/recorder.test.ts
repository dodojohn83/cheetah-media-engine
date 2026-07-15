import { describe, it, expect, beforeEach } from 'vitest';
import { startRecording, type RecordingOptions } from './recorder';
import { RendererError } from './types';

class MockBlob {
  constructor(
    private readonly buffer: Uint8Array,
    public readonly type: string,
    public readonly size = buffer.length,
  ) {}

  async arrayBuffer(): Promise<ArrayBuffer> {
    return this.buffer.buffer.slice(this.buffer.byteOffset, this.buffer.byteOffset + this.buffer.byteLength) as ArrayBuffer;
  }
}

class MockMediaStream {
  public __recorder: MockMediaRecorder | undefined;
}

class MockHTMLCanvasElement {
  public __stream = new MockMediaStream();

  captureStream(_fps?: number): MockMediaStream {
    return this.__stream;
  }
}

class MockMediaRecorder {
  public state: 'inactive' | 'recording' | 'paused' = 'inactive';
  public mimeType = '';
  public ondataavailable: ((event: { data: Blob }) => void) | undefined;
  public onstop: (() => void) | undefined;
  public onerror: ((event: unknown) => void) | undefined;
  public __timeslice: number | undefined;

  constructor(
    public readonly stream: MockMediaStream,
    public readonly options: { mimeType?: string } = {},
  ) {
    this.mimeType = options.mimeType ?? 'video/webm';
    stream.__recorder = this;
  }

  start(timeslice?: number): void {
    this.state = 'recording';
    this.__timeslice = timeslice;
  }

  stop(): void {
    this.state = 'inactive';
    this.onstop?.();
  }

  dispatchData(bytes: Uint8Array): void {
    this.ondataavailable?.({ data: new MockBlob(bytes, this.mimeType) as unknown as Blob });
  }

  static isTypeSupported(_type: string): boolean {
    return true;
  }
}

function installMocks(): void {
  if (typeof globalThis.HTMLCanvasElement === 'undefined') {
    (globalThis as unknown as { HTMLCanvasElement: unknown }).HTMLCanvasElement = MockHTMLCanvasElement;
  }
  if (typeof globalThis.MediaStream === 'undefined') {
    (globalThis as unknown as { MediaStream: unknown }).MediaStream = MockMediaStream;
  }
  if (typeof globalThis.MediaRecorder === 'undefined') {
    (globalThis as unknown as { MediaRecorder: unknown }).MediaRecorder = MockMediaRecorder;
  }
}

function makeTarget(): { stream: WritableStream<Uint8Array>; chunks: Uint8Array[] } {
  const chunks: Uint8Array[] = [];
  const stream = new WritableStream<Uint8Array>({
    write(chunk) {
      chunks.push(chunk);
      return Promise.resolve();
    },
  });
  return { stream, chunks };
}

function makeOptions(
  target: WritableStream<Uint8Array>,
  segmentDurationMs?: number,
  mimeType?: string,
): RecordingOptions {
  return {
    target,
    filename: 'test-recording',
    ...(segmentDurationMs !== undefined ? { segmentDurationMs } : {}),
    ...(mimeType !== undefined ? { mimeType } : {}),
  };
}

describe('startRecording', () => {
  beforeEach(() => {
    installMocks();
  });

  it('rejects when MediaRecorder is unavailable', async () => {
    const saved = globalThis.MediaRecorder;
    delete (globalThis as unknown as { MediaRecorder?: unknown }).MediaRecorder;
    const { stream } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    await expect(startRecording(canvas as unknown as HTMLCanvasElement, makeOptions(stream))).rejects.toBeInstanceOf(
      RendererError,
    );
    (globalThis as unknown as { MediaRecorder: unknown }).MediaRecorder = saved;
  });

  it('starts a session from a canvas capture stream', async () => {
    const { stream } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    const session = await startRecording(canvas as unknown as HTMLCanvasElement, makeOptions(stream));
    expect(session).toBeDefined();
    expect(session.getStats().durationMs).toBeGreaterThanOrEqual(0);
  });

  it('writes chunks and reports stats', async () => {
    const { stream, chunks } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    const session = await startRecording(canvas as unknown as HTMLCanvasElement, makeOptions(stream));

    const recorder = canvas.__stream.__recorder;
    expect(recorder).toBeDefined();
    recorder?.dispatchData(new Uint8Array([1, 2, 3]));
    await new Promise((resolve) => setTimeout(resolve, 0));

    const stats = session.getStats();
    expect(stats.bytesWritten).toBe(3);
    expect(stats.queueSize).toBe(0);
    expect(chunks.length).toBe(1);

    await session.stop();
  });

  it('stops and returns a result with bytes and duration', async () => {
    const { stream } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    const session = await startRecording(canvas as unknown as HTMLCanvasElement, makeOptions(stream, undefined, 'video/webm'));

    const recorder = canvas.__stream.__recorder!;
    recorder.dispatchData(new Uint8Array([4, 5, 6, 7]));
    await new Promise((resolve) => setTimeout(resolve, 0));

    const result = await session.stop();
    expect(result.bytes).toBeGreaterThanOrEqual(4);
    expect(result.durationMs).toBeGreaterThanOrEqual(0);
    expect(result.filename).toBe('test-recording');
    expect(result.mimeType).toBe('video/webm');
  });

  it('honors segment duration by passing timeslice to MediaRecorder', async () => {
    const { stream } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    const session = await startRecording(
      canvas as unknown as HTMLCanvasElement,
      makeOptions(stream, 5000),
    );
    const recorder = canvas.__stream.__recorder!;
    expect(recorder.__timeslice).toBe(5000);
    await session.stop();
  });

  it('cancels without finalizing', async () => {
    const { stream, chunks } = makeTarget();
    const CanvasCtor = (globalThis as unknown as { HTMLCanvasElement: typeof MockHTMLCanvasElement }).HTMLCanvasElement;
    const canvas = new CanvasCtor();
    const session = await startRecording(canvas as unknown as HTMLCanvasElement, makeOptions(stream));

    const recorder = canvas.__stream.__recorder!;
    recorder.dispatchData(new Uint8Array([8, 9]));
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.cancel();
    const result = session.getStats();
    expect(result.bytesWritten).toBe(2);
    expect(chunks.length).toBe(1);
  });
});
