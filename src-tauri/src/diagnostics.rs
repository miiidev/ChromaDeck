// ── Rolling monitor diagnostics snapshot log ─────────────────────────────
// Appends a machine-parsable snapshot of the monitor enumeration to
// `diagnostics.log` in the app data dir, only when the enumerated set
// changes (identity set = edid_id + connected flags, order-insensitive).
// Pure helpers are unit-tested below; the IO wrapper is thin.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::monitor::Monitor;

/// One `connected=…` line per monitor, shared with the Identify overlay
/// (Task 4) so both callers produce identical text.
pub fn per_monitor_line(m: &Monitor) -> String {
    let mut line = format!(
        "edid_id={} connected={} model={} adapter={} instance={} EDID={}",
        m.edid_id,
        if m.connected { 1 } else { 0 },
        m.model,
        m.device_name,
        m.instance_path,
        if m.edid_present {
            m.edid_len.to_string()
        } else {
            "none".to_string()
        }
    );
    if m.hdr_active {
        line.push_str(" HDR=1");
    }
    if m.acm_active {
        line.push_str(" ACM=1");
    }
    line
}

/// Full snapshot text: one header line (carries the fingerprint), then one
/// line per monitor.
pub fn format_snapshot(monitors: &[Monitor]) -> String {
    let current = snapshot_fingerprint(monitors);
    let mut out = format!(
        "--- diagnostics snapshot {} fingerprint={} ---\n",
        timestamp_now_utc(),
        current
    );
    for m in monitors {
        out.push_str(&per_monitor_line(m));
        out.push('\n');
    }
    out
}

/// FNV-1a 64-bit over the sorted `edid_id + connected` identity set
/// (each token's UTF-8 bytes followed by a NUL separator): order holds no
/// information, so reordering is free; membership or a connected flip
/// changes the hash.
pub fn snapshot_fingerprint(monitors: &[Monitor]) -> u64 {
    let mut tokens: Vec<String> = monitors
        .iter()
        .map(|m| format!("{}{}", m.edid_id, m.connected as u8))
        .collect();
    tokens.sort();
    let mut hash: u64 = 0xcbf29ce484222325;
    for t in tokens {
        for b in t.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0u64; // NUL separator bounds the token
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Pure gate: append only when the fingerprint changed.
pub fn should_log(last: u64, current: u64) -> bool {
    last != current
}

/// The fingerprint from the LAST `--- diagnostics snapshot` header in
/// `diagnostics.log`, if any (absent file or no header → `None`). Tail-only:
/// headers are small and accumulate at the end, so reading the final 64 KiB
/// is enough to resume (and avoids a full ~1 MiB read on every 5 s poll).
fn read_last_fingerprint(path: &Path) -> io::Result<Option<u64>> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    const TAIL: u64 = 64 * 1024;
    let len = std::fs::metadata(path)?.len();
    let start = len.saturating_sub(TAIL);
    let mut tail = Vec::with_capacity((len - start) as usize);
    f.seek(io::SeekFrom::Start(start))?;
    f.take(len - start).read_to_end(&mut tail)?;
    Ok(last_fingerprint_in(&String::from_utf8_lossy(&tail)))
}

/// Parse the newest snapshot header's fingerprint from text. Pure so it is
/// unit-tested independently of the IO wrapper.
fn last_fingerprint_in(text: &str) -> Option<u64> {
    let marker = "--- diagnostics snapshot ";
    text.rmatch_indices(marker)
        .next()
        .and_then(|(i, _)| {
            text[i..]
                .split_whitespace()
                .find(|w| w.starts_with("fingerprint="))
        })
        .and_then(|w| w[12..].parse::<u64>().ok())
}

/// Append the snapshot only when its identity set changed since the last
/// record. `Ok(true)` = appended. Self-trims to its final ~256 KiB once
/// the file exceeds 1 MiB.
pub fn append_snapshot_if_changed(dir: &Path, monitors: &[Monitor]) -> io::Result<bool> {
    let current = snapshot_fingerprint(monitors);
    let path = dir.join("diagnostics.log");
    match read_last_fingerprint(&path) {
        Ok(Some(last)) if !should_log(last, current) => return Ok(false),
        Ok(Some(last)) => debug_assert!(should_log(last, current)),
        Ok(None) => {} // absent file: always write
        Err(e) => return Err(e),
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(format_snapshot(monitors).as_bytes())?;
    trim_if_huge(&path)?;
    Ok(true)
}

/// Keep the file useful forever a session runs: over 1 MiB → rewrite with
/// only the final 256 KiB (a whole-truncate would lose the most recent
/// history we care about; keeping the tail keeps the newest snapshots).
fn trim_if_huge(path: &Path) -> io::Result<()> {
    const MAX: u64 = 1024 * 1024;
    const KEEP: u64 = 256 * 1024;
    let len = path.metadata()?.len();
    if len <= MAX {
        return Ok(());
    }
    let mut all = Vec::new();
    File::open(path)?.read_to_end(&mut all)?;
    let tail = all.split_off(all.len() - KEEP as usize);
    // Align to the next snapshot header (or newline) so the remaining bytes
    // start on a clean line.
    let skip = match tail.windows(2).position(|w| w == b"\n-") {
        Some(i) => i + 1,
        None => 0,
    };
    let start = &tail[skip..];
    let mut f = File::create(path)?;
    f.write_all(start)?;
    Ok(())
}

/// UTC RFC3339 timestamp for the current instant.
pub fn timestamp_now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    timestamp_from_epoch(secs)
}

/// UTC RFC3339 from a Unix-epoch second count, without a datetime
/// dependency. Days-to-civil algorithm (Howard Hinnant): z → (y, m, d),
/// then H:M:S. Pure so known epoch↔timestamp pairs pin the arithmetic.
fn timestamp_from_epoch(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let (h, m, s) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mo <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, mo, d, h, m, s)
}

