import { test, expect } from '@playwright/test';

test('demo page loads', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#app')).toBeVisible();
});
