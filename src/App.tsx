import { useState, useEffect, useCallback } from "react";
import "./App.css";
import { listMonitors, listPresets, listPins, reapplyNow } from "./lib/tauri";
import type { Monitor, Preset, EnforceEvent } from "./lib/types";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import logoLockupLight from "./assets/logo-lockup-lm.png";
import logoLockupDark from "./assets/logo-lockup-dm.png";
import { useTheme } from "./lib/theme";
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
      <input type="checkbox" checked={on ?? false} onChange={toggle} className="accent-blue" />
      <span className="text-secondary uppercase tracking-widest text-[10px]">Start with Windows</span>
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
  // Last-applied preset per monitor this session (edid_id -> preset.id)
  const [appliedMap, setAppliedMap] = useState<Record<string, string>>({});
  const { theme, toggle: toggleTheme } = useTheme();

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [monitorsData, presetsData, pinsData] = await Promise.all([
        listMonitors(),
        listPresets(),
        listPins(),
      ]);
      setMonitors(monitorsData);
      setPresets(presetsData);
      setPins(pinsData);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

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

  /** Theme-aware product mark: light deck on light theme, dark deck on dark. */
  function LogoMark() {
    const light = theme === "bauhaus-light";
    return (
      <img
        src={light ? logoLockupLight : logoLockupDark}
        className="logo-lockup"
        alt="ChromaDeck logo"
      />
    );
  }

  return (
    <main className="min-h-screen bg-paper text-ink flex flex-col">
      {/* ── Header — asymmetric grid ─────────────────────────────────────── */}
      <header className="border-b-2 border-ink px-6 py-4">
        <div className="flex items-center justify-between gap-x-4">
          {/* Left cluster: logo mark + title + version */}
          <div className="flex min-w-0 items-center gap-x-4">
            <LogoMark />

            <h1 className="truncate text-[16px] text-ink" style={{ letterSpacing: "-0.024em" }}>
              <span className="font-bold">C</span>hromaDec<span className="font-bold">k</span>
            </h1>

            <span className="shrink-0 border-2 border-ink px-1.5 py-0.5 text-xs text-secondary font-mono">
              v0.2.0
            </span>
          </div>

          {/* Right cluster: theme toggle */}
          <button
            onClick={toggleTheme}
            className="bauhaus-btn px-3 py-1.5 text-xs font-medium border-2 border-ink bg-surface text-ink hover:bg-surface-hover shadow-btn"
            aria-label={`Switch to ${theme === "bauhaus-light" ? "dark" : "light"} theme`}
            title={`Current: ${theme === "bauhaus-light" ? "Light" : "Dark"} — click to toggle`}
          >
            <span className="inline-flex items-center gap-1.5">
              <span className="text-base leading-none">{theme === "bauhaus-light" ? "◐" : "●"}</span>
              <span className="uppercase tracking-widest text-[10px]">{theme === "bauhaus-light" ? "LIGHT" : "DARK"}</span>
            </span>
          </button>
        </div>
      </header>

      {/* ── Error banner — ink-bordered block with red header bar ─────────── */}
      {error && (
        <div className="mx-6 mt-4 border-2 border-ink bg-surface shadow-btn motion-reduce:shadow-none">
          <div className="bg-primary-red px-4 py-1.5">
            <span className="text-xs font-medium text-ink" style={{ color: "white" }}>
              Failed to load: {error}
            </span>
          </div>
          <div className="flex items-center justify-end px-4 py-2">
            <button
              onClick={fetchData}
              className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn"
            >
              RETRY
            </button>
          </div>
        </div>
      )}

      {/* ── Content row: sidebar + library ──────────────────────────── */}
      <div className="flex-1 flex flex-col md:flex-row gap-5 items-start p-6">
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
          onApply={(preset) => { setApplyTargetEdid(null); setApplyingPreset(preset); }}
        />
      </div>

      {/* ── Footer / status bar ─────────────────────────────────────────── */}
      <footer className="border-t-2 border-ink px-6 py-3 flex flex-col sm:flex-row items-center sm:items-center justify-between text-xs gap-1">
        <span className="text-secondary uppercase tracking-widest">
          {loading ? "LOADING…" : `${presets.length} PRESET${presets.length !== 1 ? "S" : ""} · ${monitors.filter((m) => m.connected).length} MONITOR${monitors.filter((m) => m.connected).length !== 1 ? "S" : ""} CONNECTED · ${Object.keys(pins).length} PINNED`}
        </span>
        <span className="inline-flex items-center gap-3">
          {reapplyMsg && <span className="text-muted">{reapplyMsg}</span>}
          <button
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
            className="bauhaus-btn px-2 py-1 text-xs font-medium border-2 border-ink bg-surface text-secondary hover:bg-surface-hover shadow-btn"
            title="Re-run enforcement now"
          >
            REAPPLY
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

      {/* ── Editor modal ────────────────────────────────────────────────── */}
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