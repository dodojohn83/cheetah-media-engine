import { test, expect, type Page } from '@playwright/test';

async function createPlayer(page: Page, attrs: Record<string, string>): Promise<void> {
  await page.goto('/');
  await page.waitForFunction(() => customElements.get('cheetah-player') !== undefined);
  await page.evaluate((attributes) => {
    const app = document.getElementById('app') ?? document.body;
    app.innerHTML = '';
    const player = document.createElement('cheetah-player');
    player.setAttribute('controls', '');
    for (const [key, value] of Object.entries(attributes)) {
      player.setAttribute(key, value);
    }
    app.appendChild(player);
  }, attrs);
}

async function routeExampleComTo404(page: Page): Promise<void> {
  // External network is an uncontrolled dependency; force a deterministic
  // 404 so the player surfaces a failed state instead of timing out.
  await page.unrouteAll();
  await page.route('http://example.com/test.flv', (route) => route.abort('internetdisconnected'));
}

test.beforeEach(async ({ page }) => {
  await page.unrouteAll();
});

test('player surfaces failed state when worker URL is missing', async ({ page }) => {
  await routeExampleComTo404(page);
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/nonexistent-worker.js',
    'wasm-url': '/wasm/cheetah_media_web_bindings.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player surfaces failed state when wasm module is missing', async ({ page }) => {
  await routeExampleComTo404(page);
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/worker.js',
    'wasm-url': '/wasm/nonexistent.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player surfaces failed state when wasm module has wrong MIME type', async ({ page }) => {
  await routeExampleComTo404(page);
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/worker.js',
    'wasm-url': '/fault/wrong-mime.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player surfaces failed state for invalid source URL', async ({ page }) => {
  await createPlayer(page, {
    src: 'not-a-url',
    'worker-url': '/worker.js',
    'wasm-url': '/wasm/cheetah_media_web_bindings.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player reaches preroll with valid source and runtime URLs', async ({ page }) => {
  // Keep the MSE path busy long enough to observe preroll without depending on
  // the real example.com response.
  await page.unrouteAll();
  await page.route('http://example.com/test.flv', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'video/x-flv',
      body: Buffer.from([0x46, 0x4c, 0x56, 0x01, 0x01, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x09]),
    }),
  );
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/worker.js',
    'wasm-url': '/wasm/cheetah_media_web_bindings.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'preroll', { timeout: 10000 });
});
