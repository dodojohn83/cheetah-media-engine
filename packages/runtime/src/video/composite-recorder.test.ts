import { describe, it, expect, beforeEach } from 'vitest';
import { CompositeRecorder, type CompositeRecordingOptions, type CompositeWatermark } from './composite-recorder';
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
  constructor(public readonly tracks: unknown[] = []) {}

  getVideoTracks(): unknown[] {
    return this.tracks.filter((t) => (t as { kind?: string }).kind === 'video');
  }

  getAudioTracks(): unknown[] {
    return this.tracks.filter((t) => (t as { kind?: string }).kind === 'audio');
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
    (stream as unknown as { __recorder?: MockMediaRecorder }).__recorder = this;
  }

  start(timeslice?: number): void {
    this.state = 'recording';
    this.__timeslice = timeslice;
  }

  stop(): void {
    this.state = 'inactive';
    this.onstop?.();
  }

  pause(): void {
    if (this.state === 'recording') this.state = 'paused';
  }

  resume(): void {
    if (this.state === 'paused') this.state = 'recording';
  }

  dispatchData(bytes: Uint8Array): void {
    this.ondataavailable?.({ data: new MockBlob(bytes, this.mimeType) as unknown as Blob });
  }

  static isTypeSupported(_type: string): boolean {
    return true;
  }
}

function createMockCanvas(width: number, height: number): HTMLCanvasElement {
  const calls: { method: string; args: unknown[] }[] = [];
  const ctx = {
    clearRect: (...args: unknown[]) => calls.push({ method: 'clearRect', args }),
    drawImage: (...args: unknown[]) => calls.push({ method: 'drawImage', args }),
    putImageData: (...args: unknown[]) => calls.push({ method: 'putImageData', args }),
    fillText: (...args: unknown[]) => calls.push({ method: 'fillText', args }),
    get font() {
      return '';
    },
    set font(_value: string) {},
    get fillStyle() {
      return '';
    },
    set fillStyle(_value: string) {},
  };
  const stream = new MockMediaStream([{ kind: 'video' }]);
  const canvas = {
    width,
    height,
    getContext: (type: string) => {
      if (type === '2d') return ctx;
      return null;
    },
    captureStream: (_fps?: number) => stream,
    __calls: calls,
    __ctx: ctx,
  };
  return canvas as unknown as HTMLCanvasElement;
}

let lastCanvas: HTMLCanvasElement | undefined;
let rafId = 0;
const rafCallbacks = new Map<number, FrameRequestCallback>();

export function flushRaf(): void {
  const callbacks = Array.from(rafCallbacks.entries());
  rafCallbacks.clear();
  for (const [, cb] of callbacks) {
    cb(performance.now());
  }
}

function installMocks(): void {
  (globalThis as unknown as { MediaStream: unknown }).MediaStream = MockMediaStream;
  (globalThis as unknown as { MediaRecorder: unknown }).MediaRecorder = MockMediaRecorder;

  rafId = 0;
  rafCallbacks.clear();
  (globalThis as unknown as { requestAnimationFrame: typeof requestAnimationFrame }).requestAnimationFrame = (
    callback: FrameRequestCallback,
  ): number => {
    rafId += 1;
    rafCallbacks.set(rafId, callback);
    return rafId;
  };
  (globalThis as unknown as { cancelAnimationFrame: typeof cancelAnimationFrame }).cancelAnimationFrame = (id: number): void => {
    rafCallbacks.delete(id);
  };

  (globalThis as unknown as { document: Document }).document = {
    createElement: (tagName: string) => {
      if (tagName === 'canvas') {
        lastCanvas = createMockCanvas(640, 480);
        return lastCanvas;
      }
      return {};
    },
  } as unknown as Document;
}

function makeOptions(
  source: CanvasImageSource | ImageData,
  watermark?: CompositeWatermark,
  extra?: Partial<CompositeRecordingOptions>,
): CompositeRecordingOptions {
  return {
    source,
    filename: 'test',
    ...(watermark !== undefined ? { watermark } : {}),
    ...(extra ?? {}),
  };
}

class MockVideoElement {
  public videoWidth = 640;
  public videoHeight = 480;
}

class MockImageElement {
  public complete = true;
  public naturalWidth = 100;
  public naturalHeight = 50;
  public width = 100;
  public height = 50;
}

