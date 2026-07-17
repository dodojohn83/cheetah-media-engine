import { describe, it, expect } from 'vitest';
import { SharedAudioRingBuffer, LocalAudioRingBuffer } from './ring';

describe('SharedAudioRingBuffer', () => {
  it('writes and reads back planar frames', () => {
    const sab = new SharedArrayBuffer(32 + 16 * 2 * 4);
    const ring = new SharedAudioRingBuffer(sab, 16, 2);
    const written = ring.write([new Float32Array([1, 2, 3]), new Float32Array([4, 5, 6])]);
    expect(written).toBe(3);
    const read = ring.read(3);
    expect(read.at(0)?.at(0)).toBeCloseTo(1);
    expect(read.at(0)?.at(1)).toBeCloseTo(2);
    expect(read.at(1)?.at(0)).toBeCloseTo(4);
  });

  it('drops frames when full', () => {
    const sab = new SharedArrayBuffer(32 + 4 * 1 * 4);
    const ring = new SharedAudioRingBuffer(sab, 4, 1);
    ring.write([new Float32Array([1, 2, 3])]);
    ring.write([new Float32Array([4, 5, 6])]); // should only fit remaining capacity
    const metrics = ring.getMetrics();
    expect(metrics.available).toBeLessThanOrEqual(3);
  });

  it('wraps around the ring capacity', () => {
    const sab = new SharedArrayBuffer(32 + 8 * 1 * 4);
    const ring = new SharedAudioRingBuffer(sab, 8, 1);
    ring.write([new Float32Array([1, 2, 3, 4, 5])]);
    ring.read(3);
    ring.write([new Float32Array([6, 7, 8, 9, 10])]);
    const out = ring.read(8);
    expect(out.at(0)?.length).toBeLessThanOrEqual(8);
  });

  it('resets and clears state', () => {
    const sab = new SharedArrayBuffer(32 + 8 * 1 * 4);
    const ring = new SharedAudioRingBuffer(sab, 8, 1);
    ring.write([new Float32Array([1, 2])]);
    ring.reset();
    const metrics = ring.getMetrics();
    expect(metrics.available).toBe(0);
  });
});

describe('LocalAudioRingBuffer', () => {
  it('writes and reads back planar frames', () => {
    const ring = new LocalAudioRingBuffer(16, 2);
    ring.write([new Float32Array([1, 2, 3]), new Float32Array([4, 5, 6])]);
    const metrics = ring.getMetrics();
    expect(metrics.available).toBe(3);
  });

  it('reports overrun', () => {
    const ring = new LocalAudioRingBuffer(4, 1);
    ring.write([new Float32Array([1, 2, 3, 4, 5])]);
    ring.reportOverrun(2);
    const metrics = ring.getMetrics();
    expect(metrics.overrun).toBe(2);
  });
});
