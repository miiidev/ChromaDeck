# XERJ Global Install Plan (OpenCode)

Status: COMPLETE.

## Decisions (one sensible path; official docs source of truth: xerj.org)
- Installer: `irm https://xerj.org/get.ps1 | iex` (PowerShell, bypass execution policy)
- Install dir: `%LOCALAPPDATA%\Programs\xerj` (installer default), on user PATH
- Data dir: `%USERPROFILE%\xerj-data`
- Config: `%USERPROFILE%\xerj-data\xerj.toml`, passed via `XERJ_CONFIG` env var
- Bind: `127.0.0.1` only; ES-compat port `9200` (default); auth enabled (default, NO `--insecure`)
- Auth: `XERJ_AUTH = "ApiKey <key>"` set explicitly in OpenCode env
- Persistence: Windows Task Scheduler task "XERJ Server", trigger at user logon, MultipleInstancesPolicy=IgnoreNew (no duplicates)
- OpenCode: added `xerj` to global `mcp` in `%USERPROFILE%\.config\opencode\opencode.json` (all existing servers preserved)
- Index script: `%USERPROFILE%\Scripts\xerj-index.ps1` (no auto-indexing)

## Notes
- Windows reserved port 8080 (Hyper-V/WinNAT) blocked the native REST listener; moved `rest_port` to 18080 while keeping `es_compat_port = 9200` (XERJ_URL default). Server works on 9200.
- v1.0.0-rc.101 installed (installer resolved latest release; docs referenced RC.102).
