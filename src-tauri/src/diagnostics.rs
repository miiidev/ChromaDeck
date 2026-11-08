// ── Rolling monitor diagnostics snapshot log ─────────────────────────────
// Appends a machine-parsable snapshot of the monitor enumeration to
// `diagnostics.log` in the app data dir, only when the enumerated set
// changes (identity set = edid_id + connected flags, order-insensitive).
// Pure helpers are unit-tested below; the IO wrapper is thin.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::monitor::Monitor;

/// One `connected=…` line per monitor, shared with the Identify overlay
/// (Task 4) so both callers produce identical text.
pub fn per_monitor_line(m: &Monitor) -> String {
    format!(
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
    )
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
/// `diagnostics.log`, if any (absent file or no header → `None`).
fn read_last_fingerprint(path: &Path) -> io::Result<Option<u64>> {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let marker = "--- diagnostics snapshot ";
    let last = text
        .rmatch_indices(marker)
        .next()
        .and_then(|(i, _)| {
            text[i..]
                .split_whitespace()
                .find(|w| w.starts_with("fingerprint="))
        })
        .and_then(|w| w[12..].parse::<u64>().ok());
    Ok(last)
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

/// UTC RFC3339 from the Unix epoch without a datetime dependency.
/// Days-to-civil algorithm (Howard Hinnant): z → (y, m, d), then H:M:S.
fn timestamp_now_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let (h, m, s) = (
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60,
    );
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
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, mo, d, h, m, s
    )
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
        };
        let snapshot = format_snapshot(&[monitor.clone()]);
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
}
