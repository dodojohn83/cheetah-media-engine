/**
 * Snapshot encoder: converts the last rendered frame into a browser-supported
 * image blob (PNG/JPEG/WebP) without blocking the render queue.
 *
 * The heavy lifting (toBlob) is delegated to the browser's canvas encoder, which
 * runs off the main thread when the implementation supports it.
 */

import { RendererError } from './types';

export type SnapshotFormat = 'png' | 'jpeg' | 'webp';

export interface SnapshotEncoderOptions {
  /** Target image format. */
  readonly format?: SnapshotFormat | undefined;
  /** Compression quality for lossy formats, 0..1. */
  readonly quality?: number | undefined;
  /** Maximum width in CSS pixels; the image is scaled down preserving aspect ratio. */
  readonly maxWidth?: number | undefined;
  /** Maximum height in CSS pixels; the image is scaled down preserving aspect ratio. */
  readonly maxHeight?: number | undefined;
  /** Whether to include on-screen overlays (not yet implemented). */
  readonly includeOverlay?: boolean | undefined;
}

const FORMAT_TO_MIME: Record<SnapshotFormat, string> = {
  png: 'image/png',
  jpeg: 'image/jpeg',
  webp: 'image/webp',
};

export function formatToMime(format: SnapshotFormat): string | undefined {
  return FORMAT_TO_MIME[format];
}

const VALID_FORMATS = Object.keys(FORMAT_TO_MIME) as readonly string[];

type MutableSnapshotEncoderOptions = {
  -readonly [K in keyof SnapshotEncoderOptions]: SnapshotEncoderOptions[K];
};

export function validateSnapshotEncoderOptions(raw: unknown): SnapshotEncoderOptions {
  if (raw === undefined) return {};
  if (raw === null || typeof raw !== 'object') {
    throw new RendererError('invalid-option', 'snapshot options must be an object');
  }
  const opts = raw as Record<string, unknown>;
  const result: MutableSnapshotEncoderOptions = {};

  if ('format' in opts && opts.format !== undefined) {
    if (typeof opts.format !== 'string' || !VALID_FORMATS.includes(opts.format)) {
      throw new RendererError('invalid-option', `Unsupported snapshot format: ${opts.format}`);
    }
    result.format = opts.format as SnapshotFormat;
  }

  if ('quality' in opts && opts.quality !== undefined && result.format !== 'png') {
    if (typeof opts.quality !== 'number' || !Number.isFinite(opts.quality) || opts.quality < 0 || opts.quality > 1) {
      throw new RendererError('invalid-option', 'snapshot quality must be a finite number between 0 and 1');
    }
    result.quality = opts.quality;
  }

  if ('maxWidth' in opts && opts.maxWidth !== undefined) {
    if (typeof opts.maxWidth !== 'number' || !Number.isFinite(opts.maxWidth) || opts.maxWidth <= 0) {
      throw new RendererError('invalid-option', 'snapshot maxWidth must be a finite positive number');
    }
    result.maxWidth = opts.maxWidth;
  }

  if ('maxHeight' in opts && opts.maxHeight !== undefined) {
    if (typeof opts.maxHeight !== 'number' || !Number.isFinite(opts.maxHeight) || opts.maxHeight <= 0) {
      throw new RendererError('invalid-option', 'snapshot maxHeight must be a finite positive number');
    }
    result.maxHeight = opts.maxHeight;
  }

  if ('includeOverlay' in opts && opts.includeOverlay !== undefined) {
    if (typeof opts.includeOverlay !== 'boolean') {
      throw new RendererError('invalid-option', 'snapshot includeOverlay must be a boolean');
    }
    result.includeOverlay = opts.includeOverlay;
  }

  return result;
}

export function computeTargetSize(
  sourceWidth: number,
  sourceHeight: number,
  maxWidth?: number,
  maxHeight?: number,
): { width: number; height: number } {
  if (!Number.isFinite(sourceWidth) || sourceWidth <= 0 || !Number.isFinite(sourceHeight) || sourceHeight <= 0) {
    throw new RendererError('invalid-source', 'Snapshot source dimensions must be finite and positive');
  }
  let width = sourceWidth;
  let height = sourceHeight;
  const limitWidth = maxWidth !== undefined && Number.isFinite(maxWidth) && maxWidth > 0 ? maxWidth : undefined;
  const limitHeight = maxHeight !== undefined && Number.isFinite(maxHeight) && maxHeight > 0 ? maxHeight : undefined;
  if (limitWidth !== undefined && width > limitWidth) {
    const scale = limitWidth / width;
    width = limitWidth;
    height = Math.max(1, Math.round(height * scale));
  }
  if (limitHeight !== undefined && height > limitHeight) {
    const scale = limitHeight / height;
    height = limitHeight;
    width = Math.max(1, Math.round(width * scale));
  }
  return { width, height };
}

