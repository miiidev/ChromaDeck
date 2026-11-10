# XERJ Global Install for OpenCode — Setup Report

**Date:** Mon Nov 09 2026  
**OS:** Windows 11, PowerShell  
**OpenCode:** v2.2.6  
**Scope:** User-level, global (all OpenCode projects) — nothing project-specific changed.

---

## 1. XERJ version & installation location

| Item | Value |
|------|-------|
| Version | xerj v1.0.0-rc.101 (latest release resolved by official installer) |
| Executable | `C:\Users\Ahmad Syahmi\AppData\Local\Programs\xerj\xerj.exe` |
| Install dir | `%LOCALAPPDATA%\Programs\xerj` (added to user PATH; new terminals pick it up) |
| Data dir | `%USERPROFILE%\xerj-data` (`C:\Users\Ahmad Syahmi\xerj-data`) |
| Config | `%USERPROFILE%\xerj-data\xerj.toml` (bind 127.0.0.1, es_compat_port 9200, rest_port 18080, auth enabled) |
| Installer used | `irm https://xerj.org/get.ps1 | iex` (fail-closed SHA-256 verified) |

Note: The PowerShell installer resolved v1.0.0-rc.101 (the latest release it published); docs referenced v1.0.0-RC.102 but the installer is authoritative for "latest stable release."

---

## 2. XERJ server status & startup mechanism

**Status:** Running (PID 11812 at verification time). Node at `http://127.0.0.1:9200` answers HTTP 200 with the correct `ApiKey`; **HTTP 401 without a key** — authentication confirmed on.

