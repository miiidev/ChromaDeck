import { useState, useEffect, useCallback } from "react";
import { flushSync } from "react-dom";
import "./App.css";
import "@fontsource/montserrat/700.css";
import "@fontsource/montserrat/800.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/700.css";
import { listMonitors, listPresets, listPins, listApplied, reapplyNow } from "./lib/tauri";
import { version as appVersion } from "../package.json";
import type { Monitor, Preset, EnforceEvent } from "./lib/types";
import { mergeMonitors } from "./lib/monitorMerge";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { listen } from "@tauri-apps/api/event";
import { replayEntrance } from "./lib/motion";
import logoMark from "./assets/chromadeck-icon.svg";
import MonitorList from "./components/MonitorList";
import MonitorSidebar from "./components/MonitorSidebar";
import ApplyDialog from "./components/ApplyDialog";
import PresetEditor from "./components/PresetEditor";

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
    <label className="inline-flex items-center gap-1.5 text-xs cursor-pointer">
      <input type="checkbox" checked={on ?? false} onChange={toggle} className="accent-primary" />
      <span className="uppercase tracking-widest text-[10px] text-muted-foreground">
        Start with Windows
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
  const [applyingPreset, setApplyingPreset] = useState<Preset | null>(null);
  const [applyTargetEdid, setApplyTargetEdid] = useState<string | null>(null);
  const [appliedMap, setAppliedMap] = useState<Record<string, string>>({});
  const [lastAddedId, setLastAddedId] = useState<string | null>(null);

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
      const [monitorsData, presetsData, pinsData, appliedData] = await Promise.all([
        listMonitors(),
        listPresets(),
        listPins(),
        listApplied(),
      ]);
      setMonitors(monitorsData);
      setPresets(presetsData);
      setPins(pinsData);
      setAppliedMap(appliedData);
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
    const unlistenPromise = listen("window-shown", replayEntrance);
    return () => {
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  const handleEdit = (preset: Preset) => {
    setEditingPreset(preset);
    setShowEditor(true);
  };

  const handleCreateNew = () => {
    setEditingPreset(null);
    setShowEditor(true);
  };

  const handleEditorClose = () => {
    setShowEditor(false);
    setEditingPreset(null);
  };

  const handleEditorSaved = () => {
    setShowEditor(false);
    setEditingPreset(null);
    fetchData();
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
    <main className="h-screen overflow-hidden bg-background text-foreground flex flex-col">
      {/* ── Header ──────────────────────────────────────────────────────── */}
      <header className="border-b-2 border-ink bg-white px-6 py-3 shell-enter" style={{ ["--shell-delay" as string]: "0ms" }}>
        <div className="flex items-center justify-between gap-x-4">
          <div className="flex min-w-0 items-center gap-x-4">
            <LogoMark />
            <h1 className="truncate font-display text-lg font-extrabold uppercase tracking-wide text-ink">
              <span className="font-bold">C</span>hromaDec<span className="font-bold">k</span>
            </h1>
            <span className="rounded-lg border-2 border-ink bg-mint px-1.5 py-0.5 text-xs font-bold text-ink mono">
              v{appVersion}
            </span>
          </div>
          <div className="flex items-center gap-3" />
        </div>
      </header>

      {/* ── Error banner ────────────────────────────────────────────────── */}
      {error && (
        <div className="mx-6 mt-4 rounded-brutal border-2 border-ink bg-white shadow-brutal-sm error-banner-enter">
          <div className="bg-danger px-4 py-1.5 rounded-t-lg">
            <span className="text-xs font-bold text-ink">
              Failed to load: {error}
            </span>
          </div>
          <div className="flex items-center justify-end px-4 py-2">
            <button onClick={() => void fetchData()} data-slot="button" className="inline-flex items-center justify-center rounded-lg border-2 border-ink bg-white px-2 py-1 text-xs font-bold text-ink shadow-brutal-sm hover:bg-muted">
              <span className="btn-label">Retry</span>
            </button>
          </div>
        </div>
      )}

      {/* ── Content column: monitor strip on top, library below ─── */}
      {/* Capped + centered so ultrawide/maximized windows get margins, not stretched pads */}
      <div className="flex-1 min-h-0 mx-auto w-full max-w-7xl flex flex-col gap-5 p-6">
        <MonitorSidebar
          monitors={monitors}
          presets={presets}
          pins={pins}
          onRefresh={fetchData}
          onPinChange={fetchData}
          onApplyFor={(preset, edidId) => { setApplyTargetEdid(edidId); setApplyingPreset(preset); }}
        />
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
          onPresetDuplicated={handlePresetDuplicated}
          onBatchDeleted={handleBatchDeleted}
          onPresetDeleted={(id) => handleBatchDeleted([id])}
          lastAddedId={lastAddedId}
          onApply={(preset) => { setApplyTargetEdid(null); setApplyingPreset(preset); }}
        />
      </div>

      {/* ── Footer / status bar ─────────────────────────────────────────── */}
      <footer className="border-t-2 border-ink bg-white px-6 py-3 flex flex-col sm:flex-row items-center sm:items-center justify-between text-xs gap-1 shell-enter" style={{ ["--shell-delay" as string]: "180ms" }}>
        {(() => {
          const t = loading ? "LOADING…" : `${presets.length} PRESET${presets.length !== 1 ? "S" : ""} · ${monitors.filter((m) => m.connected).length} MONITOR${monitors.filter((m) => m.connected).length !== 1 ? "S" : ""} CONNECTED · ${Object.keys(pins).length} PINNED`;
          return <span key={t} className="uppercase tracking-widest mono text-muted-foreground footer-crossfade">{t}</span>;
        })()}
        <span className="inline-flex items-center gap-3">
          {reapplyMsg && <span className="text-muted-foreground transient-enter">{reapplyMsg}</span>}
          <button
            data-slot="button"
            onClick={async () => {
              try {
                const events: EnforceEvent[] = await reapplyNow();
                const applied = events.filter((e) => e.applied).length;
                const firstErr = events.find((e) => e.error)?.error;
                setReapplyMsg(firstErr ? `Reapply: ${firstErr}` : `Reapplied ${applied}/${events.length}`);
              } catch (err) {
                setReapplyMsg(`Reapply failed: ${String(err)}`);
              }
              setTimeout(() => setReapplyMsg(null), 5000);
              fetchData();
            }}
            className="inline-flex items-center justify-center rounded-lg border-2 border-ink bg-white px-2 py-1 text-xs font-bold text-ink shadow-brutal-sm hover:bg-muted"
            title="Re-run enforcement now"
          >
            <span className="btn-label">Reapply</span>
          </button>
          <AutostartToggle />
        </span>
      </footer>

      {/* ── Apply dialog ──────────────────────────────────────────────── */}
      {applyingPreset && (
        <ApplyDialog
          preset={applyingPreset}
          monitors={monitors}
          pins={pins}
          initialEdid={applyTargetEdid}
          onClose={() => { setApplyingPreset(null); setApplyTargetEdid(null); }}
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
          monitors={monitors}
          editPreset={editingPreset}
          onClose={handleEditorClose}
          onSaved={handleEditorSaved}
        />
      )}
    </main>
  );
}

export default App;