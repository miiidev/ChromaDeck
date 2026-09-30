import { useState, useEffect, useCallback } from "react";
import "./App.css";
import { listMonitors, listPresets, listPins, reapplyNow } from "./lib/tauri";
import type { Monitor, Preset, EnforceEvent } from "./lib/types";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import MonitorList from "./components/MonitorList";
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
    <label className="inline-flex items-center gap-1.5 text-xs text-neutral-500 cursor-pointer">
      <input type="checkbox" checked={on ?? false} onChange={toggle} className="accent-lime-400" />
      <span className="uppercase tracking-widest text-[10px]">Start with Windows</span>
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
  const [createForMonitorId, setCreateForMonitorId] = useState<string | null>(null);

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
    setCreateForMonitorId(null);
    setShowEditor(true);
  };

  const handleCreateNew = (edidId: string) => {
    setEditingPreset(null);
    setCreateForMonitorId(edidId);
    setShowEditor(true);
  };

  const handleEditorClose = () => {
    setShowEditor(false);
    setEditingPreset(null);
    setCreateForMonitorId(null);
  };

  const handleEditorSaved = () => {
    setShowEditor(false);
    setEditingPreset(null);
    setCreateForMonitorId(null);
    fetchData();
  };

  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      {/* ── Header ──────────────────────────────────────────────────────── */}
      <header className="border-b-2 border-neutral-200 px-6 py-4">
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 border-2 border-neutral-200 bg-gradient-to-br from-lime-400 to-lime-500 flex items-center justify-center">
            <svg className="w-4 h-4 text-neutral-950" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
            </svg>
          </div>
          <h1 className="text-xl font-semibold tracking-tight">ChromaDeck</h1>
          <span className="border-2 border-neutral-200 px-1.5 py-0.5 font-mono text-xs text-neutral-500">
            v0.1.0
          </span>
        </div>
      </header>

      {/* ── Error banner ────────────────────────────────────────────────── */}
      {error && (
        <div className="mx-6 mt-4 px-4 py-3 border-2 border-red-400 bg-red-950/40 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg className="w-4 h-4 text-red-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
            </svg>
            <span className="text-sm text-red-300">Failed to load: {error}</span>
          </div>
          <button
            onClick={fetchData}
            className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-800 text-neutral-300 hover:bg-neutral-700"
          >
            RETRY
          </button>
        </div>
      )}

      {/* ── Library view ────────────────────────────────────────────────── */}
      <MonitorList
        monitors={monitors}
        presets={presets}
        loading={loading}
        onEdit={handleEdit}
        onRefresh={fetchData}
        onCreateNew={handleCreateNew}
        pins={pins}
        onPinChange={fetchData}
      />

      {/* ── Footer / status bar ─────────────────────────────────────────── */}
      <footer className="border-t-2 border-neutral-200 px-6 py-3 flex items-center justify-between text-xs text-neutral-500">
        <span className="uppercase tracking-widest">
          {loading ? "LOADING…" : `${presets.length} PRESET${presets.length !== 1 ? "S" : ""} · ${monitors.filter((m) => m.connected).length} MONITOR${monitors.filter((m) => m.connected).length !== 1 ? "S" : ""} CONNECTED · ${Object.keys(pins).length} PINNED`}
        </span>
        <span className="inline-flex items-center gap-3">
          {reapplyMsg && <span className="text-neutral-400">{reapplyMsg}</span>}
          {!loading && (
            <span className="font-mono">
              {new Date().toLocaleTimeString()}
            </span>
          )}
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
            className="brutalist-btn px-2 py-1 text-xs font-medium border-2 border-neutral-200 shadow-[2px_2px_0px_#e5e7eb] active:translate-y-0.5 active:shadow-none motion-reduce:active:translate-y-0 bg-neutral-900 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
            title="Re-run enforcement now"
          >
            REAPPLY
          </button>
          <AutostartToggle />
        </span>
      </footer>

      {/* ── Editor modal ────────────────────────────────────────────────── */}
      {showEditor && (
        <PresetEditor
          monitors={monitors}
          editPreset={editingPreset}
          defaultMonitorId={createForMonitorId}
          onClose={handleEditorClose}
          onSaved={handleEditorSaved}
        />
      )}
    </main>
  );
}

export default App;