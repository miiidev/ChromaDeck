import { test, expect } from "@playwright/test";

const SCRATCH_DIR =
  "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch";

test("Phase 4: bundle import/export, diagnostics export, and schedule dialog workflow", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(err.message));

  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");

  // 1. Presets toolbar Import & Export buttons are visible and report honest status
  const importBtn = page.getByRole("button", { name: "Import presets" });
  const exportAllBtn = page.getByRole("button", { name: "Export all presets" });
  await expect(importBtn).toBeVisible();
  await expect(exportAllBtn).toBeVisible();

  await exportAllBtn.click();
  await expect(page.locator("footer")).toContainText(/Exported \d+ preset/);

  await importBtn.click();
  await expect(page.locator("footer")).toContainText(/Imported 1 preset/);

  // 2. Single-preset card export icon works
  const firstCardExport = page
    .locator('[data-slot="card"]')
    .first()
    .getByRole("button", { name: /^Export preset / });
  await firstCardExport.click();
  await expect(page.locator("footer")).toContainText(/Exported "/);

  // 3. Diagnostics export button works in footer
  const diagBtn = page.getByRole("button", { name: "Export diagnostics report" });
  await expect(diagBtn).toBeVisible();
  await diagBtn.click();
  await expect(page.locator("footer")).toContainText(/Diagnostics exported \(\d+ monitor/);

  // Screenshot main workspace with Phase 4 toolbar + footer controls
  await page.screenshot({ path: `${SCRATCH_DIR}/phase4-main-toolbar-footer.png` });

  // 4. Open ScheduleDialog, validate bad time, add overnight rule, save & verify badge
  const schedBtn = page.getByRole("button", { name: "Open display schedules" });
  await schedBtn.click();

  const dialog = page.locator('[data-slot="schedule-dialog"]');
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("Precedence: Pinned monitor > Active auto-apply > Schedule > Manual default");

  await dialog.getByRole("button", { name: "Add schedule" }).click();
  const row = dialog.locator('[data-slot="schedule-rule-row"]').first();
  await expect(row).toBeVisible();
  await expect(row).toContainText("21:00 – 07:00 (overnight)");

  // Screenshot ScheduleDialog with active overnight rule
  await page.screenshot({ path: `${SCRATCH_DIR}/phase4-schedule-dialog.png` });

  // Validation rejects identical start/end time
  const endInput = row.getByRole("textbox", { name: "End time for rule 1" });
  await endInput.fill("21:00");
  await dialog.getByRole("button", { name: "Save schedules" }).click();
  await expect(dialog.getByRole("alert")).toContainText("Start and end time cannot be the same");

  // Fix back to 07:00 and save cleanly
  await endInput.fill("07:00");
  await dialog.getByRole("button", { name: "Save schedules" }).click();
  await expect(dialog).not.toBeVisible();

  await expect(page.locator("footer")).toContainText("Saved 1 active schedule");
  await expect(schedBtn).toContainText("1");

  expect(pageErrors).toEqual([]);
});
