import { describe, it, expect } from 'vitest';
import { buildDiagnostics, sanitizeUrl, redactHeaders, DIAGNOSTICS_VERSION } from './diagnostics';
import { MetricRegistry } from './metrics';
import { startTrace, endTrace } from './trace';

describe('diagnostics', () => {
  it('sanitizes URLs to origin and path', () => {
    expect(sanitizeUrl('https://example.com/path/to/manifest.m3u8?token=secret#frag')).toBe(
      'https://example.com/path/to/manifest.m3u8',
    );
    expect(sanitizeUrl('not-a-url')).toBe('<redacted>');
  });

  it('redacts sensitive headers and preserves non-sensitive ones', () => {
    expect(
      redactHeaders({
        'X-Api-Key': 'secret',
        Cookie: 'session=abc',
        'Accept-Language': 'en',
      }),
    ).toEqual({
      'X-Api-Key': '<redacted>',
      Cookie: '<redacted>',
      'Accept-Language': 'en',
    });
  });

  it('builds a redacted bundle with version and state', () => {
    const registry = new MetricRegistry();
    registry.counter('packets', 'source').inc(3);
    const metrics = registry.snapshot();
    const bundle = buildDiagnostics(
      'player-1',
      'playing',
      2,
      { headers: { Authorization: 'Bearer secret' }, url: 'https://x.com/stream?k=v' },
      { metrics },
      { maxEventCount: 2 },
    );
    expect(bundle.version).toBe(DIAGNOSTICS_VERSION);
    expect(bundle.playerId).toBe('player-1');
    expect(bundle.state).toBe('playing');
    expect(bundle.metrics).toEqual(metrics);
    // Type guard for exactOptionalPropertyTypes tests above
    if (!bundle.metrics) throw new Error('expected metrics');
    expect(bundle.config).toEqual({
      headers: { Authorization: '<redacted>' },
      url: 'https://x.com/stream',
    });
  });

  it('limits event count in bundle', () => {
    const events = Array.from({ length: 10 }, (_, i) => ({
      type: 'tick',
      timestamp: i,
    }));
    const bundle = buildDiagnostics('p', 'idle', 0, {}, { events }, { maxEventCount: 3 });
    expect(bundle.recentEvents.length).toBe(3);
    expect(bundle.recentEvents[0]!.timestamp).toBe(7);
  });

  it('truncates trace depth', () => {
    const trace = startTrace('p', 'root', 1, 1);
    const child = endTrace(trace);
    const bundle = buildDiagnostics('p', 'idle', 0, {}, { trace: child.root }, { maxTraceDepth: 0 });
    expect(bundle.trace?.children.length).toBe(0);
  });

  it('does not fail with empty inputs', () => {
    const bundle = buildDiagnostics('p', 'idle', 0, null, {});
    expect(bundle.recentEvents.length).toBe(0);
    expect(bundle.trace).toBeUndefined();
    expect(bundle.metrics).toBeUndefined();
  });

  it('terminates when trimming to a very small max size', () => {
    const events = Array.from({ length: 3 }, (_, i) => ({
      type: 'tick',
      timestamp: i,
      details: { x: 'a'.repeat(256) },
    }));
    const bundle = buildDiagnostics('p', 'idle', 0, {}, { events }, { maxSizeBytes: 64 });
    expect(bundle.recentEvents.length).toBeLessThanOrEqual(3);
  });
});
