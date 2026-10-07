import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import IdentifyOverlay from "./components/IdentifyOverlay";

const label = getCurrentWindow().label;
if (label.startsWith("identify-")) {
  // ── Identify overlay window ──────────────────────────────────────────
  // Parse the trailing number; total is unknown at this point (the Rust
  // backend knows, but this window only knows its own number).
  const n = parseInt(label.replace("identify-", ""), 10);
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <IdentifyOverlay number={n} />
    </React.StrictMode>,
  );
} else {
  // ── Main application window ─────────────────────────────────────────
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}