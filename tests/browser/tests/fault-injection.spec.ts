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

test('player surfaces failed state when worker URL is missing', async ({ page }) => {
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/nonexistent-worker.js',
    'wasm-url': '/wasm/cheetah_media_web_bindings.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player surfaces failed state when wasm module is missing', async ({ page }) => {
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/worker.js',
    'wasm-url': '/wasm/nonexistent.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 10000 });
});

test('player surfaces failed state when wasm module has wrong MIME type', async ({ page }) => {
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
  await createPlayer(page, {
    src: 'http://example.com/test.flv',
    'worker-url': '/worker.js',
    'wasm-url': '/wasm/cheetah_media_web_bindings.js',
  });
  const player = page.locator('cheetah-player');
  await expect(player).toHaveAttribute('data-state', 'preroll', { timeout: 10000 });
});
