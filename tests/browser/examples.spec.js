import { test, expect } from '@playwright/test';

test('module example runs its queries', async ({ page }) => {
  await page.goto('/examples/web/module/index.html');

  await expect(page.locator('#box code')).toHaveCount(7);
  await expect(page.locator('#box')).toContainText('wow_id');
  await expect(page.locator('#box')).toContainText('schemaless');
});

test('opfs example counts visits across reloads and runs queries', async ({ page }) => {
  await page.goto('/examples/web/opfs/index.html');
  await expect(page.locator('#status')).toHaveText('ready');
  await expect(page.locator('#visit-count')).toHaveText('1');

  await page.locator('#sql').fill('SELECT 1 AS one;');
  await page.locator('#run').click();
  await expect(page.locator('#results td')).toHaveText('1');

  await page.locator('#sql').fill('SELECT * FROM Missing;');
  await page.locator('#run').click();
  await expect(page.locator('#results .error')).toContainText('table not found: Missing');

  await page.reload();
  await expect(page.locator('#status')).toHaveText('ready');
  await expect(page.locator('#visit-count')).toHaveText('2');

  await page.locator('#run').click();
  await expect(page.locator('#results td')).toHaveCount(2);
});
