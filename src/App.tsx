import { useState, useEffect, useCallback, useRef, useReducer } from "react";
import { flushSync } from "react-dom";
import "./App.css";
import "@fontsource/geist/400.css";
import "@fontsource/geist/600.css";
import "@fontsource/geist/700.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/700.css";
import {
  listMonitors,
  listPresets,
  listPins,
  listApplied,
  listSchedules,
  reapplyNow,
  getAutoApplyEnabled,
  setAutoApplyEnabled,
  listAutoApplied,
  exportDiagnostics,
  checkForUpdate,
  downloadAndInstallUpdate,
  type AutoApplyEvent,
  type HotkeyEvent,
} from "./lib/tauri";
import { version as appVersion } from "../package.json";
import type { Monitor, Preset, EnforceEvent, ScheduleOutcomeEvent, ScheduleRule } from "./lib/types";
import { applyAutoEvent, type AutoMap } from "./lib/autoApplyState";
import { updateStateReducer, initialUpdateState } from "./lib/autoUpdateState";
import { mergeMonitors } from "./lib/monitorMerge";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { save } from "@tauri-apps/plugin-dialog";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { replayEntrance } from "./lib/motion";
import logoMark from "./assets/chromadeck-icon.svg";
import MonitorList from "./components/MonitorList";
import MonitorSidebar from "./components/MonitorSidebar";
import ApplyDialog from "./components/ApplyDialog";
import PresetEditor from "./components/PresetEditor";
import ScheduleDialog from "./components/ScheduleDialog";
import UpdateCheckButton from "./components/updater/UpdateCheckButton";
import UpdateDialog from "./components/updater/UpdateDialog";
import { DragAndDropProvider } from "./components/DragAndDropContext";
import DragGhost from "./components/DragGhost";
import UndoToast from "./components/UndoToast";
import { Clock, FileDown } from "lucide-react";
import { cn } from "cn";

// ── Update suppression ─────────────────────────────────────────────────────
// When the user clicks "Remind me later", suppress the update dialog for 24h.
// Clicking "Check for updates" manually always overrides the suppression.

const UPDATE_SUPPRESSED_KEY = "chromadeck.updateSuppressedUntil";
const SUPPRESS_MS = 24 * 60 * 60 * 1000;

function getUpdateSuppressedUntil(): number | null {
  try {
    const raw = localStorage.getItem(UPDATE_SUPPRESSED_KEY);
    if (!raw) return null;
    const ts = Number(raw);
    return Number.isFinite(ts) ? ts : null;
  } catch {
    return null;
  }
}

function setUpdateSuppressedUntil(ts: number): void {
  try {
    localStorage.setItem(UPDATE_SUPPRESSED_KEY, String(ts));
  } catch {
    // localStorage may be unavailable in some environments; fail silently.
  }
}

function isUpdateSuppressed(): boolean {
  const until = getUpdateSuppressedUntil();
  return until !== null && until > Date.now();
}

function AutostartToggle() {
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    isEnabled().then(setOn).catch(() => setOn(false));
  }, []);
  const toggle = async () => {
    try {
      if (on) await disable();
      else await enable();
      setOn(!on);
    } catch {
      // keep current state on failure
    }
  };
  return (
    <label className="inline-flex cursor-pointer items-center gap-1.5">
      <input
        type="checkbox"
        checked={on ?? false}
        onChange={toggle}
        className="size-[14px] rounded-[4px] accent-[var(--color-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <span className="font-body text-[11px] uppercase tracking-[0.4px] text-muted-foreground whitespace-nowrap">
        Autostart
      </span>
    </label>
  );
}