// ── One-click diagnostics report export ─────────────────────────────────────

/// Returned by `export_diagnostics_cmd`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticsExportSummary {
    pub dest_path: String,
    pub bytes_written: usize,
    pub monitors_count: usize,
}

/// Redact Windows/Unix user profile directory paths (`C:\Users\<name>` or
/// `C:/Users/<name>`) and explicit home directory prefixes to `<USER>` so
/// shared bug reports do not leak the local OS username.
pub fn redact_user_paths(input: &str, home_dir: Option<&Path>, username: Option<&str>) -> String {
    let mut out = input.to_string();

    if let Some(home) = home_dir {
        let home_str = home.to_string_lossy();
        if !home_str.is_empty() {
            let win_form = home_str.replace('/', "\\");
            let unix_form = home_str.replace('\\', "/");
            if let Some(parent) = home.parent() {
                let p_win = parent.to_string_lossy().replace('/', "\\");
                let p_unix = parent.to_string_lossy().replace('\\', "/");
                out = replace_case_insensitive(&out, &win_form, &format!("{p_win}\\<USER>"));
                out = replace_case_insensitive(&out, &unix_form, &format!("{p_unix}/<USER>"));
            } else {
                out = replace_case_insensitive(&out, &win_form, "<USER>");
                out = replace_case_insensitive(&out, &unix_form, "<USER>");
            }
        }
    }

    if let Some(user) = username.map(str::trim).filter(|u| !u.is_empty()) {
        out = replace_case_insensitive(&out, &format!("\\Users\\{user}"), "\\Users\\<USER>");
        out = replace_case_insensitive(&out, &format!("/Users/{user}"), "/Users/<USER>");
    }

    out
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let hay_lower = haystack.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut result = String::with_capacity(haystack.len());
    let mut last = 0usize;
    let mut search_from = 0usize;
    while let Some(rel) = hay_lower[search_from..].find(&needle_lower) {
        let idx = search_from + rel;
        result.push_str(&haystack[last..idx]);
        result.push_str(replacement);
        last = idx + needle.len();
        search_from = last;
    }
    result.push_str(&haystack[last..]);
    result
}

/// Read the last `max_bytes` of a UTF-8 log file (aligned to the next newline
/// when truncated). Returns `"(not present)"` when the file does not exist.
fn read_log_tail(path: &Path, max_bytes: u64) -> String {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return "(not present)".into(),
        Err(e) => return format!("(read error: {e})"),
    };
    let len = match file.metadata() {
        Ok(m) => m.len(),
        Err(e) => return format!("(metadata error: {e})"),
    };
    let start = len.saturating_sub(max_bytes);
    if let Err(e) = file.seek(io::SeekFrom::Start(start)) {
        return format!("(seek error: {e})");
    }
    let mut buf = Vec::with_capacity((len - start) as usize);
    if let Err(e) = file.take(len - start).read_to_end(&mut buf) {
        return format!("(read error: {e})");
    }
    let slice = if start > 0 {
        match buf.iter().position(|&b| b == b'\n') {
            Some(pos) => &buf[pos + 1..],
            None => &buf[..],
        }
    } else {
        &buf[..]
    };
    let text = String::from_utf8_lossy(slice).trim_end().to_string();
    if text.is_empty() {
        "(empty)".into()
    } else {
        text
    }
}

