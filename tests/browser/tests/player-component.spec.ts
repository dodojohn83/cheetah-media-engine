import { test, expect } from '@playwright/test';

test('custom element renders and surfaces load failure state', async ({ page }) => {
  await page.goto('/');
  const player = page.locator('cheetah-player');
  await expect(player).toBeVisible();
  await expect(player).toHaveAttribute('data-state', 'failed', { timeout: 5000 });
  await expect(player.locator('[part="overlay"]')).toBeVisible();
});

test('reflects JavaScript properties to attributes', async ({ page }) => {
  await page.goto('/');
  const player = page.locator('cheetah-player');
  await expect(player).toBeVisible();

  await player.evaluate((el) => {
    const p = el as HTMLElement & { volume: number; muted: boolean; controls: boolean };
    p.volume = 0.25;
    p.muted = true;
    p.controls = true;
  });

  await expect(player).toHaveAttribute('volume', '0.25');
  await expect(player).toHaveAttribute('muted', '');
  await expect(player).toHaveAttribute('controls', '');
});

test('keyboard shortcuts are handled without crashing', async ({ page }) => {
  await page.goto('/');
  const player = page.locator('cheetah-player');
  await expect(player).toBeVisible();
  await player.press('Space');
  await expect(player).toHaveAttribute('data-state', 'failed');
});

test('resize sets surface CSS variables', async ({ page }) => {
  await page.goto('/');
  const player = page.locator('cheetah-player');
  await expect(player).toBeVisible();

  await player.evaluate((el) => {
    el.style.width = '640px';
    el.style.height = '360px';
  });

  const width = await player.evaluate((el) => el.style.getPropertyValue('--surface-width'));
  expect(width).toBe('640px');
});