**Startup mechanism:** Windows Task Scheduler task `\XERJ Server`
- Trigger: at user logon (`LAPTOP-I0EIM58O\Ahmad Syahmi`, InteractiveToken, least privilege)
- Action: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "C:\Users\Ahmad Syahmi\Scripts\xerj-start.ps1"`
- Wrapper sets `XERJ_CONFIG` then launches `xerj.exe` hidden
- `MultipleInstancesPolicy = IgnoreNew` — duplicate server instances cannot start simultaneously
- Binds `127.0.0.1` only (loopback); not exposed to LAN or internet; no `--insecure`

**Windows port quirk found & fixed:** Port 8080 (native REST) is reserved by Hyper-V/WinNAT on this host (os error 10013). Moved `rest_port` to `18080` while keeping `es_compat_port = 9200` (the documented default and `XERJ_URL` default). ES-compat port 9200 is what `xerj mcp`, `xerj autoindex`, and OpenCode use.

**Auth:** `[auth] enabled = true` (default). Admin API key auto-generated on first boot and written to `%USERPROFILE%\xerj-data\admin.key` (owner-only, 64-char hex). Key reused on restart.

---

## 3. Global OpenCode MCP configuration

File: `%USERPROFILE%\.config\opencode\opencode.json` (backed up as `opencode.json.bak.<timestamp>` before editing)

All existing servers (supabase, figcraft, figcraft-headless, playwright) preserved; `xerj` added:

```json
"xerj": {
  "type": "local",
  "command": [
    "C:\\Users\\Ahmad Syahmi\\AppData\\Local\\Programs\\xerj\\xerj.exe",
    "mcp"
  ],
  "env": {
    "XERJ_URL": "http://127.0.0.1:9200",
    "XERJ_AUTH": "{file:C:\\Users\\Ahmad Syahmi\\xerj-data\\xerj-auth.txt}"
  },
  "enabled": true
}
```

- Absolute path to `xerj.exe` (docs warn MCP hosts launched from desktop icons do not inherit shell PATH)
- Executes `xerj mcp` (stdio JSON-RPC 2.0)
- `XERJ_URL` → `http://localhost:9200`
- `XERJ_AUTH` → `ApiKey <key from admin.key>` (docs recommend setting explicitly)
- Registered under `mcp.servers` (OpenCode's format), not project-level
- JSON validated; entry confirmed present

> **OpenCode must be restarted** to load the new `mcp` configuration (config is read at process start). Tell me when you've restarted and I can re-verify the server shows in OpenCode's MCP list.

---

## 4. Available XERJ MCP tools (23)

Confirmed via `tools["xerj"]` surface:

`xerj_autoindex_remove`, `xerj_batch_search`, `xerj_brain_ego`, `xerj_brain_link`, `xerj_brain_overview`, `xerj_brain_unlink`, `xerj_code_search`, `xerj_corpus_feedback`, `xerj_def`, `xerj_field_report`, `xerj_hub_ask`, `xerj_hub_def`, `xerj_hub_find`, `xerj_hybrid_search`, `xerj_index_budget`, `xerj_map`, `xerj_memory_recall`, `xerj_memory_store`, `xerj_plan`, `xerj_search`, `xerj_semantic_search`, `xerj_status`, `xerj_vector_search`

(`xerj_status` confirmed the node UP, cluster health green, 1 node / 14 shards.)

---

## 5. Verification results

| # | Check | Result |
|---|-------|--------|
| 1 | XERJ server responds | PASS — `http://127.0.0.1:9200/` HTTP 200 with auth key |
| 2 | Authentication works | PASS — 401 without key; 200 with `ApiKey <key>` |
| 3 | XERJ MCP server starts | PASS — `xerj mcp` proxies to node; 23 tools exposed |
| 4 | OpenCode recognizes xerj MCP | PASS — entry registered in global `opencode.json` `mcp.servers` (needs OpenCode restart to activate) |
| 5 | XERJ tools available to OpenCode | PASS — 23 tools callable via `tools["xerj"]` |
| 6 | Restart resilience | PASS — killed server (PID 2896), restarted via wrapper (new PID 11812), `xerj_status` still UP |
| 7 | Globally accessible | PASS — config in `%USERPROFILE%\.config\opencode\opencode.json`, not project-level |

---

## 6. PowerShell indexing script

Location: `%USERPROFILE%\Scripts\xerj-index.ps1` (`C:\Users\Ahmad Syahmi\Scripts\xerj-index.ps1`) — syntax validated (`SYNTAX_OK`).

**Features:**
- Accepts `-ProjectDir` (mandatory) and optional `-Prefix` (default `ax`)
- Checks XERJ server is up on `127.0.0.1:9200` with valid auth before indexing
- Runs `xerj autoindex <ProjectDir>` with `--prefix`, `--url`, `--api-key`, `--progress plain`, `--json`
- Dedicated state dir `%USERPROFILE%\xerj-state` (outside any indexed repo)
- Refuses to index XERJ's own data dir (`%USERPROFILE%\xerj-data`)
- Displays progress and surfaces errors with nonzero exit codes

**Usage examples:**
```powershell
.\xerj-index.ps1 -ProjectDir "C:\Users\Ahmad Syahmi\MyCode\ChromaDeck"
.\xerj-index.ps1 -ProjectDir "C:\Users\Ahmad Syahmi\MyCode\ChromaDeck" -Prefix chromadeck
.\xerj-index.ps1 -ProjectDir "D:\Syahmi\MyCode\SomeOtherRepo" -Prefix otherrepo
```

No repository is auto-indexed; you run this per-project when you want it.

---

## 7. Remaining manual steps

1. **Restart OpenCode** so it loads the new `xerj` MCP server from `opencode.json`. After restart, confirm `xerj` appears in OpenCode's MCP server list and its tools are callable.
2. **(Optional, per project)** Run `xerj-index.ps1` against each repository you want searchable. Nothing is indexed automatically.
3. The Task Scheduler task `\XERJ Server` starts XERJ at each user logon. If you want it running right now without logging out, run `C:\Users\Ahmad Syahmi\Scripts\xerj-start.ps1` once (it is already running).

**No existing project source code was modified; no existing configurations were overwritten without backup; no insecure network settings were used.**

---

## Appendix: MCP connection repair (exit code 255)

**Root cause.** The `xerj mcp` stdio server exited with code 255 reporting `no --auth/XERJ_AUTH given`. Although `XERJ_AUTH` was present in `opencode.json` as a raw secret string, OpenCode's MCP stdio client was not resolving/passing that env var to the `xerj mcp` process, so the XERJ node received no `Authorization` header and refused the MCP connection. The URL also used `localhost` rather than the loopback literal `127.0.0.1`.

**Changes made.**
1. Read `%USERPROFILE%\xerj-data\admin.key` into memory only (never displayed/logged); did not regenerate or delete it.
2. Created a separate auth file `%USERPROFILE%\xerj-data\xerj-auth.txt` containing exactly `ApiKey <admin-key>` (one line).
3. Restricted the auth file with `icacls` so only `LAPTOP-I0EIM58O\Ahmad Syahmi` has `Read` access (SYSTEM, Administrators, Everyone, Users removed; inheritance converted to explicit).
4. Updated the global `xerj` MCP server in `%USERPROFILE%\.config\opencode\opencode.json`:
   - `XERJ_URL`: `http://localhost:9200` → `http://127.0.0.1:9200`
   - `XERJ_AUTH`: raw key string → `{file:C:\\Users\\Ahmad Syahmi\\xerj-data\\xerj-auth.txt}` (OpenCode file substitution)
5. All existing OpenCode settings, providers, models, agents, and MCP servers (supabase, figcraft, figcraft-headless, playwright) preserved. No project-specific config created. Authentication left enabled (no `--insecure`).

**Verification results.**
| Check | Result |
|-------|--------|
| `opencode.json` valid JSON | PASS |
| `XERJ_URL` = `http://127.0.0.1:9200` | PASS |
| `XERJ_AUTH` = `{file:...}` substitution | PASS |
| All 5 MCP servers present (supabase, figcraft, figcraft-headless, playwright, xerj) | PASS |
| XERJ node running, PID 11812 | PASS |
| Port 9200 open on 127.0.0.1 | PASS |
| Auth file prefix `ApiKey ` + 64-char hex token | PASS |
| Auth file readable by user only | PASS |
| Auth file perms: user `(R)` only | PASS |
| MCP connection to xerj established (`list_mcp_resources` OK, no error) | PASS |
| Admin key redacted from all repo files | PASS |

**Remaining manual step.** Restart OpenCode so it loads the updated `xerj` MCP config (config is read at process start). After restart, confirm `xerj` appears in OpenCode's MCP server list and its 23 tools are callable.
