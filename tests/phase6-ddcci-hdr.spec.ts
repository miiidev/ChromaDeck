import { expect, test } from "@playwright/test";

test.describe("Phase 6 — DDC/CI hardware backlight & HDR/ACM badges", () => {
  test("opt-in DDC/CI backlight toggle, numeric edit, and Reset sliders in PresetEditor", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");

    // Open New Preset editor
    await page.getByRole("button", { name: /Create preset/i }).first().click();
    const ddcciSection = page.locator('[data-slot="ddcci-section"]');
    await expect(ddcciSection).toBeVisible();

    // Browser preview probes connected demo monitor as DDC/CI ready at 65%
    await expect(ddcciSection.locator('[data-slot="ddcci-status"]')).toContainText("DDC/CI ready · current 65%");

    // Off by default — no Backlight slider visible yet
    const toggle = ddcciSection.getByRole("checkbox", { name: /Control physical monitor backlight/i });
    await expect(toggle).not.toBeChecked();
    await expect(ddcciSection.getByRole("button", { name: /Edit Backlight value/i })).toHaveCount(0);

    // Enable opt-in backlight -> initializes to probed 65%
    await toggle.click();
    await expect(toggle).toBeChecked();
    const valueBtn = ddcciSection.getByRole("button", { name: /Edit Backlight value \(65%\)/i });
    await expect(valueBtn).toBeVisible();

    // Click to type an exact value (40%)
    await valueBtn.click();
    const numInput = ddcciSection.getByRole("spinbutton", { name: /Backlight value/i });
    await numInput.fill("40");
    await numInput.press("Enter");
    await expect(ddcciSection.getByRole("button", { name: /Edit Backlight value \(40%\)/i })).toBeVisible();

    // Scroll DDC/CI section into view and capture screenshot for visual QA
    await ddcciSection.scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);
    await page.screenshot({
      path: "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch/phase6-ddcci-editor.png",
    });

    // Reset sliders clears opt-in hardware backlight back to unchecked
    await page.getByRole("button", { name: /Reset sliders/i }).click();
    await expect(toggle).not.toBeChecked();
    await expect(ddcciSection.getByRole("button", { name: /Edit Backlight value/i })).toHaveCount(0);
  });

  test("renders HDR and ACM badges on monitor cards and reacts to monitors://changed", async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.addInitScript(() => {
      localStorage.setItem("chromadeck.updateSuppressedUntil", String(Date.now() + 86400000));
      const callbacks = new Map<number, (value: unknown) => void>();
      const listeners = new Map<string, number>();
      let next = 1;
      let hdrEnabled = true;

      const monitors = () => [
        {
          edid_id: "EDID:HDR-MON",
          model: "LG UltraGear OLED",
          serial: "LG-HDR-01",
          connected: true,
          device_name: "\\\\.\\DISPLAY1",
          alias: "",
          hdr_active: hdrEnabled,
          acm_active: false,
        },
        {
          edid_id: "EDID:ACM-MON",
          model: "Dell UltraSharp U2723QE",
          serial: "DL-ACM-02",
          connected: true,
          device_name: "\\\\.\\DISPLAY2",
          alias: "",
          hdr_active: false,
          acm_active: true,
        },
      ];

      const presets = [
        {
          id: "p-night",
          name: "Night - Dim",
          icc_hash: "",
          icc_filename: "",
          brightness: 50,
          contrast: 50,
          rgb_gains: [1, 1, 1],
          gamma: 1.0,
          vibrance: 50,
          saturation: 50,
          hue_deg: 0,
          color_tag: "#38bdf8",
          tag_shape: "pill",
          attached_programs: [],
          shortcut: null,
          hardware_brightness: 35,
        },
      ];

      const internals = {
        metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
        transformCallback(callback: (value: unknown) => void) {
          const id = next++;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback(id: number) {
          callbacks.delete(id);
        },
        async invoke(command: string, args: Record<string, unknown> = {}) {
          if (command === "plugin:event|listen") {
            listeners.set(String(args.event), Number(args.handler));
            return next++;
          }
          if (command === "plugin:event|unlisten") return;
          if (command === "list_monitors_cmd") return monitors();
          if (command === "list_presets_cmd") return presets;
          if (command === "list_pins_cmd" || command === "list_applied_cmd") return {};
          if (command === "list_schedules_cmd" || command === "list_auto_applied_cmd") return [];
          if (command === "get_auto_apply_enabled_cmd") return false;
          if (command === "probe_ddcci_cmd") return { supported: true, current_percent: 70, reason: null };
          return false;
        },
      };

      Object.assign(window, {
        isTauri: true,
        __TAURI_INTERNALS__: internals,
        __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
        phase6Test: {
          toggleHdrOff() {
            hdrEnabled = false;
            const handler = listeners.get("monitors://changed");
            if (handler !== undefined) {
              callbacks.get(handler)?.({ event: "monitors://changed", id: 0, payload: null });
            }
          },
        },
      });
    });

    await page.goto("/");

    // HDR badge visible on HDR monitor, ACM badge visible on ACM monitor
    const hdrBadge = page.locator('[data-drop-monitor="EDID:HDR-MON"] [data-slot="hdr-badge"]');
    const acmBadge = page.locator('[data-drop-monitor="EDID:ACM-MON"] [data-slot="acm-badge"]');
    await expect(hdrBadge).toBeVisible();
    await expect(hdrBadge).toHaveText("HDR");
    await expect(acmBadge).toBeVisible();
    await expect(acmBadge).toHaveText("ACM");

    // Preset card shows opt-in hardware backlight summary ("BL 35%")
    await expect(page.getByText(/BL 35%/)).toBeVisible();

    // Wait for staged entrance animation to settle before visual QA capture
    await page.waitForTimeout(600);
    await page.screenshot({
      path: "C:/Users/Ahmad Syahmi/.gemini/antigravity-cli/brain/fd4a5cdc-d6cd-4e1c-a48f-47286d322183/scratch/phase6-hdr-acm-badges.png",
    });

    // Emitting monitors://changed after HDR -> SDR clears the HDR badge immediately
    await page.evaluate(() => {
      (window as unknown as { phase6Test: { toggleHdrOff: () => void } }).phase6Test.toggleHdrOff();
    });
    await expect(hdrBadge).toHaveCount(0);
    await expect(acmBadge).toBeVisible();
  });
});