/// Best-effort Windows OS product/build string from registry (zero FFI).
pub fn windows_os_version() -> String {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(cv) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
            let product: String = cv.get_value("ProductName").unwrap_or_default();
            let display: String = cv.get_value("DisplayVersion").unwrap_or_default();
            let build: String = cv.get_value("CurrentBuildNumber").unwrap_or_default();
            let ubr: u32 = cv.get_value("UBR").unwrap_or(0);
            let build_num: u32 = build.parse().unwrap_or(0);
            let product_clean = if build_num >= 22000 && product.contains("Windows 10") {
                product.replace("Windows 10", "Windows 11")
            } else {
                product
            };
            let mut parts = Vec::new();
            if !product_clean.is_empty() {
                parts.push(product_clean);
            }
            if !display.is_empty() {
                parts.push(display);
            }
            if !build.is_empty() {
                parts.push(format!("Build {build}.{ubr}"));
            }
            if !parts.is_empty() {
                return parts.join(" ");
            }
        }
        "Windows (unknown build)".into()
    }
    #[cfg(not(windows))]
    {
        std::env::consts::OS.into()
    }
}

/// Snapshot payload for pure diagnostics report formatting.
pub struct DiagnosticsReportInput<'a> {
    pub app_version: &'a str,
    pub generated_at_utc: &'a str,
    pub os_version: &'a str,
    pub data_dir: &'a str,
    pub monitors: &'a [Monitor],
    pub probes: &'a [(String, Result<crate::color::MonitorProbe, String>)],
    pub capabilities: &'a [(String, crate::capabilities::DisplayCapabilities)],
    pub presets_count: usize,
    pub pins_count: usize,
    pub applied_count: usize,
    pub schedules_count: usize,
    pub auto_apply_enabled: bool,
    pub diagnostics_log_tail: &'a str,
    pub auto_apply_log_tail: &'a str,
}

/// Format a structured, human- and support-readable diagnostics report.
pub fn format_diagnostics_report(input: &DiagnosticsReportInput<'_>) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("============================================================\n");
    out.push_str("ChromaDeck Diagnostics Report\n");
    out.push_str("============================================================\n");
    out.push_str(&format!("Generated (UTC): {}\n", input.generated_at_utc));
    out.push_str(&format!("ChromaDeck:      v{}\n", input.app_version));
    out.push_str(&format!("OS:              {}\n", input.os_version));
    out.push_str(&format!("Data directory:  {}\n", input.data_dir));
    out.push_str(&format!(
        "Store summary:   presets={} pins={} applied={} schedules={} auto_apply={}\n",
        input.presets_count,
        input.pins_count,
        input.applied_count,
        input.schedules_count,
        input.auto_apply_enabled
    ));

    out.push_str("\n── Monitors ────────────────────────────────────────────────\n");
    if input.monitors.is_empty() {
        out.push_str("(no monitors enumerated)\n");
    } else {
        for m in input.monitors {
            out.push_str(&per_monitor_line(m));
            out.push('\n');
        }
    }

    out.push_str("\n── Per-Monitor Live Probes ─────────────────────────────────\n");
    if input.probes.is_empty() {
        out.push_str("(no connected monitors probed)\n");
    } else {
        for (edid, probe) in input.probes {
            match probe {
                Ok(p) => {
                    let json = serde_json::to_string_pretty(p).unwrap_or_else(|_| "{}".into());
                    out.push_str(&format!("[{edid}]\n{json}\n\n"));
                }
                Err(err) => {
                    out.push_str(&format!("[{edid}]\nerror: {err}\n\n"));
                }
            }
        }
    }

    out.push_str("── Per-Monitor Capabilities ────────────────────────────────\n");
    if input.capabilities.is_empty() {
        out.push_str("(no connected monitors)\n");
    } else {
        for (edid, caps) in input.capabilities {
            let json = serde_json::to_string_pretty(caps).unwrap_or_else(|_| "{}".into());
            out.push_str(&format!("[{edid}]\n{json}\n\n"));
        }
    }

    out.push_str("── diagnostics.log (tail) ──────────────────────────────────\n");
    out.push_str(input.diagnostics_log_tail);
    out.push('\n');

    out.push_str("\n── auto-apply.log (tail) ───────────────────────────────────\n");
    out.push_str(input.auto_apply_log_tail);
    out.push('\n');

    out
}