describe('CompositeRecorder', () => {
  beforeEach(() => {
    installMocks();
    lastCanvas = undefined;
  });

  it('records a non-empty blob from a video source', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const recorder = new CompositeRecorder();
    const startPromise = recorder.start(makeOptions(source));

    const canvas = lastCanvas!;
    const mockStream = canvas.captureStream() as unknown as MockMediaStream;
    const mediaRecorder = mockStream as unknown as { __recorder?: MockMediaRecorder };
    await startPromise;
    flushRaf();

    mediaRecorder.__recorder?.dispatchData(new Uint8Array([1, 2, 3, 4]));
    const result = await recorder.stop();

    expect(result.bytes).toBeGreaterThanOrEqual(4);
    expect(result.blob.size).toBeGreaterThanOrEqual(4);
    expect(result.mimeType).toBeTruthy();
    expect(recorder.recordingActive).toBe(false);
  });

  it('draws a text watermark onto the canvas', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const watermark: CompositeWatermark = { type: 'text', text: 'Demo', x: 10, y: 20 };
    const recorder = new CompositeRecorder();
    await recorder.start(makeOptions(source, watermark));
    flushRaf();
    const canvas = lastCanvas! as unknown as { __calls: { method: string; args: unknown[] }[] };
    await recorder.stop();

    const fillTextCalls = canvas.__calls.filter((c) => c.method === 'fillText');
    expect(fillTextCalls.length).toBeGreaterThan(0);
    expect(fillTextCalls[0]?.args[0]).toBe('Demo');
  });

  it('draws an image watermark onto the canvas', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const image = new MockImageElement() as unknown as CanvasImageSource;
    const watermark: CompositeWatermark = { type: 'image', image, x: 5, y: 5, width: 50, height: 25 };
    const recorder = new CompositeRecorder();
    await recorder.start(makeOptions(source, watermark));
    flushRaf();
    const canvas = lastCanvas! as unknown as { __calls: { method: string; args: unknown[] }[] };
    await recorder.stop();

    const drawImageCalls = canvas.__calls.filter((c) => c.method === 'drawImage');
    // First drawImage draws the source, subsequent ones draw the watermark.
    expect(drawImageCalls.length).toBeGreaterThanOrEqual(2);
  });

  it('pauses and resumes without throwing', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const recorder = new CompositeRecorder();
    await recorder.start(makeOptions(source));
    expect(recorder.recordingActive).toBe(true);

    recorder.pause();
    expect(recorder.progress.state).toBe('paused');

    recorder.resume();
    expect(recorder.recordingActive).toBe(true);
    flushRaf();

    const mockStream = lastCanvas!.captureStream() as unknown as MockMediaStream & { __recorder?: MockMediaRecorder };
    expect(mockStream.__recorder).toBeDefined();
    mockStream.__recorder!.dispatchData(new Uint8Array([5, 6]));
    const result = await recorder.stop();
    expect(result.bytes).toBeGreaterThanOrEqual(2);
  });

  it('reports live bytes written in progress', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const recorder = new CompositeRecorder();
    await recorder.start(makeOptions(source));
    flushRaf();

    const mockStream = lastCanvas!.captureStream() as unknown as MockMediaStream & { __recorder?: MockMediaRecorder };
    mockStream.__recorder!.dispatchData(new Uint8Array([1, 2, 3]));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(recorder.progress.bytesWritten).toBeGreaterThanOrEqual(3);
    await recorder.stop();
  });

  it('stops automatically when maxDurationMs is exceeded', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const recorder = new CompositeRecorder();
    await recorder.start(makeOptions(source, undefined, { maxDurationMs: 100 }));
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(recorder.recordingActive).toBe(false);
    expect(recorder.progress.state).toBe('stopped');
  });

  it('throws when the source size cannot be determined', async () => {
    const recorder = new CompositeRecorder();
    await expect(recorder.start(makeOptions({} as CanvasImageSource))).rejects.toBeInstanceOf(RendererError);
  });

  it('throws with non-finite or non-positive fps', async () => {
    const source = new MockVideoElement() as unknown as HTMLVideoElement;
    const recorder = new CompositeRecorder();
    await expect(recorder.start(makeOptions(source, undefined, { fps: NaN }))).rejects.toBeInstanceOf(RendererError);
  });
});
