import { test, expect } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

interface BrowserSnapshot {
  readonly browser: string;
  readonly userAgent: string;
  readonly platform: string;
  readonly hardwareConcurrency: number | undefined;
  readonly deviceMemory: number | undefined;
  readonly crossOriginIsolated: boolean;
  readonly sharedArrayBuffer: boolean;
  readonly webglVendor: string | undefined;
  readonly webglRenderer: string | undefined;
  readonly webCodecs: boolean;
  readonly mediaSource: boolean;
  readonly webAudio: boolean;
  readonly webgpu: boolean;
  readonly webgl2: boolean;
  readonly wasm: boolean;
}

test('records a browser capability snapshot', async ({ page, browserName }, testInfo) => {
  await page.goto('/');

  const snapshot = await page.evaluate((name): BrowserSnapshot => {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    const debugInfo = gl?.getExtension('WEBGL_debug_renderer_info');

    const hasGlobal = (n: string) => typeof (globalThis as unknown as Record<string, unknown>)[n] !== 'undefined';

    return {
      browser: name,
      userAgent: navigator.userAgent,
      platform: navigator.platform,
      hardwareConcurrency: navigator.hardwareConcurrency,
      deviceMemory: (navigator as unknown as { deviceMemory?: number }).deviceMemory,
      crossOriginIsolated: (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated ?? false,
      sharedArrayBuffer: hasGlobal('SharedArrayBuffer'),
      webglVendor: debugInfo ? gl?.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) as string : undefined,
      webglRenderer: debugInfo ? gl?.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) as string : undefined,
      webCodecs: hasGlobal('VideoDecoder') && hasGlobal('AudioDecoder'),
      mediaSource: hasGlobal('MediaSource'),
      webAudio: hasGlobal('AudioContext'),
      webgpu: hasGlobal('GPU'),
      webgl2: hasGlobal('WebGL2RenderingContext'),
      wasm: hasGlobal('WebAssembly'),
    };
  }, browserName);

  expect(snapshot.wasm).toBe(true);

  const snapshotDir = process.env.CI ? join(process.cwd(), 'test-results') : testInfo.outputDir;
  mkdirSync(snapshotDir, { recursive: true });
  const file = join(snapshotDir, `browser-snapshot-${browserName}.json`);
  writeFileSync(file, JSON.stringify(snapshot, null, 2));
  testInfo.attach('browser-snapshot', { path: file, contentType: 'application/json' });
});

test('isolated demo enables crossOriginIsolated and SharedArrayBuffer', async ({ page }) => {
  await page.goto('/isolated');
  const isolated = await page.evaluate(() => ({
    crossOriginIsolated: (globalThis as unknown as { crossOriginIsolated?: boolean }).crossOriginIsolated ?? false,
    sharedArrayBuffer: typeof SharedArrayBuffer !== 'undefined',
  }));
  expect(isolated.crossOriginIsolated).toBe(true);
  expect(isolated.sharedArrayBuffer).toBe(true);

  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'preroll', { timeout: 10000 });
});
