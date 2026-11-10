import { describe, it, expect } from "vitest";
import { updateStateReducer, initialUpdateState, type UpdateState } from "./autoUpdateState";

describe("updateStateReducer", () => {
  it("starts idle", () => {
    expect(initialUpdateState()).toEqual({ status: "idle" });
  });

  it("transitions idle -> checking -> available", () => {
    let state: UpdateState = initialUpdateState();
    state = updateStateReducer(state, { type: "CHECK_START" });
    expect(state.status).toBe("checking");

    state = updateStateReducer(state, {
      type: "CHECK_SUCCESS",
      payload: { available: true, version: "0.9.0", notes: "Fixes bugs", date: "2026-11-10" },
    });
    expect(state.status).toBe("available");
    if (state.status === "available") {
      expect(state.version).toBe("0.9.0");
      expect(state.notes).toBe("Fixes bugs");
      expect(state.date).toBe("2026-11-10");
    }
  });

  it("transitions idle -> checking -> idle when no update", () => {
    let state: UpdateState = initialUpdateState();
    state = updateStateReducer(state, { type: "CHECK_START" });
    state = updateStateReducer(state, {
      type: "CHECK_SUCCESS",
      payload: { available: false, version: "0.8.4", notes: null, date: null },
    });
    expect(state.status).toBe("idle");
  });

  it("transitions checking -> error on failure", () => {
    let state: UpdateState = initialUpdateState();
    state = updateStateReducer(state, { type: "CHECK_START" });
    state = updateStateReducer(state, {
      type: "CHECK_FAILURE",
      payload: { message: "network error" },
    });
    expect(state.status).toBe("error");
    if (state.status === "error") expect(state.message).toBe("network error");
  });

  it("transitions available -> downloading -> ready-to-install", () => {
    let state: UpdateState = {
      status: "available",
      version: "0.9.0",
      notes: null,
      date: null,
    };
    state = updateStateReducer(state, { type: "DOWNLOAD_START" });
    expect(state.status).toBe("downloading");

    state = updateStateReducer(state, {
      type: "DOWNLOAD_PROGRESS",
      payload: { downloaded: 500, total: 1000, percent: 50 },
    });
    if (state.status === "downloading") {
      expect(state.downloaded).toBe(500);
      expect(state.total).toBe(1000);
      expect(state.percent).toBe(50);
    }

    state = updateStateReducer(state, { type: "DOWNLOAD_SUCCESS" });
    expect(state.status).toBe("ready-to-install");
  });

  it("stays downloading on progress updates", () => {
    let state: UpdateState = {
      status: "downloading",
      version: "0.9.0",
      notes: null,
      date: null,
      downloaded: 0,
      total: 1000,
      percent: 0,
    };
    state = updateStateReducer(state, {
      type: "DOWNLOAD_PROGRESS",
      payload: { downloaded: 300, total: 1000, percent: 30 },
    });
    if (state.status === "downloading") {
      expect(state.downloaded).toBe(300);
      expect(state.percent).toBe(30);
    }
    expect(state.status).toBe("downloading");
  });

  it("transitions downloading -> error on failure", () => {
    let state: UpdateState = {
      status: "downloading",
      version: "0.9.0",
      notes: null,
      date: null,
      downloaded: 100,
      total: 1000,
      percent: 10,
    };
    state = updateStateReducer(state, {
      type: "DOWNLOAD_FAILURE",
      payload: { message: "signature verification failed" },
    });
    expect(state.status).toBe("error");
    if (state.status === "error") expect(state.message).toBe("signature verification failed");
  });

  it("ignores unrelated actions", () => {
    let state: UpdateState = { status: "idle" };
    state = updateStateReducer(state, { type: "DOWNLOAD_START" });
    expect(state.status).toBe("idle");

    state = updateStateReducer(state, { type: "DOWNLOAD_SUCCESS" });
    expect(state.status).toBe("idle");
  });

  it("RESET returns to idle", () => {
    let state: UpdateState = {
      status: "error",
      message: "some error",
    };
    state = updateStateReducer(state, { type: "RESET" });
    expect(state).toEqual({ status: "idle" });
  });
});
