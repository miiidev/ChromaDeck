import { test, expect } from '@playwright/test';

test('Check for updates shows feedback when no update available', async ({ page }) => {
  await page.goto('http://localhost:1420');
  
  // Wait for the app to load
  await page.waitForSelector('h1:has-text("ChromaDeck")');
  
  // Click the "Check for updates" button in the footer
  const checkButton = page.getByRole('button', { name: /Check for updates/ });
  await expect(checkButton).toBeVisible();
  await checkButton.click();
  
  // Wait for feedback message to appear (check is async in real app)
  await page.waitForTimeout(3000);
  
  // Should show either "You're up to date!" or an update dialog
  const upToDate = page.getByText(/You.*up to date/);
  const updateDialog = page.getByText(/Update available/);
  
  // One of these should appear
  const upToDateVisible = await upToDate.isVisible().catch(() => false);
  const updateDialogVisible = await updateDialog.isVisible().catch(() => false);
  
  expect(upToDateVisible || updateDialogVisible).toBe(true);
});

test('Check for updates button returns to normal after check', async ({ page }) => {
  await page.goto('http://localhost:1420');
  
  await page.waitForSelector('h1:has-text("ChromaDeck")');
  
  const checkButton = page.getByRole('button', { name: /Check for updates/ });
  await expect(checkButton).toBeVisible();
  
  await checkButton.click();
  
  // Wait for the check to complete
  await page.waitForTimeout(3000);
  
  // Button should be back to normal (not checking)
  await expect(checkButton).toContainText('Check for updates');
});
