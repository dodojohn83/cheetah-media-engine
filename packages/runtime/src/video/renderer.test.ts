import { describe, it, expect, vi } from 'vitest';
import { RendererSurface } from './surface';
import { Canvas2DRenderer } from './canvas2d';
import { createRenderer, VideoRenderer } from './renderer';
import { buildYuvToRgbCoeffs, getColorRange, getYuvMatrix } from './color';
import type { RenderFrame, VisibleRect, RendererConfig } from './types';
import { RendererError } from './types';

function makeMockFrame(overrides?: Partial<RenderFrame>): RenderFrame {
  const visibleRect: VisibleRect = overrides?.visibleRect ?? { x: 0, y: 0, width: 64, height: 48 };
  const format = overrides?.format ?? 'rgba';
  return {
    timestamp: 0,
    codedWidth: 64,
    codedHeight: 48,
    visibleRect,
    format,
    colorSpace: { fullRange: false, matrix: 'bt.709' },
    close: vi.fn(),
    allocationSize: vi.fn(({ rect } = {}) => {
      const r = rect ?? visibleRect;
      if (format === 'rgba') return r.width * r.height * 4;
      if (format === 'i420') return r.width * r.height + 2 * Math.max(1, Math.floor(r.width / 2)) * Math.max(1, Math.floor(r.height / 2));
      return r.width * r.height * 4;
    }),
    copyTo: vi.fn(async (destination, { rect } = {}) => {
      const r = rect ?? visibleRect;
      const size = r.width * r.height * 4;
      if (destination.byteLength < size) throw new Error('destination too small');
      const arr = new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
      arr.fill(128);
      return size;
    }),
    ...overrides,
  };
}

function makeMockCanvas(width = 320, height = 240): HTMLCanvasElement {
  const drawImage = vi.fn();
  const clearRect = vi.fn();
  const setTransform = vi.fn();
  const translate = vi.fn();
  const rotate = vi.fn();
  const scale = vi.fn();
  const getImageData = vi.fn(() => ({ width: 0, height: 0, data: new Uint8ClampedArray(0) }));

  const ctx = {
    drawImage,
    clearRect,
    setTransform,
    translate,
    rotate,
    scale,
    getImageData,
  } as unknown as CanvasRenderingContext2D;

  const canvas = {
    width,
    height,
    getContext: vi.fn((type: string) => (type === '2d' ? ctx : null)),
  } as unknown as HTMLCanvasElement;

  return canvas;
}

describe('RendererSurface', () => {
  it('resizes by DPR and reports real pixel dimensions', () => {
    const canvas = makeMockCanvas(100, 100);
    const surface = new RendererSurface(canvas);
    surface.resize(200, 100);
    expect(canvas.width).toBe(200);
    expect(canvas.height).toBe(100);
    expect(surface.width).toBe(200);
    expect(surface.height).toBe(100);
  });

  it('applies DPR to canvas backing size', () => {
    const canvas = makeMockCanvas(100, 100);
    const surface = new RendererSurface(canvas);
    surface.configure({ canvas, fit: 'contain', dpr: 2 });
    surface.resize(100, 50);
    expect(canvas.width).toBe(200);
    expect(canvas.height).toBe(100);
  });

  it.each([
    { fit: 'contain' as const, frame: { w: 4, h: 3 }, expected: { w: 160, h: 120 } },
    { fit: 'cover' as const, frame: { w: 4, h: 3 }, expected: { w: 213, h: 160 } },
    { fit: 'stretch' as const, frame: { w: 4, h: 3 }, expected: { w: 160, h: 160 } },
  ])('computes viewport for fit=$fit', ({ fit, frame, expected }) => {
    const canvas = makeMockCanvas(160, 160);
    const surface = new RendererSurface(canvas);
    surface.configure({ canvas, fit });
    surface.resize(160, 160);
    const viewport = surface.computeViewport(frame.w, frame.h);
    expect(viewport.width).toBeCloseTo(expected.w, 0);
    expect(viewport.height).toBeCloseTo(expected.h, 0);
  });

  it('returns full visible rect by default', () => {
    const frame = { codedWidth: 100, codedHeight: 80 };
    const rect = RendererSurface.resolveVisibleRect(frame);
    expect(rect).toEqual({ x: 0, y: 0, width: 100, height: 80 });
  });

  it('rejects non-finite or non-positive canvas dimensions', () => {
    const badCanvas = { width: 0, height: -10, getContext: vi.fn(() => null) } as unknown as HTMLCanvasElement;
    expect(() => new RendererSurface(badCanvas)).toThrow(RendererError);
  });

  it('rejects non-canvas in constructor', () => {
    expect(() => new RendererSurface('not canvas' as unknown as HTMLCanvasElement)).toThrow(
      'canvas must be a canvas-like element',
    );
  });

  it('rejects invalid frames in resolveVisibleRect', () => {
    expect(() => RendererSurface.resolveVisibleRect(null as unknown as RenderFrame)).toThrow(
      'frame must be an object',
    );
    expect(() =>
      RendererSurface.resolveVisibleRect({ codedWidth: NaN, codedHeight: 48 } as unknown as RenderFrame),
    ).toThrow('codedWidth');
    expect(() =>
      RendererSurface.resolveVisibleRect({
        codedWidth: 64,
        codedHeight: 48,
        visibleRect: { x: 0, y: 0, width: -1, height: 48 },
      } as unknown as RenderFrame),
    ).toThrow('visibleRect');
  });
});

