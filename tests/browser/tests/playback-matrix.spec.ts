import { test, expect } from '@playwright/test';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

interface Fixture {
  readonly id: string;
  readonly protocol: string;
  readonly codec: string;
  readonly resolution?: string;
  readonly sample_rate?: number;
  readonly channels?: number;
  readonly duration_ms?: number;
}

interface PlaybackResult {
  readonly status: 'success' | 'skipped' | 'failed';
  readonly fixture: string;
  readonly protocol: string;
  readonly buffered: number;
  readonly currentTime: number;
  readonly duration?: number;
  readonly error?: string;
  readonly support: {
    readonly mediaSource: boolean;
  };
}

type PagePlaybackResult = PlaybackResult | { readonly status: 'initializing' };

interface Manifest {
  readonly fixtures: readonly Fixture[];
}

const manifestPath = resolve(process.cwd(), '..', '..', 'testing', 'fixtures', 'manifest.json');
const manifest: Manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as Manifest;

const playableProtocols = new Set([
  'http-fmp4',
  'ws-fmp4',
  'hls',
  'll-hls',
  'http-flv',
  'ws-flv',
]);

// Fixtures that are expected to reach success in Chromium using the MSE backend.
// Other fixtures may be skipped due to codec/browser support and are still
// recorded as valid evidence.
const expectedSuccessInChromium = new Set([
  'h264-1280x720-30fps-fmp4',
  'h264-http-fmp4-640x480',
  'h264-ws-fmp4-640x480',
  'aac-48khz-fmp4',
  'hls-h264-fmp4-640x480',
]);

const fixtures = manifest.fixtures.filter((f) => playableProtocols.has(f.protocol));

test.describe('real media playback matrix', () => {
  for (const fixture of fixtures) {
    test(`${fixture.id} (${fixture.protocol} / ${fixture.codec})`, async ({ page, browserName }, testInfo) => {
      if (browserName !== 'chromium') {
        test.skip();
      }

      const logs: string[] = [];
      page.on('console', (msg) => logs.push(`[${msg.type()}] ${msg.text()}`));
      page.on('pageerror', (err) => logs.push(`[pageerror] ${err.message}`));

      await page.goto(`/playback-test.html?fixture=${encodeURIComponent(fixture.id)}`);

      const handle = await page.waitForFunction(() => {
        const r = (window as unknown as { __playbackResult?: PagePlaybackResult }).__playbackResult;
        return r && r.status !== 'initializing' ? r : undefined;
      }, { timeout: 60000 });

      const result = (await handle.evaluate((r: PagePlaybackResult | undefined) => r)) as PlaybackResult;

      const evidence = {
        fixture,
        browser: browserName,
        userAgent: await page.evaluate(() => navigator.userAgent),
        result,
        logs,
      };

      const outputDir = testInfo.outputDir;
      mkdirSync(outputDir, { recursive: true });
      const file = resolve(outputDir, `playback-${fixture.id}-${browserName}.json`);
      writeFileSync(file, JSON.stringify(evidence, null, 2));
      testInfo.attach('playback-evidence', { path: file });

      if (result.status === 'failed') {
        expect(result.status, `playback failed: ${result.error ?? 'unknown error'}`).not.toBe('failed');
      }

      if (expectedSuccessInChromium.has(fixture.id)) {
        expect(result.status, `${fixture.id} should play successfully in Chromium`).toBe('success');
      } else {
        expect(['success', 'skipped']).toContain(result.status);
      }
    });
  }
})
