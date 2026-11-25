import { test, expect } from "@playwright/test";

const SCRATCH_DIR =
  "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch";

test("PresetEditor supports RGB gains, inline numeric edit, Capture, Live preview, and Reset sliders", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");

  await page.getByRole("button", { name: /Create preset/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  // Hardware bar with monitor picker, Live preview toggle, and Capture button
  const hardwareBar = dialog.locator('[data-slot="editor-hardware-bar"]');
  await expect(hardwareBar).toBeVisible();

  // Capture live values in preview mode
  const captureBtn = hardwareBar.getByRole("button", { name: /^Capture$/ });
  await expect(captureBtn).toBeEnabled();
  await captureBtn.click();
  await expect(hardwareBar).toContainText("Captured live NVCP state");

  // Toggle live preview on and off
  const livePreviewCheckbox = hardwareBar.getByRole("checkbox", {
    name: /Live preview/i,
  });
  await livePreviewCheckbox.click();
  await expect(livePreviewCheckbox).toBeChecked();
  await expect(hardwareBar).toContainText("Live");

  // Edit Red gain via click-to-type numeric input
  const redValueBtn = dialog.getByRole("button", { name: /Edit Red value/ });
  await redValueBtn.scrollIntoViewIfNeeded();
  await redValueBtn.click();
  const redInput = dialog.getByRole("spinbutton", { name: "Red value" });
  await redInput.fill("1.18");
  await redInput.press("Enter");
  await expect(
    dialog.getByRole("button", { name: "Edit Red value (1.18)" }),
  ).toBeVisible();

  // Capture visual QA screenshot showing hardware bar, parameters, RGB gains, and ICC row
  await hardwareBar.scrollIntoViewIfNeeded();
  await page.screenshot({ path: `${SCRATCH_DIR}/phase1-editor.png` });

  // Reset sliders restores Red gain back to 1.00
  await dialog.getByRole("button", { name: /Reset sliders/ }).click();
  await expect(
    dialog.getByRole("button", { name: "Edit Red value (1.00)" }),
  ).toBeVisible();

  // Close editor cleanly
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).not.toBeVisible();
});

test("Reset all dialog exposes opt-in ICC dissociation checkbox", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");

  await page.getByRole("button", { name: "Reset all" }).click();
  const dialog = page.getByRole("dialog", { name: /Reset all monitors/i });
  await expect(dialog).toBeVisible();

  const iccCheckbox = dialog.getByRole("checkbox", {
    name: /Also dissociate ChromaDeck ICC profiles/i,
  });
  await expect(iccCheckbox).toBeVisible();
  await expect(iccCheckbox).not.toBeChecked();

  await page.screenshot({ path: `${SCRATCH_DIR}/phase1-reset-all.png` });

  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).not.toBeVisible();
});