describe('Canvas2DRenderer', () => {
  it('configures and renders a frame', async () => {
    const canvas = makeMockCanvas(320, 240);
    const renderer = new Canvas2DRenderer(canvas);
    await renderer.configure({ canvas, fit: 'contain' });
    const frame = makeMockFrame({ format: 'rgba' });
    await renderer.render(frame);
    expect(frame.close).not.toHaveBeenCalled();
    const metrics = renderer.getMetrics();
    expect(metrics.framesRendered).toBe(1);
    expect(metrics.framesSubmitted).toBe(1);
  });

  it('throws if configured without a 2d context', async () => {
    const canvas = { width: 1, height: 1, getContext: vi.fn(() => null) } as unknown as HTMLCanvasElement;
    const renderer = new Canvas2DRenderer(canvas);
    await expect(renderer.configure({ canvas })).rejects.toThrow(RendererError);
  });

  it('counts dropped frames when drawImage fails', async () => {
    const canvas = makeMockCanvas(320, 240);
    const ctx = canvas.getContext('2d') as unknown as CanvasRenderingContext2D & { drawImage: ReturnType<typeof vi.fn> };
    ctx.drawImage.mockImplementation(() => {
      throw new Error('draw error');
    });
    const renderer = new Canvas2DRenderer(canvas);
    await renderer.configure({ canvas });
    const frame = makeMockFrame();
    await expect(renderer.render(frame)).rejects.toThrow(RendererError);
    expect(renderer.getMetrics().framesDropped).toBe(1);
  });

  it('returns a snapshot from getImageData', async () => {
    const canvas = makeMockCanvas(320, 240);
    const ctx = canvas.getContext('2d') as unknown as CanvasRenderingContext2D & { getImageData: ReturnType<typeof vi.fn> };
    ctx.getImageData.mockReturnValue({ width: 320, height: 240, data: new Uint8ClampedArray(320 * 240 * 4) });
    const renderer = new Canvas2DRenderer(canvas);
    await renderer.configure({ canvas });
    const snapshot = await renderer.snapshot();
    expect(snapshot.width).toBe(320);
    expect(snapshot.height).toBe(240);
    expect(renderer.getMetrics().snapshotsTaken).toBe(1);
  });
});

