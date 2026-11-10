# Auto-Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ChromaDeck checks a hosted update manifest on startup and on demand, downloads a signed update, verifies its Ed25519 signature, and prompts the user to install and restart — all inside the app.

**Architecture:** Tauri v2 updater plugin (`tauri-plugin-updater`) registered in `tauri.conf.json`; a new Rust module `src-tauri/src/updater.rs` exposes `check_for_update_cmd` and `download_and_install_cmd`; the frontend calls typed wrappers in `src/lib/tauri.ts` and renders a check-for-updates button, update dialog, progress bar, and restart prompt.

**Tech Stack:** Rust (Tauri v2, `tauri-plugin-updater`, `semver`), React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-10-auto-updater-design.md` — the plan argues from that spec; executors read both.

**Status:** **IMPLEMENTED** — all tasks complete, v0.9.2 released with auto-updater.

---

## Completed Tasks

### Task 1: Updater plugin dependency + Tauri config ✅
- `tauri-plugin-updater = "2"` added to `Cargo.toml`
- `"updater"` plugin block added to `tauri.conf.json` with real endpoint and public key

### Task 2: Rust updater module — check-for-update command ✅
- `check_for_update_cmd` implemented in `src-tauri/src/updater.rs`
- Returns `{ available, version, notes, date }`

### Task 3: Rust updater module — download-and-install command + signature ✅
- `download_and_install_cmd` implemented — downloads, verifies Ed25519 signature, installs via Windows installer
- Emits `update://progress` events

### Task 4: Frontend bridge — typed invoke wrappers ✅
- `checkForUpdate()`, `downloadAndInstallUpdate()`, `onUpdateProgress()` in `src/lib/tauri.ts`
- `UpdateInfo` and `UpdateProgress` interfaces

### Task 5: Frontend UI — check-for-updates button, dialog, progress, restart ✅
- `UpdateCheckButton.tsx` — footer button with "Checking…" spinner
- `UpdateDialog.tsx` — dialog showing version/notes/progress; "Download & install" / "Restart now" / "Remind me later" / "OK"
- `autoUpdateState.ts` — pure state machine reducer (`idle` → `checking` → `available` → `downloading` → `ready-to-install` → `error`)
- Wired into `App.tsx`: startup check respects 24h suppression; manual check always shows dialog
- `handleRemindMeLater()` suppresses dialog for 24h via localStorage

### Task 6: Version bump + full gates ✅
- All three version files at `0.9.2`
- `cargo test`: 272 passed, 0 failed
- `npm run test`: 199 passed, 0 failed
- `npm run build`: green

---

## Endpoint

Manifest hosted at `https://chromadeck.github.io/update-manifest.json` (GitHub Pages). Tauri config points to it with the embedded Ed25519 public key.

## Release signing

Private key held off-tree in `chromadeck-web/scripts/.keys/secret_key.der`. Sign new releases with `chromadeck-web/scripts/sign-file.mjs <new-msi>` then update the manifest JSON with the new signature.
