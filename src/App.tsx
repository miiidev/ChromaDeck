import { useState, useEffect, useCallback } from "react";
import "./App.css";
import { listMonitors, listPresets, listPins, reapplyNow } from "./lib/tauri";
import type { Monitor, Preset, EnforceEvent } from "./lib/types";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import logoLockupDark from "./assets/logo-lockup-dm.png";
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

  function LogoMark() {
    return (
      <img
        src={logoLockupDark}
        className="logo-lockup"
        alt="ChromaDeck logo"
      />
    );
  }

  return (
    <main className="min-h-screen bg-background text-foreground flex flex-col">
      {/* ── Header ──────────────────────────────────────────────────────── */}
      <header className="border-b border-border bg-card px-6 py-3">
        <div className="flex items-center justify-between gap-x-4">
          <div className="flex min-w-0 items-center gap-x-4">
            <LogoMark />
            <h1 className="truncate text-base text-foreground font-sans" style={{ letterSpacing: "-0.024em" }}>
              <span className="font-bold">C</span>hromaDec<span className="font-bold">k</span>
            </h1>
            <span className="rounded-lg border border-border bg-popover px-1.5 py-0.5 text-xs text-muted-foreground mono">
              v0.4.0
            </span>
          </div>
          <div className="flex items-center gap-3" />
        </div>
      </header>

      {/* ── Error banner ────────────────────────────────────────────────── */}
      {error && (
        <div className="mx-6 mt-4 rounded-lg border border-border bg-card">
          <div className="bg-destructive/20 px-4 py-1.5 rounded-md">
            <span className="text-xs font-medium text-foreground">
              Failed to load: {error}
            </span>
          </div>
          <div className="flex items-center justify-end px-4 py-2">
            <button onClick={fetchData} className="inline-flex items-center justify-center rounded-lg border border-border bg-card px-2 py-1 text-xs font-medium text-muted-foreground hover:bg-muted hover:text-foreground active:translate-y-px">
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
      <footer className="border-t border-border bg-card px-6 py-3 flex flex-col sm:flex-row items-center sm:items-center justify-between text-xs gap-1">
        <span className="uppercase tracking-widest mono text-muted-foreground">
          {loading ? "LOADING…" : `${presets.length} PRESET${presets.length !== 1 ? "S" : ""} · ${monitors.filter((m) => m.connected).length} MONITOR${monitors.filter((m) => m.connected).length !== 1 ? "S" : ""} CONNECTED · ${Object.keys(pins).length} PINNED`}
        </span>
        <span className="inline-flex items-center gap-3">
          {reapplyMsg && <span className="text-muted-foreground">{reapplyMsg}</span>}
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
            className="inline-flex items-center justify-center rounded-lg border border-border bg-card px-2 py-1 text-xs font-medium text-muted-foreground hover:bg-muted hover:text-foreground"
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