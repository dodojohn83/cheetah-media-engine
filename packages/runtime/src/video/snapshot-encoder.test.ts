import { describe, it, expect } from 'vitest';
import {
  encodeSnapshot,
  formatToMime,
  computeTargetSize,
  type CanvasLike,
} from './snapshot-encoder';
import { RendererError } from './types';

if (typeof ImageData === 'undefined') {
  (globalThis as unknown as { ImageData: typeof ImageData }).ImageData = class {
    constructor(
      public data: Uint8ClampedArray,
      public width: number,
      public height: number,
    ) {}
  } as unknown as typeof ImageData;
}

interface FakeCanvasStorage {
  putImageDataCalls: unknown[];
  drawImageCalls: unknown[];
  toBlobCalls: { type: string | undefined; quality: number | undefined }[];
}

function makeFakeOffscreenCanvas(): void {
  if (typeof OffscreenCanvas !== 'undefined') return;
  const instances: FakeOffscreenCanvas[] = [];
  class FakeOffscreenCanvas {
    public static readonly __instances = instances;
    public readonly __calls: FakeCanvasStorage;
    constructor(
      public width: number,
      public height: number,
    ) {
      this.__calls = { putImageDataCalls: [], drawImageCalls: [], toBlobCalls: [] };
      instances.push(this);
    }

    getContext(_id: string): { putImageData: (data: unknown) => void; drawImage: (image: unknown) => void } | null {
      const storage = this.__calls;
      return {
        putImageData(data: unknown) {
          storage.putImageDataCalls.push(data);
        },
        drawImage(image: unknown) {
          storage.drawImageCalls.push(image);
        },
      };
    }

    convertToBlob(options?: { type?: string; quality?: number }): Promise<Blob> {
      this.__calls.toBlobCalls.push({ type: options?.type, quality: options?.quality });
      const data = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
      return Promise.resolve(new Blob([data], { type: options?.type ?? 'image/png' }));
    }

    toBlob(callback: (blob: Blob | null) => void, type?: string, quality?: number): void {
      this.__calls.toBlobCalls.push({ type, quality });
      const data = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
      callback(new Blob([data], { type: type ?? 'image/png' }));
    }
  }
  (globalThis as unknown as { OffscreenCanvas: unknown }).OffscreenCanvas = FakeOffscreenCanvas;
}

makeFakeOffscreenCanvas();

function pushDrawn(
  drawn: { source?: unknown; type?: string | undefined; quality?: number | undefined }[],
  type?: string,
  quality?: number,
): void {
  const entry: { source?: unknown; type?: string | undefined; quality?: number | undefined } = {};
  if (type !== undefined) entry.type = type;
  if (quality !== undefined) entry.quality = quality;
  drawn.push(entry);
}

function makeMockCanvas(width: number, height: number, drawn: { source?: unknown; type?: string | undefined; quality?: number | undefined }[]): CanvasLike {
  const calls = {
    putImageData: undefined as unknown,
    drawImage: undefined as unknown,
  };
  const ctx = {
    putImageData(imagedata: unknown) {
      calls.putImageData = imagedata;
    },
    drawImage(image: unknown, _dx: number, _dy: number, dw: number, dh: number) {
      calls.drawImage = { image, dw, dh };
    },
  };
  const canvas = {
    width,
    height,
    getContext(id: string) {
      return id === '2d' ? ctx : null;
    },
    toBlob(callback: (blob: Blob | null) => void, type?: string, quality?: number) {
      pushDrawn(drawn, type, quality);
      const data = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
      callback(new Blob([data], { type: type ?? 'image/png' }));
    },
  } as unknown as CanvasLike;
  (canvas as unknown as { __calls: unknown }).__calls = calls;
  return canvas;
}

describe('computeTargetSize', () => {
  it('returns source size when no limits are given', () => {
    expect(computeTargetSize(1920, 1080)).toEqual({ width: 1920, height: 1080 });
  });

  it('scales down to fit max width', () => {
    expect(computeTargetSize(1920, 1080, 960)).toEqual({ width: 960, height: 540 });
  });

  it('scales down to fit max height', () => {
    expect(computeTargetSize(1920, 1080, undefined, 540)).toEqual({ width: 960, height: 540 });
  });

  it('scales down to fit both dimensions', () => {
    expect(computeTargetSize(1920, 1080, 480, 480)).toEqual({ width: 480, height: 270 });
  });

  it('ignores non-positive limits', () => {
    expect(computeTargetSize(1920, 1080, 0, 0)).toEqual({ width: 1920, height: 1080 });
  });
});

