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
  const m = matrix?.toLowerCase() ?? '';
  if (m === 'bt.601' || m === 'bt601' || m === 'smpte170m') {
    return { kr: 0.299, kg: 0.587, kb: 0.114 };
  }
  // Default to BT.709.
  return { kr: 0.2126, kg: 0.7152, kb: 0.0722 };
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
  const { kr, kg, kb } = matrix;
  // Y is normalized to [0,1] from its raw range; Cb/Cr are expanded from the
  // [0,1] texture value centered at 0.5 to their legal [-1,1] or full-range
  // [-0.5,0.5] excursion.
  const yScale = 255 / (range.yMax - range.yMin);
  const yShift = range.yMin / 255;
  const cScale = 255 / (range.cMax - range.cZero);

  const rV = 2 * (1 - kr) * cScale;
  const bU = 2 * (1 - kb) * cScale;
  const gU = (-2 * (1 - kb) * kb * cScale) / kg;
  const gV = (-2 * (1 - kr) * kr * cScale) / kg;
  const yOff = -(yShift * yScale);

  // Columns of the matrix correspond to [y, u, v]; rows are [R, G, B].
  const col0 = [yScale, yScale, yScale];
  const col1 = [0, gU, bU];
  const col2 = [rV, gV, 0];
  const coeffs: number[] = [];
  for (let row = 0; row < 3; row += 1) {
    coeffs.push(col0[row]!);
    coeffs.push(col1[row]!);
    coeffs.push(col2[row]!);
  }
  return { coeffs, offset: [yOff, yOff, yOff] };
}

export function resolveColorSpace(info?: ColorSpaceInfo): { matrix: YuvMatrix; range: ColorRange } {
  const matrix = getYuvMatrix(info?.matrix);
  const fullRange = info?.fullRange ?? false;
  return { matrix, range: getColorRange(fullRange) };
}