export interface CanvasLike {
  width: number;
  height: number;
  getContext(contextId: '2d'): CanvasRenderingContext2DLike | null;
  toBlob(callback: (blob: Blob | null) => void, type?: string, quality?: number): void;
}

export interface CanvasRenderingContext2DLike {
  putImageData(imagedata: ImageData, dx: number, dy: number): void;
  drawImage(image: unknown, dx: number, dy: number, dw: number, dh: number): void;
}

function createCanvas(width: number, height: number): CanvasLike {
  if (typeof document !== 'undefined' && typeof document.createElement === 'function') {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    return canvas as unknown as CanvasLike;
  }
  if (typeof OffscreenCanvas !== 'undefined') {
    const canvas = new OffscreenCanvas(width, height);
    const offscreen = canvas as unknown as {
      toBlob?: (callback: (blob: Blob | null) => void, type?: string, quality?: number) => void;
    };
    if (typeof offscreen.toBlob !== 'function') {
      offscreen.toBlob = (callback, type, quality): void => {
        const opts: { type?: string; quality?: number } = {};
        if (type !== undefined) opts.type = type;
        if (quality !== undefined) opts.quality = quality;
        canvas
          .convertToBlob(opts)
          .then((blob) => callback(blob))
          .catch(() => callback(null));
      };
    }
    return canvas as unknown as CanvasLike;
  }
  throw new RendererError('unsupported', 'No canvas implementation available for snapshot encoding');
}

function isImageData(source: unknown): source is ImageData {
  return (
    (typeof ImageData !== 'undefined' && source instanceof ImageData) ||
    (source !== null &&
      typeof source === 'object' &&
      'data' in source &&
      (source as { data?: unknown }).data instanceof Uint8ClampedArray &&
      'width' in source &&
      'height' in source)
  );
}

function isCanvasLike(source: unknown): source is CanvasLike {
  return source !== null && typeof source === 'object' && typeof (source as { getContext?: unknown }).getContext === 'function';
}

/**
 * Encode a still image into a Blob of the requested format.
 *
 * @param source An `ImageData`, `HTMLCanvasElement`, `OffscreenCanvas` or
 *               duck-typed canvas object.
 * @param options Encoding options.
 */
export async function encodeSnapshot(
  source: ImageData | CanvasLike,
  rawOptions: SnapshotEncoderOptions = {},
): Promise<Blob> {
  const options = validateSnapshotEncoderOptions(rawOptions);
  const format = options.format ?? 'png';
  const mimeType = formatToMime(format);
  if (!mimeType) {
    throw new RendererError('unsupported', `Unsupported snapshot format: ${format}`);
  }

  const sourceWidth = source.width;
  const sourceHeight = source.height;
  const { width, height } = computeTargetSize(sourceWidth, sourceHeight, options.maxWidth, options.maxHeight);

  const canvas = createCanvas(width, height);
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    throw new RendererError('no-context', 'Cannot create 2D context for snapshot encoding');
  }

  if (isImageData(source)) {
    if (width === sourceWidth && height === sourceHeight) {
      ctx.putImageData(source, 0, 0);
    } else {
      // putImageData copies pixels 1:1 and does not scale. Draw the raw pixels
      // onto an intermediate source-sized canvas and then scale to the target.
      const srcCanvas = createCanvas(sourceWidth, sourceHeight);
      const srcCtx = srcCanvas.getContext('2d');
      if (!srcCtx) {
        throw new RendererError('no-context', 'Cannot create intermediate 2D context for snapshot scaling');
      }
      srcCtx.putImageData(source, 0, 0);
      ctx.drawImage(srcCanvas, 0, 0, width, height);
    }
  } else if (isCanvasLike(source)) {
    ctx.drawImage(source, 0, 0, width, height);
  } else {
    throw new RendererError('invalid-source', 'Snapshot source must be ImageData or a canvas');
  }

  if (options.includeOverlay) {
    // Overlays are not yet supported; silently ignored to avoid failing the
    // whole snapshot when a caller requests them.
  }

  return new Promise((resolve, reject) => {
    if (typeof canvas.toBlob !== 'function') {
      reject(new RendererError('unsupported', 'Canvas toBlob not supported'));
      return;
    }
    const onBlob = (blob: Blob | null): void => {
      if (!blob) {
        reject(new RendererError('encoding-failed', `Canvas toBlob returned null for ${mimeType}`));
        return;
      }
      resolve(blob);
    };
    const quality = options.quality;
    if (quality !== undefined && format !== 'png') {
      if (!Number.isFinite(quality)) {
        reject(new RendererError('invalid-option', 'quality must be a finite number'));
        return;
      }
      canvas.toBlob(onBlob, mimeType, Math.min(1, Math.max(0, quality)));
    } else {
      canvas.toBlob(onBlob, mimeType);
    }
  });
}
