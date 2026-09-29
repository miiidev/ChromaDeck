import "./App.css";

function App() {
  return (
    <main className="min-h-screen bg-neutral-950 text-neutral-100 flex flex-col">
      {/* Header */}
      <header className="border-b border-neutral-800 px-6 py-4">
        <h1 className="text-xl font-semibold tracking-tight">ChromaDeck</h1>
      </header>

      {/* Library view placeholder */}
      <section className="flex-1 p-6 space-y-6">
        <h2 className="text-lg font-medium text-neutral-300">Library</h2>

        {/* Monitors placeholder */}
        <div className="space-y-4">
          {["Monitor 1", "Monitor 2"].map((monitor, i) => (
            <div
              key={i}
              className="rounded-lg border border-neutral-800 bg-neutral-900/50 p-4"
            >
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-medium text-neutral-200">
                  {monitor}
                </h3>
                <span className="inline-flex items-center gap-1.5 text-xs text-emerald-400">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                  Connected
                </span>
              </div>
              <p className="text-xs text-neutral-500 italic">
                No presets yet. Click Edit to create one.
              </p>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}

export default App;
