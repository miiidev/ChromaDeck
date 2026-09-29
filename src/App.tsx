import { useState, useEffect, useCallback } from "react";
import "./App.css";
import { listMonitors, listPresets } from "./lib/tauri";
import type { Monitor, Preset } from "./lib/types";
import MonitorList from "./components/MonitorList";
import PresetEditor from "./components/PresetEditor";

function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingPreset, setEditingPreset] = useState<Preset | null>(null);
  const [showEditor, setShowEditor] = useState(false);

  const fetchData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [monitorsData, presetsData] = await Promise.all([
        listMonitors(),
        listPresets(),
      ]);
      setMonitors(monitorsData);
      setPresets(presetsData);
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

  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      {/* Header */}
      <header className="border-b border-neutral-800 px-6 py-4">
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-indigo-500 to-violet-600 flex items-center justify-center">
            <svg className="w-4 h-4 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25" />
            </svg>
          </div>
          <h1 className="text-xl font-semibold tracking-tight">ChromaDeck</h1>
          <span className="text-xs text-neutral-600 font-mono">v0.1.0</span>
        </div>
      </header>

      {/* Error banner */}
      {error && (
        <div className="mx-6 mt-4 px-4 py-3 rounded-lg bg-red-950/40 border border-red-900/50 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg className="w-4 h-4 text-red-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
            </svg>
            <span className="text-sm text-red-300">Failed to load: {error}</span>
          </div>
          <button
            onClick={fetchData}
            className="text-xs text-red-400 hover:text-red-300 underline"
          >
            Retry
          </button>
        </div>
      )}

      {/* Library view */}
      <MonitorList
        monitors={monitors}
        presets={presets}
        loading={loading}
        onEdit={handleEdit}
        onRefresh={fetchData}
        onCreateNew={handleCreateNew}
      />

      {/* Status bar */}
      <footer className="border-t border-neutral-800 px-6 py-2 flex items-center justify-between text-xs text-neutral-600">
        <span>
          {loading ? "Loading…" : `${presets.length} preset${presets.length !== 1 ? "s" : ""} · ${monitors.filter((m) => m.connected).length} monitor${monitors.filter((m) => m.connected).length !== 1 ? "s" : ""} connected`}
        </span>
        {!loading && (
          <span>
            Last refresh: {new Date().toLocaleTimeString()}
          </span>
        )}
      </footer>

      {/* Editor modal */}
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