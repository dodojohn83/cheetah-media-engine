/**
 * Color-space conversion helpers for YUV -> RGB.
 *
 * Supports BT.601 and BT.709 with limited and full range. Unknown metadata is
 * treated as BT.709 limited range, which is the most common default for modern
 * HD content.
 */

import type { ColorSpaceInfo } from './types';

export interface ColorRange {
  /** Minimum raw Y value (e.g. 16 for limited range). */
  readonly yMin: number;
  /** Maximum raw Y value (e.g. 235 for limited range). */
  readonly yMax: number;
  /** Raw neutral chroma value (128 for 8-bit). */
  readonly cZero: number;
  /** Maximum raw chroma value (240 for limited range). */
  readonly cMax: number;
}

export function getColorRange(fullRange: boolean): ColorRange {
  if (typeof fullRange !== 'boolean') {
    throw new Error('fullRange must be a boolean');
  }
  if (fullRange) {
    return { yMin: 0, yMax: 255, cZero: 128, cMax: 255 };
  }
  return { yMin: 16, yMax: 235, cZero: 128, cMax: 240 };
}

export interface YuvMatrix {
  readonly kr: number;
  readonly kg: number;
  readonly kb: number;
}

export function getYuvMatrix(matrix?: string): YuvMatrix {
  const m = typeof matrix === 'string' ? matrix.toLowerCase() : '';
  if (m === 'bt.601' || m === 'bt601' || m === 'smpte170m') {
    return { kr: 0.299, kg: 0.587, kb: 0.114 };
  }
  // Default to BT.709.
  return { kr: 0.2126, kg: 0.7152, kb: 0.0722 };
}

function validateYuvMatrix(matrix: YuvMatrix): void {
  if (!matrix || typeof matrix !== 'object') {
    throw new Error('matrix must be an object');
  }
  if (!Number.isFinite(matrix.kr) || !Number.isFinite(matrix.kg) || !Number.isFinite(matrix.kb)) {
    throw new Error('matrix.kr, matrix.kg and matrix.kb must be finite numbers');
  }
  if (matrix.kg === 0) {
    throw new Error('matrix.kg must not be zero');
  }
}

function validateColorRange(range: ColorRange): void {
  if (!range || typeof range !== 'object') {
    throw new Error('range must be an object');
  }
  if (
    !Number.isFinite(range.yMin) ||
    !Number.isFinite(range.yMax) ||
    !Number.isFinite(range.cZero) ||
    !Number.isFinite(range.cMax)
  ) {
    throw new Error('range.yMin, range.yMax, range.cZero and range.cMax must be finite numbers');
  }
  if (range.yMax <= range.yMin) {
    throw new Error('range.yMax must be greater than range.yMin');
  }
  if (range.cMax <= range.yMin) {
    throw new Error('range.cMax must be greater than range.yMin');
  }
}

/**
 * Build the column-major 3x3 GLSL matrix and constant offset that converts
 * raw normalized YUV data to RGB.
 *
 * The shader performs `u_matrix * vec3(y, u - 0.5, v - 0.5) + u_offset`, where
 * `y` is in [0,1] and `u`/`v` are sampled from [0,1] textures then shifted to
 * be centered at 0. The returned coefficients absorb limited/full range and the
 * BT.601/709 primaries so the output is linear 0..1 RGB.
 */
export function buildYuvToRgbCoeffs(matrix: YuvMatrix, range: ColorRange): { coeffs: number[]; offset: number[] } {
  validateYuvMatrix(matrix);
  validateColorRange(range);
  const { kr, kg, kb } = matrix;
  // Y is normalized to [0,1] from its raw range; Cb/Cr are centered at cZero and
  // scaled to [-0.5,0.5] over the full chroma excursion [yMin, cMax].
  const yScale = 255 / (range.yMax - range.yMin);
  const yShift = range.yMin / 255;
  const cScale = 255 / (range.cMax - range.yMin);

  const rV = 2 * (1 - kr) * cScale;
  const bU = 2 * (1 - kb) * cScale;
  const gU = (-2 * (1 - kb) * kb * cScale) / kg;
  const gV = (-2 * (1 - kr) * kr * cScale) / kg;
  const yOff = -(yShift * yScale);

  // Columns of the matrix correspond to [y, u, v]; rows are [R, G, B].
  // uniformMatrix3fv expects column-major order, so we emit columns verbatim.
  const col0 = [yScale, yScale, yScale];
  const col1 = [0, gU, bU];
  const col2 = [rV, gV, 0];
  const coeffs: number[] = [...col0, ...col1, ...col2];
  return { coeffs, offset: [yOff, yOff, yOff] };
}

export function resolveColorSpace(info?: ColorSpaceInfo): { matrix: YuvMatrix; range: ColorRange } {
  const matrix = getYuvMatrix(info?.matrix);
  const fullRange = info?.fullRange ?? false;
  return { matrix, range: getColorRange(fullRange) };
}
