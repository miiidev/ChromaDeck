import { describe, expect, it } from "vitest";
import {
  formatImportSummary,
  formatScheduleWindow,
  isOvernightWindow,
  isValidHhmm,
  parseHhmm,
  validateScheduleRule,
} from "./schedule";

describe("schedule helpers", () => {
  it("parses valid HH:MM and rejects malformed or out-of-range values", () => {
    expect(parseHhmm("00:00")).toBe(0);
    expect(parseHhmm("21:30")).toBe(1290);
    expect(isValidHhmm("23:59")).toBe(true);
    expect(isValidHhmm("24:00")).toBe(false);
    expect(isValidHhmm("9:00")).toBe(false);
    expect(isValidHhmm("12:60")).toBe(false);
  });

  it("detects overnight windows and formats labels", () => {
    expect(isOvernightWindow("21:00", "07:00")).toBe(true);
    expect(isOvernightWindow("08:00", "18:00")).toBe(false);
    expect(formatScheduleWindow("21:00", "07:00")).toBe("21:00 – 07:00 (overnight)");
    expect(formatScheduleWindow("08:00", "18:00")).toBe("08:00 – 18:00");
  });

  it("validates schedule rules against empty targets and identical times", () => {
    expect(
      validateScheduleRule({
        id: "1",
        edid_id: "M1",
        preset_id: "P1",
        start_time: "21:00",
        end_time: "07:00",
        enabled: true,
      }),
    ).toBeNull();
    expect(
      validateScheduleRule({
        id: "1",
        edid_id: "M1",
        preset_id: "P1",
        start_time: "08:00",
        end_time: "08:00",
        enabled: true,
      }),
    ).toContain("cannot be the same");
  });

  it("formats import summaries honestly with rename and conflict counts", () => {
    expect(
      formatImportSummary({
        imported_count: 2,
        icc_imported_count: 1,
        renamed_collisions: ["FPS -> FPS (imported)"],
        stripped_conflicting_programs: ["cs2.exe (FPS (imported))"],
        stripped_conflicting_shortcuts: [],
      }),
    ).toBe("Imported 2 presets · 1 ICC · 1 renamed · 1 conflict cleared");
  });
});
