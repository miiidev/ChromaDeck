import { describe, it, expect } from "vitest";
import {
  checkForUpdate,
  downloadAndInstallUpdate,
  onUpdateProgress,
  type UpdateInfo,
  type UpdateProgress,
} from "./tauri";

describe("checkForUpdate (browser preview)", () => {
  it("returns no update available", async () => {
    const info = await checkForUpdate();
    expect(info.available).toBe(false);
    expect(info.notes).toBeNull();
    expect(info.date).toBeNull();
  });

  it("returns UpdateInfo with expected shape", async () => {
    const info: UpdateInfo = await checkForUpdate();
    expect(typeof info.available).toBe("boolean");
    expect(typeof info.version).toBe("string");
    expect(info.notes === null || typeof info.notes === "string").toBe(true);
    expect(info.date === null || typeof info.date === "string").toBe(true);
  });
});

describe("downloadAndInstallUpdate (browser preview)", () => {
  it("resolves without calling the backend", async () => {
    await expect(downloadAndInstallUpdate()).resolves.toBeUndefined();
  });
});

describe("onUpdateProgress (browser preview)", () => {
  it("returns an unsubscribe function", () => {
    let received: UpdateProgress | null = null;
    const unsubscribe = onUpdateProgress((p) => {
      received = p;
    });
    expect(typeof unsubscribe).toBe("function");
    unsubscribe();
    expect(received).toBeNull();
  });
});
