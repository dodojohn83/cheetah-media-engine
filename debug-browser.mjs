import { chromium } from 'playwright';

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  page.on('console', (msg) => console.log('CONSOLE:', msg.type(), msg.text()));
  page.on('pageerror', (err) => console.log('PAGEERROR:', err.stack || err.message));
  await page.goto('http://localhost:5173/');
  await page.waitForTimeout(1000);
  const state = await page.locator('cheetah-player').getAttribute('data-state');
  const html = await page.locator('cheetah-player').evaluate((el) => el.outerHTML);
  console.log('STATE after 1s:', state);
  console.log('HTML:', html);
  await browser.close();
})();