#[tauri::command]
pub fn export_diagnostics_cmd(
    state: tauri::State<'_, crate::store::AppStore>,
    dest_path: String,
) -> Result<DiagnosticsExportSummary, String> {
    let (data_dir, presets_count, pins_count, applied_count, schedules_count, auto_apply_enabled) = {
        let store = state.0.lock().map_err(|e| e.to_string())?;
        (
            store.data_dir().to_path_buf(),
            store.list_presets().len(),
            store.list_pins().len(),
            store.list_applied().len(),
            store.list_schedules().len(),
            store.auto_apply_enabled(),
        )
    };

    let monitors = crate::monitor::list_monitors();
    let mut probes = Vec::new();
    let mut capabilities = Vec::new();
    for m in monitors.iter().filter(|m| m.connected) {
        probes.push((
            m.edid_id.clone(),
            crate::color::probe_monitor_cmd(m.edid_id.clone()),
        ));
        capabilities.push((
            m.edid_id.clone(),
            crate::capabilities::get_display_capabilities(m.edid_id.clone()),
        ));
    }

    let diag_tail = read_log_tail(&data_dir.join("diagnostics.log"), 16 * 1024);
    let auto_tail = read_log_tail(&data_dir.join("auto-apply.log"), 16 * 1024);
    let now_utc = timestamp_now_utc();
    let os_ver = windows_os_version();
    let data_dir_str = data_dir.to_string_lossy().to_string();

    let raw_report = format_diagnostics_report(&DiagnosticsReportInput {
        app_version: env!("CARGO_PKG_VERSION"),
        generated_at_utc: &now_utc,
        os_version: &os_ver,
        data_dir: &data_dir_str,
        monitors: &monitors,
        probes: &probes,
        capabilities: &capabilities,
        presets_count,
        pins_count,
        applied_count,
        schedules_count,
        auto_apply_enabled,
        diagnostics_log_tail: &diag_tail,
        auto_apply_log_tail: &auto_tail,
    });

    let home = dirs::home_dir();
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok();
    let redacted = redact_user_paths(&raw_report, home.as_deref(), username.as_deref());

    let dest = Path::new(&dest_path);
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(dest, redacted.as_bytes()).map_err(|e| e.to_string())?;

    Ok(DiagnosticsExportSummary {
        dest_path,
        bytes_written: redacted.len(),
        monitors_count: monitors.len(),
    })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, connected: bool) -> Monitor {
        Monitor {
            edid_id: id.into(),
            model: String::new(),
            serial: String::new(),
            connected,
            device_name: String::new(),
            alias: String::new(),
            instance_path: String::new(),
            edid_present: false,
            edid_len: 0,
            hdr_active: false,
            acm_active: false,
        }
    }

    #[test]
    fn fingerprint_order_insensitive_change_sensitive() {
        let a = vec![m("A", true), m("B", false)];
        let b = vec![m("B", false), m("A", true)];
        assert_eq!(snapshot_fingerprint(&a), snapshot_fingerprint(&b));
        assert_ne!(
            snapshot_fingerprint(&a),
            snapshot_fingerprint(&[m("B", true), m("A", true)])
        );
    }

    #[test]
    fn format_snapshot_renders_each_monitor_line() {
        let monitor = Monitor {
            edid_id: "A|MON/INFO".into(),
            model: "Model X".into(),
            serial: "S1".into(),
            connected: true,
            device_name: r"\\.\DISPLAY1".into(),
            alias: "Office".into(),
            instance_path: r"MON\INFO\1".into(),
            edid_present: true,
            edid_len: 256,
            hdr_active: false,
            acm_active: false,
        };
        let snapshot = format_snapshot(std::slice::from_ref(&monitor));
        assert!(snapshot.lines().count() >= 2); // header + 1 monitor
        assert!(
            snapshot.contains(r"\\.\DISPLAY1")
                && snapshot.contains("EDID=256")
                && snapshot.contains("connected=1")
        );
        // Line 2 of the snapshot is exactly per_monitor_line for that monitor.
        assert_eq!(
            per_monitor_line(&monitor),
            snapshot.lines().nth(1).unwrap().to_string()
        );
        // Header carries the fingerprint for the IO wrapper's resume.
        assert!(snapshot.lines().next().unwrap().contains("fingerprint="));
    }

    #[test]
    fn append_gate_writes_only_on_change() {
        assert!(should_log(7, 9));
        assert!(!should_log(9, 9));
    }

    #[test]
    fn timestamp_from_epoch_known_values() {
        // Instantly catches off-by-one and leap-day regressions in the
        // days-to-civil algorithm (which CI would otherwise never prove).
        assert_eq!(timestamp_from_epoch(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp_from_epoch(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(timestamp_from_epoch(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(timestamp_from_epoch(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(timestamp_from_epoch(1_762_560_000), "2025-11-08T00:00:00Z");
    }

    #[test]
    fn last_fingerprint_in_scans_headers_from_the_tail() {
        let text = "--- diagnostics snapshot 2026-01-01T00:00:00Z fingerprint=111 ---\n\
                    edid_id=A connected=1\n\
                    --- diagnostics snapshot 2026-01-02T00:00:00Z fingerprint=222 ---\n\
                    edid_id=A connected=0\n";
        assert_eq!(last_fingerprint_in(text), Some(222));
        assert_eq!(last_fingerprint_in("no headers here\n"), None);
    }

    #[test]
    fn append_gate_io_writes_once_and_resumes() -> io::Result<()> {
        let temp = TempDir::default();
        let monitors = vec![m("A", true)];
        let dir = temp.path();
        // First run: log does not exist → append.
        assert!(append_snapshot_if_changed(dir, &monitors)?);
        let path = dir.join("diagnostics.log");
        let len1 = path.metadata()?.len();
        assert!(len1 > 0);
        // Second run: identical identity set must not grow the log
        // (re-enumeration churn stays at one snapshot per change).
        assert!(!append_snapshot_if_changed(dir, &monitors)?);
        assert_eq!(path.metadata()?.len(), len1);
        // A change appends again and the resume read (tail-parse) still
        // sees the newest fingerprint.
        let changed = vec![m("A", false)];
        assert!(append_snapshot_if_changed(dir, &changed)?);
        assert!(path.metadata()?.len() > len1);
        Ok(())
    }

    #[test]
    fn redact_user_paths_strips_windows_and_unix_usernames() {
        let home = Path::new(r"C:\Users\Alice Example");
        let input = r"Data: C:\Users\Alice Example\AppData\Local\ChromaDeck and c:/users/alice example/docs";
        let redacted = redact_user_paths(input, Some(home), Some("Alice Example"));
        assert!(!redacted.to_lowercase().contains("alice example"));
        assert!(redacted.contains(r"C:\Users\<USER>\AppData\Local\ChromaDeck"));
        assert!(redacted.contains("C:/Users/<USER>/docs") || redacted.contains("c:/users/<USER>/docs"));
    }

    #[test]
    fn format_diagnostics_report_includes_all_sections() {
        let mon = m("EDID:TEST", true);
        let report = format_diagnostics_report(&DiagnosticsReportInput {
            app_version: "0.9.8",
            generated_at_utc: "2026-11-26T00:00:00Z",
            os_version: "Windows 11 24H2 Build 26100.1",
            data_dir: r"C:\Users\<USER>\AppData\Local\ChromaDeck",
            monitors: std::slice::from_ref(&mon),
            probes: &[],
            capabilities: &[],
            presets_count: 3,
            pins_count: 1,
            applied_count: 1,
            schedules_count: 2,
            auto_apply_enabled: true,
            diagnostics_log_tail: "diag-line",
            auto_apply_log_tail: "auto-line",
        });
        assert!(report.contains("ChromaDeck:      v0.9.8"));
        assert!(report.contains("presets=3 pins=1 applied=1 schedules=2 auto_apply=true"));
        assert!(report.contains("edid_id=EDID:TEST connected=1"));
        assert!(report.contains("diag-line"));
        assert!(report.contains("auto-line"));
    }

    /// Minimal RAII temp dir for IO tests (no external test deps).
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    impl Default for TempDir {
        fn default() -> Self {
            let base = std::env::temp_dir().join("chromadeck-diagnostics-test");
            let dir = base.join(format!("run-{}", std::process::id()));
            // A stale run dir must not break isolation.
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("temp dir create");
            TempDir(dir)
        }
    }
}
