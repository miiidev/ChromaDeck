# Auto-Updater — Design Spec

**Status:** implemented (v0.9.2); manifest hosted at `https://chromadeck.github.io/update-manifest.json`.
**Version target:** landed with v0.9.2.

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
  "version": "0.9.2",
  "notes": "ChromaDeck v0.9.2 — auto-updater...",
  "pub_date": "2026-11-11T00:00:00Z",
  "url": "https://github.com/miiidev/ChromaDeck/releases/download/v0.9.2/ChromaDeck_0.9.2_x64_en-US.msi",
  "signature": "RWQUd31f18or..."
}
```

Signed with your Ed25519 private key; public key embedded in the app. Tauri v2's updater plugin expects `url` and `signature` at the top level (Dynamic format), not wrapped in a `"data"` object.

## What's left to the builder

Nothing — the auto-updater is complete:

- Endpoint: `https://chromadeck.github.io/update-manifest.json` (GitHub Pages)
- Public key embedded in `src-tauri/tauri.conf.json` `"plugins.updater.pubkey"`: the **contents of the `.pub` file** produced by `tauri signer generate` (base64 of the full minisign public-key text, beginning `dW50cnVzdGVk…`).
- Release signing key (Tauri signer format, empty password) held off-tree in `chromadeck-web/scripts/.keys/chromadeck-updater.key` (gitignored).
- `bundle.createUpdaterArtifacts: true` makes `tauri build` write `<installer>.sig` next to each installer.

To ship a new release:

1. Bump all three version files (`package.json` via `npm version <v> --no-git-tag-version`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`).
2. Build with the key in the environment:
   `TAURI_SIGNING_PRIVATE_KEY="$(cat …/chromadeck-updater.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" npm run tauri build`
3. Set the manifest's `signature` to the **entire contents** of `ChromaDeck_<v>_x64_en-US.msi.sig` (already base64). Also update `version`, `url`, `notes` and `pub_date`.
4. Create a GitHub release with the MSI as an asset, then push the manifest.

### v0.9.2 signing defect (fixed in v0.9.3)

v0.9.2 embedded only the raw minisign key line as `pubkey`. The plugin base64-decodes `pubkey` and requires UTF-8 minisign text, so every verification failed with `SignatureUtf8` before any signature was checked. `chromadeck-web/scripts/sign-file.mjs` was also non-conformant: it emitted only the signature line, and its global signature covered the `trusted comment: ` prefix. v0.9.3 switches to a key generated by the Tauri signer. **v0.9.2 installs cannot auto-update and need one manual install of v0.9.3.** The old `secret_key.der` and `sign-file.mjs` are retired.

## Testing / gates

- Rust: manifest parsing, signature accept/reject tests, URL construction.
- Frontend: check-for-update flow, progress wiring, install prompt (vitest, node env).
- Gate before merge: `cargo test`, `npm run test`, `npm run build`.
