import { test, expect } from "@playwright/test";

const SCRATCH_DIR =
  "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch";

test("PresetEditor running-programs combobox picks open apps and accepts typed exe", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");

  await page.getByRole("button", { name: /Create preset/ }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  const programInput = dialog.getByRole("combobox", {
    name: "Program executable",
  });
  await programInput.scrollIntoViewIfNeeded();
  await programInput.click();

  // Dropdown lists running apps from listRunningPrograms()
  const vlcOption = page.getByRole("option", { name: "vlc.exe" });
  await expect(vlcOption).toBeVisible();
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${SCRATCH_DIR}/phase2-programs-combobox.png` });

  // Clicking an option attaches the chip immediately
  await vlcOption.click();
  await expect(
    dialog.locator('[data-slot="program-chip"]', { hasText: "vlc.exe" }),
  ).toBeVisible();

  // Typing a custom executable still works via Enter
  await programInput.fill("cyberpunk2077.exe");
  await programInput.press("Enter");
  await expect(
    dialog.locator('[data-slot="program-chip"]', {
      hasText: "cyberpunk2077.exe",
    }),
  ).toBeVisible();

  // Refresh button stays enabled and clickable
  await dialog.getByRole("button", { name: "Refresh running programs" }).click();

  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).not.toBeVisible();
});
