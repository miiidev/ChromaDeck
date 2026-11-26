import { expect, test } from '@playwright/test';

test('updater prevents duplicate downloads, distinguishes installation and displays errors', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('chromadeck.updateSuppressedUntil', String(Date.now() + 86400000));
    const callbacks = new Map<number, (value: unknown) => void>();
    const listeners = new Map<string, number>();
    let next = 1;
    let downloads = 0;
    let rejectDownload: ((error: unknown) => void) | undefined;
    const internals = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback(callback: (value: unknown) => void) {
        const id = next++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) { callbacks.delete(id); },
      async invoke(command: string, args: Record<string, unknown> = {}) {
        if (command === 'plugin:event|listen') {
          listeners.set(String(args.event), Number(args.handler));
          return next++;
        }
        if (command === 'plugin:event|unlisten') return;
        if (command === 'check_for_update_cmd') return { available: true, version: '1.0.0', notes: 'Regression test', date: null };
        if (command === 'download_and_install_cmd') {
          downloads++;
          return new Promise((_resolve, reject) => { rejectDownload = reject; });
        }
        if (command === 'list_monitors_cmd' || command === 'list_presets_cmd' || command === 'list_schedules_cmd') return [];
        if (command === 'list_pins_cmd' || command === 'list_applied_cmd') return {};
        if (command === 'list_auto_applied_cmd') return null;
        return false;
      },
    };
    Object.assign(window, {
      isTauri: true,
      __TAURI_INTERNALS__: internals,
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
      updaterTest: {
        downloads: () => downloads,
        emit(event: string, payload: unknown) {
          const handler = listeners.get(event);
          if (handler !== undefined) callbacks.get(handler)?.({ event, id: 0, payload });
        },
        fail: () => rejectDownload?.('Signature verification failed'),
      },
    });
  });
  await page.goto('/');
  await page.getByRole('button', { name: /Check for updates/ }).click();
  await page.getByRole('button', { name: 'Download & install', exact: true }).click();
  await expect(page.getByText('Downloading', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Installing…' })).toBeDisabled();
  await page.evaluate(() => {
    const harness = (window as unknown as { updaterTest: { emit: (event: string, payload: unknown) => void } }).updaterTest;
    harness.emit('update://progress', { downloaded: 100, total: 100, percent: 100 });
  });
  await expect(page.getByText('100%', { exact: true })).toBeVisible();
  await expect(page.getByText('Downloading', { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const harness = (window as unknown as { updaterTest: { emit: (event: string, payload: unknown) => void } }).updaterTest;
    harness.emit('update://installing', null);
  });
  await expect(page.getByRole('status')).toContainText('Installing update');
  await expect(page.getByRole('button', { name: 'Restart now' })).toHaveCount(0);
  await page.evaluate(() => (window as unknown as { updaterTest: { fail: () => void } }).updaterTest.fail());
  await expect(page.getByRole('alert')).toContainText('Signature verification failed');
  await expect(page.getByRole('button', { name: 'Check again' })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { updaterTest: { downloads: () => number } }).updaterTest.downloads())).toBe(1);
});