describe('VideoRenderer factory', () => {
  it('selects canvas2d when webgl2 is not available', async () => {
    const canvas = makeMockCanvas(320, 240);
    const renderer = createRenderer({ preferred: 'webgl2' });
    await renderer.configure({ canvas });
    expect(renderer.getMetrics().framesSubmitted).toBe(0);
  });

  it('renders a frame through the selected backend', async () => {
    const canvas = makeMockCanvas(320, 240);
    const renderer = new VideoRenderer({ preferred: 'canvas2d' });
    await renderer.configure({ canvas });
    const frame = makeMockFrame();
    await renderer.render(frame);
    expect(renderer.getMetrics().framesRendered).toBe(1);
    renderer.close();
  });

  it('rejects render before configure', async () => {
    const renderer = new VideoRenderer();
    const frame = makeMockFrame();
    await expect(renderer.render(frame)).rejects.toThrow(RendererError);
  });

  it('rejects invalid configure arguments', async () => {
    const renderer = new VideoRenderer();
    await expect(renderer.configure(undefined as unknown as RendererConfig)).rejects.toThrow(
      'renderer config must be an object',
    );
    await expect(renderer.configure({ canvas: null } as unknown as RendererConfig)).rejects.toThrow(
      'canvas must be a canvas-like element',
    );
  });

  it('rejects invalid snapshot options', async () => {
    const canvas = makeMockCanvas(320, 240);
    const ctx = canvas.getContext('2d') as unknown as CanvasRenderingContext2D & { getImageData: ReturnType<typeof vi.fn> };
    ctx.getImageData.mockReturnValue({ width: 320, height: 240, data: new Uint8ClampedArray(320 * 240 * 4) });
    const renderer = new VideoRenderer({ preferred: 'canvas2d' });
    await renderer.configure({ canvas });

    await expect(renderer.snapshot(null as unknown as Parameters<typeof renderer.snapshot>[0])).rejects.toThrow(
      RendererError,
    );
    await expect(renderer.snapshot({ format: 'gif' as 'png' })).rejects.toThrow('Unsupported snapshot format');
    await expect(renderer.snapshot({ maxWidth: -1 })).rejects.toThrow('snapshot maxWidth must be a finite positive number');
  });
});

describe('Color conversion', () => {
  it('defaults to BT.709 limited range', () => {
    const matrix = getYuvMatrix('unknown');
    expect(matrix.kr).toBeCloseTo(0.2126);
    const range = getColorRange(false);
    expect(range.yMin).toBe(16);
  });

  it('converts full-range black and white YUV', () => {
    const { coeffs, offset } = buildYuvToRgbCoeffs(getYuvMatrix('bt.709'), getColorRange(true));
    const c = coeffs;
    // coeffs are in column-major order: [Ry, Gy, By, Ru, Gu, Bu, Rv, Gv, Bv].
    const [rB, gB, bB] = [0, 1, 2].map((row) => c[row]! * 0 + c[row + 3]! * 0 + c[row + 6]! * 0 + offset[0]!);
    expect(rB).toBeCloseTo(0, 5);
    expect(gB).toBeCloseTo(0, 5);
    expect(bB).toBeCloseTo(0, 5);

    const [rW, gW, bW] = [0, 1, 2].map((row) => c[row]! * 1 + c[row + 3]! * 0 + c[row + 6]! * 0 + offset[0]!);
    expect(rW).toBeCloseTo(1, 5);
    expect(gW).toBeCloseTo(1, 5);
    expect(bW).toBeCloseTo(1, 5);
  });

  it('converts limited-range black and white YUV using raw normalized values', () => {
    const { coeffs, offset } = buildYuvToRgbCoeffs(getYuvMatrix('bt.709'), getColorRange(false));
    const c = coeffs;
    const blackY = 16 / 255;
    const whiteY = 235 / 255;
    const [rB, gB, bB] = [0, 1, 2].map((row) => c[row]! * blackY + c[row + 3]! * 0 + c[row + 6]! * 0 + offset[0]!);
    expect(rB).toBeCloseTo(0, 5);
    expect(gB).toBeCloseTo(0, 5);
    expect(bB).toBeCloseTo(0, 5);

    const [rW, gW, bW] = [0, 1, 2].map((row) => c[row]! * whiteY + c[row + 3]! * 0 + c[row + 6]! * 0 + offset[0]!);
    expect(rW).toBeCloseTo(1, 5);
    expect(gW).toBeCloseTo(1, 5);
    expect(bW).toBeCloseTo(1, 5);
  });

  it('converts full-range pure red to RGB', () => {
    const { coeffs, offset } = buildYuvToRgbCoeffs(getYuvMatrix('bt.709'), getColorRange(true));
    const c = coeffs;
    const kb = 0.0722;
    // For BT.709 full-range pure red: Y=Kr, Cb=-Y/(2*(1-Kb)), Cr=0.5.
    const y = 0.2126;
    const u = -y / (2 * (1 - kb));
    const v = 0.5;
    const rgb = [0, 1, 2].map((row) => c[row]! * y + c[row + 3]! * u + c[row + 6]! * v + offset[0]!);
    expect(rgb[0]).toBeCloseTo(1, 3);
    expect(rgb[1]).toBeCloseTo(0, 3);
    expect(rgb[2]).toBeCloseTo(0, 3);
  });
});
