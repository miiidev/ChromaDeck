# Auto-Updater — Design Spec

**Status:** approved in chat (2026-11-10); wired to a real endpoint supplied separately.
**Version target:** lands with v0.9.0 (version fields bumped at release).

## Intent and why

Users download a new installer for every ChromaDeck release. The app should check for updates itself, download them in the background, verify them, and offer to apply — keeping users on the latest version without leaving the app.

## Goals

- On startup (and on a user-initiated "Check for updates"), the app polls an update manifest and tells the user when a newer version is available.
- Download the update, verify its Ed25519 signature, and surface a clear prompt to install.
- After install, the app replaces itself and restarts on the new version.
- Never silently execute unsigned code; unsigned manifests are informational only (show "update available" but no install).

## Non-goals

- No beta/stable channels at launch — a single stable channel.
- No mandatory/forced updates (user always confirms install).
- No delta/patch updates — full installer replacement.
- No macOS/Linux-specific install logic beyond what the plugin provides; Windows (.msi/.exe) is the primary target since the app is Windows-native today.
- No update server to operate — you still need to host the manifest somewhere.

## Constraints

- Tauri v2, matching existing plugin style (opener, dialog, single-instance, autostart).
- Rust backend commands invoked from `src/lib/tauri.ts` typed wrappers.
- Version lives in three places and must stay in sync: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
- The update endpoint is a URL you control; the app ships with the Ed25519 public key embedded.
- No new system dependencies beyond the updater plugin crates.

## Approaches considered

1. **Tauri updater plugin (chosen).** `@tauri-apps/plugin-updater` + `tauri-plugin-updater`. Handles manifest polling, download, signature verification, and OS-specific installation. Least custom code; signed updates can install/replace the app automatically on Windows. Requires hosting a manifest and signing releases with an Ed25519 key.

2. **GitHub Releases API + manual download.** Poll GitHub for the latest release, download the asset, launch the installer, exit. More control but custom download progress, signature verification, and per-OS install logic.

3. **Self-hosted update server.** Full control over channels; most work to build and operate. Overkill for a single stable channel.

## Decided design (how)

### Backend (Rust)

- Add `tauri-plugin-updater` to `src-tauri/Cargo.toml` and register it in `tauri.conf.json` plugins; add `"updater"` config block with `active`, endpoints, and embedded Ed25519 public key.
- New module `src-tauri/src/updater.rs`.
- Command `check_for_update_cmd` → returns `{ available: boolean, version: string, notes: string | null, date: string | null }`.
- Command `download_and_install_cmd` → downloads the update with progress, verifies the Ed25519 signature, then triggers OS installation. Progress emitted to frontend via `update://progress { downloaded, total, percent }` events.
- The plugin is initialized in `setup` with the public key and update endpoint URL.
- After install succeeds, the plugin spawns the new installer and the app exits; on Windows the installer replaces the running binary.

### Frontend (React + TS)

- New typed wrappers in `src/lib/tauri.ts`: `checkForUpdate()`, `downloadAndInstallUpdate()`.
- New UI surface: a "Check for updates" button (settings footer or app menu) and an update banner/dialog showing version + release notes + progress bar while downloading.
- On "update available": non-blocking banner; clicking opens a dialog with "Download & install".
- While downloading: progress bar + "Installing…" state; after success: prompt to restart now or later.

### Update manifest

Hosted JSON at your update endpoint:

```json
{
  "version": "0.9.0",
  "date": "2026-11-10",
  "notes": "...",
  "url": "https://updates.chromadeck.dev/ChromaDeck_0.9.0.msi",
  "sha256": "...",
  "signature": "..."
}
```

Signed with your Ed25519 private key; public key embedded in the app.

## What's left to the builder

- Pick and provision the update endpoint URL (where the manifest lives).
- Generate the Ed25519 key pair; embed the public key; keep the private key for signing release manifests.
- Decide where the "Check for updates" UI lives (settings footer vs. app menu vs. tray).
- Whether v0.9.0 ships with the updater wired but pointing at a placeholder endpoint (dry-run, no real updates yet).

## Testing / gates

- Rust: manifest parsing, signature accept/reject tests, URL construction.
- Frontend: check-for-update flow, progress wiring, install prompt (vitest, node env).
- Gate before merge: `cargo test`, `npm run test`, `npm run build`.
