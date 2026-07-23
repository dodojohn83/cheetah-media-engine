import { describe, it, expect } from 'vitest';
import { MetricRegistry } from './metrics';

describe('MetricRegistry', () => {
  it('records and snapshots counters', () => {
    const registry = new MetricRegistry();
    const counter = registry.counter('packets-received', 'source', 'packets');
    counter.inc(5);
    counter.inc();
    const snapshot = registry.snapshot();
    expect(snapshot.metrics.source?.['packets-received']).toEqual({
      type: 'counter',
      value: 6,
      unit: 'packets',
    });
  });

  it('records and snapshots gauges', () => {
    const registry = new MetricRegistry();
    const gauge = registry.gauge('buffer-size', 'memory', 'bytes');
    gauge.set(1024);
    const snapshot = registry.snapshot();
    expect(snapshot.metrics.memory?.['buffer-size']).toEqual({
      type: 'gauge',
      value: 1024,
      unit: 'bytes',
    });
  });

  it('computes histogram percentiles from buckets', () => {
    const registry = new MetricRegistry();
    const histogram = registry.histogram('decode-latency', 'decode', 'ms');
    for (let i = 1; i <= 100; i += 1) {
      histogram.record(i);
    }
    const snapshot = registry.snapshot();
    const hist = snapshot.metrics.decode?.['decode-latency'];
    expect(hist?.type).toBe('histogram');
    if (hist?.type !== 'histogram') throw new Error('expected histogram');
    expect(hist.count).toBe(100);
    expect(hist.sum).toBe(5050);
    expect(hist.percentiles.p50).toBeGreaterThan(0);
    expect(hist.percentiles.p95).toBeGreaterThanOrEqual(hist.percentiles.p50);
    expect(hist.percentiles.p99).toBeGreaterThanOrEqual(hist.percentiles.p95);
    expect(hist.percentiles.p50).toBeCloseTo(50, 5);
  });

  it('does not derive p95 from an average', () => {
    const registry = new MetricRegistry();
    const histogram = registry.histogram('latency', 'timeline', 'ms');
    // 99 values at 1 ms and one outlier at 1000 ms.
    for (let i = 0; i < 99; i += 1) histogram.record(1);
    histogram.record(1000);
    const snapshot = registry.snapshot();
    const hist = snapshot.metrics.timeline?.['latency'];
    if (hist?.type !== 'histogram') throw new Error('expected histogram');
    const avg = hist.sum / hist.count; // about 10.9 ms
    // p95 should be near the 95th percentile (1 ms), not near the average.
    expect(hist.percentiles.p95).toBeLessThan(avg);
    expect(hist.percentiles.p95).toBeLessThanOrEqual(10);
  });

  it('places overflow values in a separate bucket for percentile accuracy', () => {
    const registry = new MetricRegistry();
    const histogram = registry.histogram('decode-latency', 'decode', 'ms');
    for (let i = 0; i < 50; i += 1) histogram.record(100); // within finite buckets
    for (let i = 0; i < 50; i += 1) histogram.record(70_000); // overflow
    const snapshot = registry.snapshot();
    const hist = snapshot.metrics.decode?.['decode-latency'];
    if (hist?.type !== 'histogram') throw new Error('expected histogram');
    expect(hist.count).toBe(100);
    expect(hist.buckets[hist.buckets.length - 1]!.upperBound).toBe(70_000);
    // The overflow bucket is distinct and tail percentiles are not capped at the last finite boundary.
    expect(hist.percentiles.p95).toBeGreaterThan(60_000);
    expect(hist.percentiles.p99).toBeGreaterThan(60_000);
  });

  it('resets all metrics or a single category', () => {
    const registry = new MetricRegistry();
    registry.counter('a', 'source').inc(3);
    registry.counter('b', 'render').inc(7);
    registry.reset('source');
    let snapshot = registry.snapshot();
    expect(snapshot.metrics.source?.a).toEqual({ type: 'counter', value: 0 });
    expect(snapshot.metrics.render?.b).toEqual({ type: 'counter', value: 7 });
    registry.reset();
    snapshot = registry.snapshot();
    expect(snapshot.metrics.render?.b).toEqual({ type: 'counter', value: 0 });
  });

  it('throws when re-registering a metric with a different type', () => {
    const registry = new MetricRegistry();
    registry.counter('x', 'fallback');
    expect(() => registry.gauge('x', 'fallback')).toThrow();
  });

  it('ignores negative and non-finite histogram samples', () => {
    const registry = new MetricRegistry();
    const histogram = registry.histogram('frame-time', 'render', 'ms');
    histogram.record(-1);
    histogram.record(NaN);
    histogram.record(Infinity);
    histogram.record(5);
    const snapshot = registry.snapshot();
    const hist = snapshot.metrics.render?.['frame-time'];
    if (hist?.type !== 'histogram') throw new Error('expected histogram');
    expect(hist.count).toBe(1);
    expect(hist.sum).toBe(5);
  });

  it('ignores negative and non-finite counter increments', () => {
    const registry = new MetricRegistry();
    const counter = registry.counter('c', 'source');
    counter.inc(-1);
    counter.inc(NaN);
    counter.inc(Infinity);
    counter.inc(3);
    const snapshot = registry.snapshot();
    expect(snapshot.metrics.source?.c).toEqual({ type: 'counter', value: 3 });
  });

  it('ignores non-finite gauge values', () => {
    const registry = new MetricRegistry();
    const gauge = registry.gauge('g', 'memory', 'bytes');
    gauge.set(NaN);
    gauge.set(Infinity);
    gauge.set(42);
    const snapshot = registry.snapshot();
    expect(snapshot.metrics.memory?.g).toEqual({ type: 'gauge', value: 42, unit: 'bytes' });
  });

  it('rejects invalid metric names and categories', () => {
    const registry = new MetricRegistry();
    expect(() => registry.counter('', 'source')).toThrow();
    expect(() => registry.counter('x', 'unknown' as 'source')).toThrow();
  });
});
