import { describe, it, expect } from 'vitest';
import { AudioResampler } from './resampler';

function makeConstantFrames(channels: number, frames: number, value: number): Float32Array[] {
  const result: Float32Array[] = [];
  for (let c = 0; c < channels; c += 1) {
    result.push(new Float32Array(frames).fill(value));
  }
  return result;
}

describe('AudioResampler', () => {
  it('passes 48k->48k 1:1 for constant input', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 48000,
      outputSampleRate: 48000,
      channels: 2,
    });
    const input = makeConstantFrames(2, 100, 0.5);
    const output = resampler.push(input);
    expect(output.length).toBe(2);
    expect(output.at(0)?.length).toBe(99);
    expect(output.at(0)?.at(0)).toBeCloseTo(0.5);
  });

  it('converts 44.1k to 48k producing more output frames', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 44100,
      outputSampleRate: 48000,
      channels: 1,
    });
    const input = [new Float32Array(4410).fill(1)];
    const output = resampler.push(input);
    expect(output.at(0)?.length ?? 0).toBeGreaterThan(4400);
  });

  it('flushes remaining samples', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 48000,
      outputSampleRate: 48000,
      channels: 2,
    });
    resampler.push(makeConstantFrames(2, 10, 0.25));
    const flushed = resampler.flush();
    expect(flushed.length).toBe(2);
    expect(flushed.at(0)?.length ?? 0).toBeGreaterThan(0);
  });

  it('limits ratio change speed', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 48000,
      outputSampleRate: 48000,
      channels: 1,
      minRatio: 0.9,
      maxRatio: 1.1,
      maxRatioDelta: 0.05,
    });
    resampler.setRatio(1.2);
    expect(resampler.currentRatio).toBeCloseTo(1.05);
  });

  it('clamps ratio relative to the base ratio', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 44100,
      outputSampleRate: 48000,
      channels: 1,
      minRatio: 0.95,
      maxRatio: 1.05,
      maxRatioDelta: 1,
    });
    resampler.setRatio(2);
    expect(resampler.currentRatio).toBeCloseTo((44100 / 48000) * 1.05, 2);
  });

  it('resets ratio to default after reset', () => {
    const resampler = new AudioResampler({
      inputSampleRate: 44100,
      outputSampleRate: 48000,
      channels: 1,
    });
    resampler.setRatio(2);
    resampler.reset();
    expect(resampler.currentRatio).toBeCloseTo(44100 / 48000);
  });

  it('rejects invalid constructor options', () => {
    expect(() => new AudioResampler({ inputSampleRate: 0, outputSampleRate: 48000, channels: 1 })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: NaN, channels: 1 })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 0, channels: 1 })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 48000, channels: 0 })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 48000, channels: -1 })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 48000, channels: 1, minRatio: NaN })).toThrow();
    expect(() => new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 48000, channels: 1, minRatio: 1.1, maxRatio: 1.0 })).toThrow();
  });

  it('rejects non-Float32Array or non-array push input', () => {
    const resampler = new AudioResampler({ inputSampleRate: 48000, outputSampleRate: 48000, channels: 1 });
    expect(() => resampler.push(null as unknown as Float32Array[])).toThrow('push frames must be an array');
    expect(() => resampler.push([new Int16Array([1, 2]) as unknown as Float32Array])).toThrow(
      'push frames must contain only Float32Array instances',
    );
  });
});