/** Master switch for foreground auto-apply. Mirrors the tray checkbox. */
function AutoApplyToggle({ on, onChange }: { on: boolean; onChange: (next: boolean) => void }) {
  const [busy, setBusy] = useState(false);
  const toggle = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await setAutoApplyEnabled(!on);
      onChange(!on);
    } catch {
      // keep current state on failure
    } finally {
      setBusy(false);
    }
  };
  return (
    <label className="inline-flex cursor-pointer items-center gap-1.5">
      <input
        type="checkbox"
        checked={on}
        onChange={toggle}
        aria-label="Auto-apply mapped programs"
        className="size-[14px] rounded-[4px] accent-[var(--color-primary)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <span className="font-body text-[11px] uppercase tracking-[0.4px] text-muted-foreground whitespace-nowrap">
        Auto-apply
      </span>
    </label>
  );
}

function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingPreset, setEditingPreset] = useState<Preset | null>(null);
  const [showEditor, setShowEditor] = useState(false);
  const [pins, setPins] = useState<Record<string, string>>({});
  const [reapplyMsg, setReapplyMsg] = useState<string | null>(null);
  const [lastReapply, setLastReapply] = useState<{ errors: number; total: number } | null>(null);
  // Bumped on every Reapply so the footer health light remounts and replays
  // its one-shot ring even when the derived health state is unchanged.
  const [healthTick, setHealthTick] = useState(0);
  const [applyingPreset, setApplyingPreset] = useState<Preset | null>(null);
  const [applyTargetEdid, setApplyTargetEdid] = useState<string | null>(null);
  const [appliedMap, setAppliedMap] = useState<Record<string, string>>({});
  // Auto-apply is a separate, in-memory map: presets stay in the library.
  const [autoMap, setAutoMap] = useState<AutoMap>({});
  const [autoEnabled, setAutoEnabled] = useState(false);
  const [autoMsg, setAutoMsg] = useState<string | null>(null);
  const [lastAddedId, setLastAddedId] = useState<string | null>(null);
  const reapplyTimerRef = useRef<number | null>(null);
  const autoTimerRef = useRef<number | null>(null);
  const [updateState, dispatchUpdate] = useReducer(updateStateReducer, initialUpdateState());
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateDialogKind, setUpdateDialogKind] = useState<"available" | "up-to-date" | "downloading" | "installing" | "error">("available");
  const [updateMsg, setUpdateMsg] = useState<string | null>(null);
  const updateTimerRef = useRef<number | null>(null);
  const [scheduleDialogOpen, setScheduleDialogOpen] = useState(false);
  const [schedules, setSchedules] = useState<ScheduleRule[]>([]);
  const [exportingDiagnostics, setExportingDiagnostics] = useState(false);

  /** Transient footer note for auto-apply outcomes. */
  const showAutoMsg = (message: string) => {
    if (autoTimerRef.current !== null) {
      window.clearTimeout(autoTimerRef.current);
    }
    setAutoMsg(message);
    autoTimerRef.current = window.setTimeout(() => {
      autoTimerRef.current = null;
      setAutoMsg(null);
    }, 5000);
  };

  // Reapply status is transient: replace any pending clear, and clean up on unmount.
  const showReapplyMsg = (message: string) => {
    if (reapplyTimerRef.current !== null) {
      window.clearTimeout(reapplyTimerRef.current);
    }
    setReapplyMsg(message);
    reapplyTimerRef.current = window.setTimeout(() => {
      reapplyTimerRef.current = null;
      setReapplyMsg(null);
    }, 5000);
  };

  // Update check status: transient footer note for update check outcomes.
  const showUpdateMsg = (message: string) => {
    if (updateTimerRef.current !== null) {
      window.clearTimeout(updateTimerRef.current);
    }
    setUpdateMsg(message);
    updateTimerRef.current = window.setTimeout(() => {
      updateTimerRef.current = null;
      setUpdateMsg(null);
    }, 5000);
  };

  useEffect(
    () => () => {
      if (reapplyTimerRef.current !== null) {
        window.clearTimeout(reapplyTimerRef.current);
      }
      if (autoTimerRef.current !== null) {
        window.clearTimeout(autoTimerRef.current);
      }
      if (updateTimerRef.current !== null) {
        window.clearTimeout(updateTimerRef.current);
      }
    },
    [],
  );

  // Optimistic batch-delete removal: drop the deleted cards locally so no
  // reload flash interrupts the explosion, then sync quietly in the
  // background (pins/applied/footer) without touching the loading flag.
  // The removal runs inside a View Transition (Chromium/WebView2) so the
  // surviving cards glide into the freed gaps instead of jumping.
  const handleBatchDeleted = (ids: string[]) => {
    if (ids.length > 0) {
      const remove = () =>
        setPresets((prev) => prev.filter((p) => !ids.includes(p.id)));
      const doc = document as Document & {
        startViewTransition?: (cb: () => void) => void;
      };
      if (typeof doc.startViewTransition === "function") {
        doc.startViewTransition(() => {
          flushSync(remove);
        });
      } else {
        remove();
      }
      setLastAddedId((cur) => (cur !== null && ids.includes(cur) ? null : cur));
    }
    void fetchData(true);
  };
  // Optimistic duplicate insert: append the created preset locally so the
  // new card fades in on its own instead of reloading the whole library.
  const handlePresetDuplicated = (created: Preset) => {
    setPresets((prev) =>
      prev.some((p) => p.id === created.id) ? prev : [...prev, created],
    );
    setLastAddedId(created.id);
    window.setTimeout(() => {
      setLastAddedId((cur) => (cur === created.id ? null : cur));
    }, 600);
  };

  const fetchData = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    setError(null);
    try {
      const [monitorsData, presetsData, pinsData, appliedData, schedulesData] = await Promise.all([
        listMonitors(),
        listPresets(),
        listPins(),
        listApplied(),
        listSchedules().catch(() => [] as ScheduleRule[]),
      ]);
      setMonitors(monitorsData);
      setPresets(presetsData);
      setPins(pinsData);
      setAppliedMap(appliedData);
      setSchedules(Array.isArray(schedulesData) ? schedulesData : []);
    } catch (err) {
      setError(String(err));
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // ── Background poll: 5s interval + window focus refresh ──────────────
  // Fetches monitors ONLY (never presets/pins). Merges against current
  // state so disconnected monitors persist as red offline cards rather
  // than disappearing. Silent on failure — keeps last-known state.

  useEffect(() => {
    let mounted = true;
    let inFlight = false;

    const tick = async () => {
      if (!mounted || inFlight) return;
      inFlight = true;
      try {
        const fresh = await listMonitors();
        if (mounted) {
          setMonitors((prev) => mergeMonitors(prev, fresh));
        }
      } catch {
        // silent: keep last-known state on failure
      } finally {
        inFlight = false;
      }
    };

    const POLL_MS = 5000;
    const intervalId = setInterval(tick, POLL_MS);
    window.addEventListener("focus", tick);

    return () => {
      mounted = false;
      clearInterval(intervalId);
      window.removeEventListener("focus", tick);
    };
  }, []);

  // ── Entrance replay: the window hides (not unmounts) on close, so the
  // backend pings `window-shown` on every re-show (tray Show / tray click /
  // second launch). Replay the staged load animation in place.
  useEffect(() => {
    // `listen` needs Tauri IPC — skip it in the browser preview.
    if (!isTauri()) return;
    const unlistenPromise = listen("window-shown", replayEntrance);
    return () => {
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  // ── Auto-apply: seed markers + follow watcher outcomes ──────────────
  useEffect(() => {
    if (!isTauri()) return;
    let mounted = true;

    void listAutoApplied()
      .then((items) => {
        if (mounted && items.length > 0) {
          setAutoMap(
            Object.fromEntries(items.map((item) => [item.edid_id, item.preset_id])),
          );
        }
      })
      .catch(() => {});
    void getAutoApplyEnabled()
      .then((on) => {
        if (mounted) setAutoEnabled(on);
      })
      .catch(() => {});

    const outcome = listen<AutoApplyEvent>("auto-apply://outcome", (event) => {
      const payload = event.payload;
      setAutoMap((prev) => applyAutoEvent(prev, payload));
      if (payload.kind === "applied") {
        showAutoMsg(`Auto-applied ${payload.preset_name} to ${payload.monitor_name}`);
      } else if (payload.kind === "reverted") {
        showAutoMsg(
          payload.restored_preset_name
            ? `Restored ${payload.restored_preset_name} on ${payload.monitor_name}`
            : `Reverted ${payload.monitor_name}`,
        );
      } else if (payload.kind === "skipped") {
        showAutoMsg(`Auto-apply skipped: ${payload.error ?? "monitor is pinned"}`);
      } else if (payload.kind === "error") {
        showAutoMsg(`Auto-apply failed: ${payload.error ?? "unknown error"}`);
      }
    });
    const enabled = listen<boolean>("auto-apply://enabled", (event) => {
      setAutoEnabled(event.payload);
    });
    const storeChanged = listen("store://changed", () => {
      void fetchData(true);
    });
    const monitorsChanged = listen("monitors://changed", () => {
      void listMonitors()
        .then((fresh) => {
          if (mounted) {
            setMonitors((prev) => mergeMonitors(prev, fresh));
          }
        })
        .catch(() => {});
    });
    const hotkeyTriggered = listen<HotkeyEvent>("hotkey://triggered", (event) => {
      const p = event.payload;
      showReapplyMsg(p.error ? `${p.message}: ${p.error}` : p.message);
      void fetchData(true);
    });
    const scheduleOutcome = listen<ScheduleOutcomeEvent>("schedule://outcome", (event) => {
      const p = event.payload;
      if (p.kind === "applied") {
        showAutoMsg(`Scheduled ${p.preset_name} on ${p.monitor_name}`);
      } else if (p.kind === "reverted") {
        showAutoMsg(
          p.restored_preset_name
            ? `Schedule ended: restored ${p.restored_preset_name} on ${p.monitor_name}`
            : `Schedule ended: reset ${p.monitor_name}`,
        );
      } else if (p.kind === "error") {
        showAutoMsg(`Schedule failed: ${p.error ?? "unknown error"}`);
      }
    });

    return () => {
      mounted = false;
      outcome.then((unlisten) => unlisten()).catch(() => {});
      enabled.then((unlisten) => unlisten()).catch(() => {});
      storeChanged.then((unlisten) => unlisten()).catch(() => {});
      monitorsChanged.then((unlisten) => unlisten()).catch(() => {});
      hotkeyTriggered.then((unlisten) => unlisten()).catch(() => {});
      scheduleOutcome.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [fetchData]);

  // ── Auto-updater: check on startup + follow progress events ───────────
  const updateInFlight = useRef(false);
  const updateCheckGeneration = useRef(0);

  useEffect(() => {
    if (!isTauri()) return;
    let mounted = true;
    const generation = ++updateCheckGeneration.current;

    if (!isUpdateSuppressed()) void checkForUpdate()
      .then((info) => {
        if (!mounted || generation !== updateCheckGeneration.current || updateInFlight.current) return;
        dispatchUpdate({
          type: "CHECK_SUCCESS",
          payload: {
            available: info.available,
            version: info.version,
            notes: info.notes,
            date: info.date,
          },
        });
        // Only show the dialog on startup if an update is available.
        if (info.available) {
          setUpdateDialogOpen(true);
          setUpdateDialogKind("available");
        } else {
          showUpdateMsg("You're up to date");
        }
      })
      .catch((err) => {
        if (!mounted || generation !== updateCheckGeneration.current || updateInFlight.current) return;
        dispatchUpdate({ type: "CHECK_FAILURE", payload: { message: String(err) } });
        showUpdateMsg(`Update check failed: ${String(err)}`);
      });

    const progress = listen<{ downloaded: number; total: number | null; percent: number | null }>(
      "update://progress",
      (event) => {
        const payload = event.payload;
        if (mounted) {
          dispatchUpdate({
            type: "DOWNLOAD_PROGRESS",
            payload,
          });

        }
      },
    );

    const installing = listen("update://installing", () => {
      if (!mounted) return;
      dispatchUpdate({ type: "DOWNLOAD_SUCCESS" });
      setUpdateDialogKind("installing");
    });

    return () => {
      mounted = false;
      progress.then((unlisten) => unlisten()).catch(() => {});
      installing.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  const handleCheckUpdate = async () => {
    if (updateInFlight.current) return;
    const generation = ++updateCheckGeneration.current;
    // Manual check always overrides suppression.
    setUpdateSuppressedUntil(0);
    dispatchUpdate({ type: "CHECK_START" });
    try {
      const info = await checkForUpdate();
      if (generation !== updateCheckGeneration.current || updateInFlight.current) return;
      dispatchUpdate({
        type: "CHECK_SUCCESS",
        payload: {
          available: info.available,
          version: info.version,
          notes: info.notes,
          date: info.date,
        },
      });
      // Always show the dialog on manual check.
      setUpdateDialogOpen(true);
      setUpdateDialogKind(info.available ? "available" : "up-to-date");
    } catch (err) {
      if (generation !== updateCheckGeneration.current || updateInFlight.current) return;
      dispatchUpdate({ type: "CHECK_FAILURE", payload: { message: String(err) } });
      showUpdateMsg(`Update check failed: ${String(err)}`);
      setUpdateDialogKind("error");
      setUpdateDialogOpen(true);
    }
  };

  const handleDownloadAndInstall = async () => {
    if (updateInFlight.current) return;
    updateInFlight.current = true;
    updateCheckGeneration.current++;
    setUpdateDialogKind("downloading");
    setUpdateDialogOpen(true);
    dispatchUpdate({ type: "DOWNLOAD_START" });
    try {
      await downloadAndInstallUpdate();
      // Windows hands off to the installer and exits. A returning command
      // must not offer a restart that would download the update again.
      setUpdateDialogOpen(false);
      showUpdateMsg("Update installation completed");
    } catch (err) {
      dispatchUpdate({ type: "DOWNLOAD_FAILURE", payload: { message: String(err) } });
      setUpdateDialogKind("error");
      setUpdateDialogOpen(true);
    } finally {
      updateInFlight.current = false;
    }
  };

  const handleRemindMeLater = () => {
    // Suppress the update dialog for 24 hours.
    setUpdateSuppressedUntil(Date.now() + SUPPRESS_MS);
    setUpdateDialogOpen(false);
  };

  const handleEdit = (preset: Preset) => {
    setEditingPreset(preset);
    setShowEditor(true);
  };

  const handleCreateNew = () => {
    setEditingPreset(null);
    setShowEditor(true);
  };

  const handleEditorSaved = () => {
    // The editor closes itself and reports back through onClosed; only the
    // data needs refreshing here.
    fetchData();
  };

  /** Dialog close requests are recorded inside the dialog. The parent keeps
   *  the dialog mounted until `onClosed` reports the exit animation ended. */
  const handleDialogCloseRequest = () => {};

  const handleEditorClosed = () => {
    setShowEditor(false);
    setEditingPreset(null);
  };

  const handleExportDiagnostics = async () => {
    if (exportingDiagnostics) return;
    setExportingDiagnostics(true);
    try {
      const defaultName = `chromadeck-diagnostics-v${appVersion}.log`;
      const destPath = isTauri()
        ? await save({
            title: "Export ChromaDeck Diagnostics Report",
            defaultPath: defaultName,
            filters: [{ name: "Log / Text Report", extensions: ["log", "txt"] }],
          })
        : defaultName;
      if (!destPath) return;
      const summary = await exportDiagnostics(destPath);
      showReapplyMsg(
        `Diagnostics exported (${summary.monitors_count} monitor${summary.monitors_count === 1 ? "" : "s"})`,
      );
    } catch (err) {
      showReapplyMsg(`Diagnostics export failed: ${String(err)}`);
    } finally {
      setExportingDiagnostics(false);
    }
  };

  function LogoMark() {
    return (
      <img
        src={logoMark}
        className="logo-lockup"
        alt="ChromaDeck logo"
      />
    );
  }

  return (
    <DragAndDropProvider
      monitors={monitors}
      appliedMap={appliedMap}
      onApplied={(presetId, edids) => {
        // Optimistic: reflect the drop immediately, then re-sync quietly.
        setAppliedMap((prev) => {
          const next = { ...prev };
          for (const edid of edids) next[edid] = presetId;
          return next;
        });
        void fetchData(true);
      }}
      onRefresh={() => void fetchData(true)}
    >
    <main className="flex h-screen flex-col overflow-hidden bg-background text-foreground">
      {/* ── Header: flat strip ─────────────────────────────────────────── */}
      <header
        className="shell-enter flex h-12 shrink-0 items-center justify-between border-b border-border bg-background px-6"
        style={{ ["--shell-delay" as string]: "0ms" }}
      >
        <div className="flex min-w-0 items-center gap-3">
          <LogoMark />
          <h1 className="enter-reveal truncate text-[15px] font-semibold tracking-[-0.02em] text-foreground">
            <span className="enter-reveal-child">ChromaDeck</span>
          </h1>
          <span className="h-4 w-px shrink-0 bg-border" aria-hidden="true" />
          <span className="mono shrink-0 text-[10px] text-muted-foreground">
            v{appVersion}
          </span>
        </div>
      </header>

      {/* ── Error banner ────────────────────────────────────────────────── */}
      {error && (
        <div className="error-banner-enter mx-6 mt-4 flex items-center gap-2.5 rounded-[8px] border border-border bg-surface px-3 py-2">
          <svg className="size-4 shrink-0 text-destructive-text" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v4m0 4h.01M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z" />
          </svg>
          <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-destructive-text" title={`FAILED TO LOAD: ${error}`}>
            FAILED TO LOAD: {error}
          </span>
          <button onClick={() => void fetchData()} data-slot="button" className="inline-flex shrink-0 items-center justify-center rounded-[8px] border border-border bg-transparent px-2.5 py-1 font-mono text-[10px] text-foreground transition-colors hover:border-border-hover hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring">
            <span className="btn-label">Retry</span>
          </button>
        </div>
      )}

      {/* ── Content: two stacked panels, monitors capped & scrollable, presets absorb extra ─── */}
      <div className="flex min-h-0 flex-1 flex-col gap-4 p-6">
        <div
          className={cn(
            "flex min-h-0 flex-[0_0_auto] shrink-0 flex-col overflow-hidden",
            (loading || monitors.length > 0) && "min-h-[176px]",
          )}
          style={{
            maxHeight:
              loading || monitors.length > 0
                ? "min(500px, max(176px, calc(100vh - 404px)))"
                : undefined,
          }}
        >
          <MonitorSidebar
            monitors={monitors}
            presets={presets}
            loading={loading}
            pins={pins}
            appliedMap={appliedMap}
            autoMap={autoMap}
            setAppliedMap={setAppliedMap}
            onRefresh={fetchData}
            onPinChange={fetchData}
          />
        </div>
        <div className="flex min-h-0 flex-1">
          <MonitorList
            monitors={monitors}
            presets={presets}
            loading={loading}
            onEdit={handleEdit}
            onRefresh={fetchData}
            onCreateNew={handleCreateNew}
            pins={pins}
            onPinChange={fetchData}
            appliedMap={appliedMap}
            autoMap={autoMap}
            onPresetDuplicated={handlePresetDuplicated}
            onPresetDeleted={(id) => handleBatchDeleted([id])}
            lastAddedId={lastAddedId}
            onStatusMessage={showReapplyMsg}
            onApply={(preset) => { setApplyTargetEdid(null); setApplyingPreset(preset); }}
          />
        </div>
      </div>

      {/* ── Footer / status bar: flat strip ──────────────────────────── */}
      <footer
        className="shell-enter flex h-14 shrink-0 items-center justify-between gap-3 border-t border-border bg-background pl-6 pr-4"
        style={{ ["--shell-delay" as string]: "180ms" }}
      >
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          {(() => {
            const errors = lastReapply?.errors ?? 0;
            const healthKey: "loading" | "ok" | "error" | "empty" = loading
              ? "loading"
              : errors > 0
                ? "error"
                : monitors.some((m) => m.connected)
                  ? "ok"
                  : "empty";
            const health =
              healthKey === "error"
                ? {
                    label: `Last reapply reported ${errors} error${errors === 1 ? "" : "s"}`,
                    tone: "bg-destructive animate-pulse motion-reduce:animate-none",
                  }
                : healthKey === "ok"
                  ? { label: "Enforcement OK", tone: "bg-primary" }
                  : healthKey === "loading"
                    ? { label: "Loading monitor status", tone: "bg-subtle-fg" }
                    : { label: "No monitors connected", tone: "bg-subtle-fg" };
            return (
              <span
                // Remount on every health change (and every Reapply) so the
                // .health-pulse ring animation replays once.
                key={`${healthKey}-${healthTick}`}
                role="img"
                aria-label={health.label}
                title={health.label}
                className={`health-pulse relative inline-block size-1.5 shrink-0 rounded-full ${health.tone}`}
              />
            );
          })()}
          {(() => {
            const t = loading
              ? "LOADING…"
              : `${presets.length} PRESET${presets.length !== 1 ? "S" : ""} · ${monitors.filter((m) => m.connected).length} MONITOR${monitors.filter((m) => m.connected).length !== 1 ? "S" : ""} CONNECTED · ${Object.keys(pins).length} PINNED`;
            return (
              <span
                key={t}
                title={t}
                className="value-swap font-body min-w-0 truncate text-[11px] uppercase tracking-[0.4px] tabular-nums text-muted-foreground"
              >
                {t}
              </span>
            );
          })()}
          {reapplyMsg && (
            <>
              <span className="h-4 w-px shrink-0 bg-border" aria-hidden="true" />
              <span title={reapplyMsg} className="transient-enter font-body min-w-[72px] shrink truncate text-[11px] text-muted-foreground">
                {reapplyMsg}
              </span>
            </>
          )}
          {autoMsg && (
            <>
              <span className="h-4 w-px shrink-0 bg-border" aria-hidden="true" />
              <span title={autoMsg} className="transient-enter font-body min-w-[72px] shrink truncate text-[11px] text-muted-foreground">
                {autoMsg}
              </span>
            </>
          )}
          {updateMsg && (
            <>
              <span className="h-4 w-px shrink-0 bg-border" aria-hidden="true" />
              <span title={updateMsg} className="transient-enter font-body min-w-[72px] shrink truncate text-[11px] text-muted-foreground">
                {updateMsg}
              </span>
            </>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2.5 min-[920px]:gap-3.5">
          <span className="h-4 w-px bg-border" aria-hidden="true" />
          <AutoApplyToggle on={autoEnabled} onChange={setAutoEnabled} />
          <AutostartToggle />
          <button
            type="button"
            data-slot="button"
            onClick={() => setScheduleDialogOpen(true)}
            title="Configure time-of-day display schedules"
            aria-label="Open display schedules"
            className="inline-flex items-center justify-center gap-1.5 rounded-[8px] border border-border bg-transparent px-2.5 py-1.5 font-body text-[11px] uppercase tracking-[0.4px] text-foreground transition-colors hover:border-border-hover hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
          >
            <Clock className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <span className="btn-label">Schedules</span>
            {schedules.filter((s) => s.enabled).length > 0 && (
              <span className="rounded-full bg-primary-subtle px-1.5 py-0.2 font-mono text-[9px] text-primary">
                {schedules.filter((s) => s.enabled).length}
              </span>
            )}
          </button>
          <button
            type="button"
            data-slot="button"
            onClick={() => void handleExportDiagnostics()}
            disabled={exportingDiagnostics}
            title="Export redacted hardware and log diagnostics report"
            aria-label="Export diagnostics report"
            className="inline-flex items-center justify-center gap-1.5 rounded-[8px] border border-border bg-transparent px-2 py-1.5 font-body text-[11px] uppercase tracking-[0.4px] text-muted-foreground transition-colors hover:border-border-hover hover:bg-surface-hover hover:text-foreground focus-visible:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring disabled:opacity-50"
          >
            <FileDown className="size-3.5 shrink-0" aria-hidden="true" />
            <span className="hidden btn-label min-[1080px]:inline">
              {exportingDiagnostics ? "Exporting…" : "Diagnostics"}
            </span>
          </button>
          <button
            data-slot="button"
            onClick={async () => {
              try {
                const events: EnforceEvent[] = await reapplyNow();
                setLastReapply({
                  errors: events.filter((e) => e.error).length,
                  total: events.length,
                });
                setHealthTick((tick) => tick + 1);
                const applied = events.filter((e) => e.applied).length;
                const firstErr = events.find((e) => e.error)?.error;
                showReapplyMsg(firstErr ? `Reapply: ${firstErr}` : `Reapplied ${applied}/${events.length}`);
              } catch (err) {
                showReapplyMsg(`Reapply failed: ${String(err)}`);
              }
              fetchData();
            }}
            className="inline-flex items-center justify-center rounded-[8px] border border-border bg-transparent px-2.5 py-1.5 font-body text-[11px] uppercase tracking-[0.4px] text-foreground transition-colors hover:border-border-hover hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
            title="Re-run enforcement now"
          >
            <span className="btn-label">Reapply</span>
          </button>
          <UpdateCheckButton
            checking={updateState.status === "checking"}
            onCheck={handleCheckUpdate}
          />
        </div>
      </footer>

      {/* ── Apply dialog ──────────────────────────────────────────────── */}
      {applyingPreset && (
        <ApplyDialog
          preset={applyingPreset}
          monitors={monitors}
          pins={pins}
          initialEdid={applyTargetEdid}
          onClose={handleDialogCloseRequest}
          onClosed={() => {
            setApplyingPreset(null);
            setApplyTargetEdid(null);
          }}
          onApplied={(info) => {
            if (info) {
              setAppliedMap((prev) => ({ ...prev, [info.edid]: info.presetId }));
            }
            fetchData();
          }}
        />
      )}

      {/* ── Editor dialog ──────────────────────────────────────────────── */}
      {showEditor && (
        <PresetEditor
          presets={presets}
          monitors={monitors}
          editPreset={editingPreset}
          onClose={handleDialogCloseRequest}
          onClosed={handleEditorClosed}
          onSaved={handleEditorSaved}
        />
      )}

      {/* ── Schedule dialog ────────────────────────────────────────────── */}
      {scheduleDialogOpen && (
        <ScheduleDialog
          open={scheduleDialogOpen}
          monitors={monitors}
          presets={presets}
          onClose={() => setScheduleDialogOpen(false)}
          onSaved={(saved) => {
            setSchedules(saved);
            const activeCount = saved.filter((s) => s.enabled).length;
            showReapplyMsg(
              activeCount > 0
                ? `Saved ${activeCount} active schedule${activeCount === 1 ? "" : "s"}`
                : "Schedules cleared",
            );
          }}
        />
      )}

      {/* ── Drag overlay + undo toast (portals) ────────────────────────── */}
      <DragGhost />
      <UndoToast />

      {/* ── Update dialog ──────────────────────────────────────────────── */}
      {updateDialogOpen && (
        <UpdateDialog
          open={updateDialogOpen}
          onClose={() => setUpdateDialogOpen(false)}
          onDownloadAndInstall={handleDownloadAndInstall}
          onRetry={handleCheckUpdate}
          error={updateState.status === "error" ? updateState.message : null}
          onRemindMeLater={handleRemindMeLater}
          update={{
            available: updateDialogKind === "available",
            version: "version" in updateState ? updateState.version : appVersion,
            notes: "notes" in updateState ? updateState.notes : null,
            date: "date" in updateState ? updateState.date : null,
          }}
          kind={updateDialogKind}
          progress={
            updateState.status === "downloading"
              ? updateState.percent
              : null
          }
        />
      )}
    </main>
    </DragAndDropProvider>
  );
}

export default App;