import { describe, it, expect } from 'vitest';
import { extractPlanarF32, type AudioFrame } from './format';

describe('extractPlanarF32', () => {
  it('copies planar f32 data', () => {
    const frame: AudioFrame = {
      timestamp: 0,
      sampleRate: 48000,
      channels: 2,
      numberOfFrames: 3,
      format: 'f32-planar',
      data: [new Float32Array([1, 2, 3]), new Float32Array([4, 5, 6])],
    };
    const out = extractPlanarF32(frame, 2);
    expect(out.length).toBe(2);
    expect(out.at(0)?.at(0)).toBeCloseTo(1);
    expect(out.at(1)?.at(2)).toBeCloseTo(6);
  });

  it('duplicates mono to stereo', () => {
    const frame: AudioFrame = {
      timestamp: 0,
      sampleRate: 48000,
      channels: 1,
      numberOfFrames: 3,
      format: 'f32-planar',
      data: [new Float32Array([1, 2, 3])],
    };
    const out = extractPlanarF32(frame, 2);
    expect(out.length).toBe(2);
    expect(out.at(1)?.at(1)).toBeCloseTo(2);
  });

  it('converts s16 planar', () => {
    const frame: AudioFrame = {
      timestamp: 0,
      sampleRate: 48000,
      channels: 1,
      numberOfFrames: 3,
      format: 's16-planar',
      copyTo: (dst: Float32Array | Int16Array | Uint8Array, { frameCount }) => {
        if (dst instanceof Int16Array && frameCount === 3) {
          dst[0] = -32768;
          dst[1] = 0;
          dst[2] = 32767;
        }
      },
    };
    const out = extractPlanarF32(frame, 1);
    expect(out.at(0)?.at(0)).toBeCloseTo(-1);
    expect(out.at(0)?.at(2)).toBeCloseTo(32767 / 32768);
  });

  it('converts u8 planar', () => {
    const frame: AudioFrame = {
      timestamp: 0,
      sampleRate: 48000,
      channels: 1,
      numberOfFrames: 2,
      format: 'u8',
      copyTo: (dst: Float32Array | Int16Array | Uint8Array, { frameCount }) => {
        if (dst instanceof Uint8Array && frameCount === 2) {
          dst[0] = 0;
          dst[1] = 255;
        }
      },
    };
    const out = extractPlanarF32(frame, 1);
    expect(out.at(0)?.at(0)).toBeCloseTo(-1);
    expect(out.at(0)?.at(1)).toBeCloseTo(127 / 128);
  });

  it('throws for missing data and copyTo', () => {
    const frame: AudioFrame = {
      timestamp: 0,
      sampleRate: 48000,
      channels: 1,
      numberOfFrames: 1,
      format: 'f32-planar',
    };
    expect(() => extractPlanarF32(frame, 1)).toThrow('must provide data or copyTo');
  });
});
