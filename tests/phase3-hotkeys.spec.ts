import { test, expect } from "@playwright/test";

const SCRATCH_DIR =
  "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch";

test("PresetEditor records, validates, and clears global hotkey chords", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");

  await page.getByRole("button", { name: /Create preset/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  const recorder = dialog.locator('[data-slot="shortcut-recorder"]');
  await recorder.scrollIntoViewIfNeeded();
  const recordBtn = recorder.getByRole("button", {
    name: /Record global shortcut/i,
  });
  await recordBtn.click();
  await expect(recorder).toContainText("Press modifier + key");

  // Bare key without modifier shows inline guidance
  await page.keyboard.press("F1");
  await expect(recorder).toContainText("Include Ctrl, Alt, Shift, or Win");

  // Valid modifier + key chord records cleanly
  await page.keyboard.press("Control+Alt+F2");
  await expect(recorder).toContainText("Ctrl+Alt+F2");
  await page.screenshot({ path: `${SCRATCH_DIR}/phase3-shortcut-recorder.png` });

  // Clear button removes the shortcut
  await recorder.getByRole("button", { name: "Clear global shortcut" }).click();
  await expect(recorder).toContainText("None — click to record");

  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).not.toBeVisible();
});
