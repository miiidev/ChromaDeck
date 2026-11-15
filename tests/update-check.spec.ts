import { test, expect } from '@playwright/test';

test('Check for updates shows feedback when no update available', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /Check for updates/ }).click();
  await expect(page.getByRole('dialog', { name: /up to date|Update available/ })).toBeVisible();
});

test('Check for updates button returns to normal after check', async ({ page }) => {
  await page.goto('/');
  const checkButton = page.getByRole('button', { name: /Check for updates/ });
  await checkButton.click();
  const dialog = page.getByRole('dialog', { name: /up to date/ });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'OK', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(checkButton).toBeEnabled();
  await expect(checkButton).toContainText('Check for updates');
});