describe('formatToMime', () => {
  it('maps snapshot formats to MIME types', () => {
    expect(formatToMime('png')).toBe('image/png');
    expect(formatToMime('jpeg')).toBe('image/jpeg');
    expect(formatToMime('webp')).toBe('image/webp');
  });
});

function getLastFakeCanvas(): FakeCanvasStorage | undefined {
  const ctor = (globalThis as unknown as { OffscreenCanvas?: { __instances?: { __calls: FakeCanvasStorage }[] } })
    .OffscreenCanvas;
  const instances = ctor?.__instances;
  if (!instances || instances.length === 0) return undefined;
  const instance = instances[instances.length - 1];
  return instance?.__calls;
}

describe('encodeSnapshot', () => {
  it('encodes an ImageData source with default PNG format', async () => {
    const image = new ImageData(new Uint8ClampedArray(4 * 2 * 2), 2, 2);
    const blob = await encodeSnapshot(image, { maxWidth: 4, maxHeight: 4 });
    expect(blob).toBeInstanceOf(Blob);
    expect(blob.type).toBe('image/png');

    const calls = getLastFakeCanvas();
    expect(calls).toBeDefined();
    expect(calls!.putImageDataCalls.length).toBe(1);
    expect(calls!.toBlobCalls[0]?.type).toBe('image/png');
  });

  it('encodes a canvas source with requested JPEG quality', async () => {
    const drawn: { source?: unknown; type?: string | undefined; quality?: number | undefined }[] = [];
    const canvas = makeMockCanvas(200, 100, drawn);
    const blob = await encodeSnapshot(canvas, { format: 'jpeg', quality: 0.5, maxWidth: 100 });
    expect(blob).toBeInstanceOf(Blob);

    const calls = getLastFakeCanvas();
    expect(calls).toBeDefined();
    expect(calls!.drawImageCalls.length).toBe(1);
    expect(calls!.toBlobCalls[0]?.type).toBe('image/jpeg');
    expect(calls!.toBlobCalls[0]?.quality).toBe(0.5);
  });

  it('rejects unsupported formats', async () => {
    const drawn: { source?: unknown; type?: string | undefined; quality?: number | undefined }[] = [];
    const canvas = makeMockCanvas(10, 10, drawn);
    // Cast to avoid TypeScript rejecting the invalid format at compile time.
    await expect(encodeSnapshot(canvas, { format: 'gif' as 'png' })).rejects.toBeInstanceOf(RendererError);
  });

  it('ignores quality for PNG', async () => {
    const drawn: { source?: unknown; type?: string | undefined; quality?: number | undefined }[] = [];
    const canvas = makeMockCanvas(10, 10, drawn);
    await encodeSnapshot(canvas, { format: 'png', quality: 0.5 });

    const calls = getLastFakeCanvas();
    expect(calls).toBeDefined();
    expect(calls!.toBlobCalls[0]?.type).toBe('image/png');
    expect(calls!.toBlobCalls[0]?.quality).toBeUndefined();
  });

  it('scales ImageData down using an intermediate canvas', async () => {
    const ctor = (globalThis as unknown as { OffscreenCanvas?: { __instances?: { width: number; height: number; __calls: FakeCanvasStorage }[] } })
      .OffscreenCanvas;
    const before = ctor?.__instances?.length ?? 0;
    const image = new ImageData(new Uint8ClampedArray(4 * 8 * 8), 8, 8);
    await encodeSnapshot(image, { format: 'png', maxWidth: 4, maxHeight: 4 });

    const instances = ctor?.__instances;
    expect(instances).toBeDefined();
    const created = instances!.slice(before);
    expect(created.length).toBe(2);
    // encodeSnapshot creates the target canvas first, then a source-sized
    // intermediate canvas to draw/scale the ImageData.
    const targetCanvas = created[0];
    const sourceCanvas = created[1];
    expect(sourceCanvas?.width).toBe(8);
    expect(sourceCanvas?.height).toBe(8);
    expect(targetCanvas?.width).toBe(4);
    expect(targetCanvas?.height).toBe(4);
    expect(targetCanvas?.__calls.drawImageCalls.length).toBe(1);
  });
});
