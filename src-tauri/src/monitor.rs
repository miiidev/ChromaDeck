// ── EDID-based monitor enumeration ──────────────────────────────────────────
// Windows-only. Uses SetupAPI + registry for canonical EDID reads, falling
// back to EnumDisplayDevices when EDID data is unavailable (virtual displays).

use serde::Serialize;
use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use winreg::enums::*;
use winreg::RegKey;
use winreg::types::FromRegValue;
use winreg::RegValue;
use windows::core::GUID;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Graphics::Gdi::*;

/// GUID for the monitor device interface class.
const GUID_DEVINTERFACE_MONITOR: GUID =
    GUID::from_u128(0xE6F07B5F_EE97_4a90_A076_33F57BF4EAA7);

/// A monitor identified by its stable EDID serial / product string.
#[derive(Debug, Clone, Serialize)]
pub struct Monitor {
    pub edid_id: String,
    pub model: String,
    pub serial: String,
    pub connected: bool,
    pub device_name: String, // e.g. "\\.\DISPLAY1"
}

// ── EDID parsing ───────────────────────────────────────────────────────────

/// Parse the serial-number string from a raw 128‑byte EDID block.
///
/// EDID stores monitor descriptors (name, serial, etc.) in four 18‑byte blocks
/// starting at offsets 0x36, 0x48, 0x5A and 0x6C.  A descriptor with tag 0xFF
/// contains the serial as a 13‑byte ASCII string (padded with spaces / 0x0A).
pub fn parse_edid_serial(edid: &[u8; 128]) -> Option<String> {
    for off in [0x36, 0x48, 0x5A, 0x6C] {
        // Monitor descriptors have 0x0000 in the first two bytes.
        if edid[off] != 0x00 || edid[off + 1] != 0x00 {
            continue;
        }
        if edid[off + 3] == 0xFF {
            // Bytes 4..17 carry the serial string.
            let raw = &edid[off + 4..off + 18];
            let s: String = raw
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect();
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

/// Parse the monitor-name string from a raw 128‑byte EDID block.
///
/// Descriptor tag 0xFC holds the monitor's name as a 13‑byte ASCII string.
pub fn parse_edid_model_name(edid: &[u8; 128]) -> Option<String> {
    for off in [0x36, 0x48, 0x5A, 0x6C] {
        if edid[off] != 0x00 || edid[off + 1] != 0x00 {
            continue;
        }
        if edid[off + 3] == 0xFC {
            let raw = &edid[off + 4..off + 18];
            let s: String = raw
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect();
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

// ── Monitor enumeration ────────────────────────────────────────────────────

/// Enumerate all connected monitors.  Primary path uses SetupAPI + registry
/// to get a canonical EDID; falls back to `EnumDisplayDevicesW` when the
/// EDID blob is missing (e.g. virtual / remote displays).
pub fn list_monitors() -> Vec<Monitor> {
    // ── Attempt 1: SetupAPI + registry EDID ───────────────────────────
    if let Ok(monitors) = enum_setupapi_monitors() {
        if !monitors.is_empty() {
            return monitors;
        }
    }

    // ── Attempt 2: fallback to EnumDisplayDevices ─────────────────────
    enum_fallback_displays()
}

// ── Helper: wide-char array to String ─────────────────────────────────────

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..len])
        .to_string_lossy()
        .to_string()
}

// ── SetupAPI approach (primary) ───────────────────────────────────────────

fn enum_setupapi_monitors() -> Result<Vec<Monitor>, windows::core::Error> {
    let mut monitors = Vec::new();

    // SAFETY: SetupDiGetClassDevsW enumerates present monitor devices.
    let dev_info_set = unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_MONITOR as *const GUID),
            None,
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )?
    };

    // Iterator over device info elements.
    let mut dev_index: u32 = 0;
    loop {
        let mut dev_info_data = SP_DEVINFO_DATA::default();
        dev_info_data.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;

        // SAFETY: SetupDiEnumDeviceInfo iterates the device set.
        let ok = unsafe {
            SetupDiEnumDeviceInfo(dev_info_set, dev_index, &mut dev_info_data)
        };
        if ok.is_err() {
            break; // no more devices
        }
        dev_index += 1;

        // Get device instance ID — this gives the path into the registry.
        let instance_id = match get_device_instance_id(dev_info_set, &dev_info_data) {
            Some(id) => id,
            None => continue,
        };

        // Read the EDID registry value.
        let edid_bytes = match read_edid_registry(&instance_id) {
            Some(b) => b,
            None => continue,
        };

        // Parse as 128-byte EDID.
        let edid: &[u8; 128] = match edid_bytes.as_slice().try_into() {
            Ok(arr) => arr,
            Err(_) => continue,
        };

        let serial = parse_edid_serial(edid).unwrap_or_default();
        let model = parse_edid_model_name(edid)
            .or_else(|| get_device_desc(dev_info_set, &dev_info_data))
            .or_else(|| {
                instance_id
                    .split('\\')
                    .nth(1)
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| instance_id.clone());

        // Use EDID product serial as edid_id when available, else instance path.
        let edid_id = if serial.is_empty() {
            instance_id.clone()
        } else {
            serial.clone()
        };

        monitors.push(Monitor {
            edid_id,
            model,
            serial,
            connected: true, // DIGCF_PRESENT already filters to present
            device_name: String::new(),
        });
    }

    // Cross-reference with EnumDisplayDevices to populate device_name
    enrich_with_device_names(&mut monitors);

    // SAFETY: Destroy the device info set.
    unsafe { SetupDiDestroyDeviceInfoList(dev_info_set)? };

    Ok(monitors)
}

/// Read the device instance ID string.
fn get_device_instance_id(
    dev_info_set: HDEVINFO,
    dev_info_data: &SP_DEVINFO_DATA,
) -> Option<String> {
    let mut buf = [0u16; 512];

    // SAFETY: SetupDiGetDeviceInstanceIdW writes into the buffer.
    let result = unsafe {
        SetupDiGetDeviceInstanceIdW(
            dev_info_set,
            dev_info_data as *const SP_DEVINFO_DATA,
            Some(&mut buf),
            None,
        )
    };
    if result.is_err() {
        return None;
    }

    Some(wide_to_string(&buf))
}

/// Read the device description from SetupAPI.
fn get_device_desc(
    dev_info_set: HDEVINFO,
    dev_info_data: &SP_DEVINFO_DATA,
) -> Option<String> {
    let mut buf = [0u8; 512]; // buffer in bytes
    let mut data_type: u32 = 0;

    // SAFETY: SetupDiGetDeviceRegistryPropertyW reads SPDRP_DEVICEDESC.
    let result = unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            dev_info_set,
            dev_info_data as *const SP_DEVINFO_DATA,
            SPDRP_DEVICEDESC,
            Some(&mut data_type),
            Some(&mut buf),
            None,
        )
    };
    if result.is_err() {
        return None;
    }

    // The property comes back as REG_SZ (UTF-16LE), but we read it into bytes.
    // The first null-terminated pair of bytes is our string.
    let len = buf
        .chunks_exact(2)
        .position(|pair| pair[0] == 0 && pair[1] == 0)
        .unwrap_or(buf.len() / 2);

    let wide: Vec<u16> = buf[..len * 2]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    let s = OsString::from_wide(&wide)
        .to_string_lossy()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// A newtype so we can implement `FromRegValue` for raw EDID bytes.
struct EdidBlob(Vec<u8>);

impl FromRegValue for EdidBlob {
    fn from_reg_value(val: &RegValue) -> io::Result<Self> {
        Ok(EdidBlob(val.bytes.clone()))
    }
}

/// Read the 128‑byte EDID blob from the registry for a given device instance.
///
/// The EDID is stored at:
///   HKLM\SYSTEM\CurrentControlSet\Enum\<instance_id>\Device Parameters\EDID
fn read_edid_registry(instance_id: &str) -> Option<Vec<u8>> {
    let enum_key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Enum", KEY_READ)
        .ok()?;

    let device_key = enum_key
        .open_subkey_with_flags(instance_id, KEY_READ)
        .ok()?;

    let params_key = device_key
        .open_subkey_with_flags(r"Device Parameters", KEY_READ)
        .ok()?;

    let blob: EdidBlob = params_key.get_value("EDID").ok()?;
    if blob.0.len() < 128 {
        return None;
    }
    Some(blob.0)
}

// ── EnumDisplayDevices fallback ──────────────────────────────────────────

/// Cross‑reference monitors from SetupAPI with EnumDisplayDevices to get
/// the user‑mode display device name (e.g. `\\.\DISPLAY1`) needed for
/// CreateDC / SetDeviceGammaRamp.
fn enrich_with_device_names(monitors: &mut Vec<Monitor>) {
    for disp_index in 0.. {
        let mut dev = DISPLAY_DEVICEW::default();
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let ok = unsafe { EnumDisplayDevicesW(None, disp_index, &mut dev, 0) };
        if !ok.as_bool() {
            break;
        }

        let device_id = wide_to_string(&dev.DeviceID);
        if device_id.is_empty() {
            continue;
        }

        // Match by EDID device ID
        for mon in monitors.iter_mut() {
            if mon.device_name.is_empty() && mon.edid_id == device_id {
                mon.device_name = wide_to_string(&dev.DeviceName);
            }
        }
    }
}

fn enum_fallback_displays() -> Vec<Monitor> {
    let mut monitors = Vec::new();

    for disp_index in 0.. {
        let mut dev = DISPLAY_DEVICEW::default();
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        // SAFETY: EnumDisplayDevicesW is a straightforward Win32 call.
        let ok = unsafe { EnumDisplayDevicesW(None, disp_index, &mut dev, 0) };
        if !ok.as_bool() {
            break;
        }

        let device_id = wide_to_string(&dev.DeviceID);
        let device_string = wide_to_string(&dev.DeviceString);
        let device_name = wide_to_string(&dev.DeviceName);

        if device_id.is_empty() {
            continue;
        }

        let connected = (dev.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP)
            == DISPLAY_DEVICE_ATTACHED_TO_DESKTOP;

        monitors.push(Monitor {
            edid_id: device_id.clone(),
            model: device_string,
            serial: String::new(),
            connected,
            device_name: device_name,
        });
    }

    monitors
}

// ── Tauri command ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_monitors_cmd() -> Vec<Monitor> {
    list_monitors()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal 128‑byte EDID with a serial‑number descriptor at
    /// offset 0x48 (block 2).  Everything else is zeroed.
    fn fake_edid_blob(serial: &str) -> Box<[u8; 128]> {
        let mut buf = Box::new([0u8; 128]);
        // EDID header (magic)
        buf[0..8]
            .copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);

        // Place a serial descriptor at offset 0x48 (18‑byte block 2).
        let off = 0x48;
        buf[off] = 0x00; // word low
        buf[off + 1] = 0x00; // word high  → monitor descriptor
        buf[off + 2] = 0x00; // reserved
        buf[off + 3] = 0xFF; // serial tag

        // Copy the string into bytes 4..17, pad with 0x0A.
        let raw = serial.as_bytes();
        let n = raw.len().min(13);
        buf[off + 4..off + 4 + n].copy_from_slice(&raw[..n]);
        if n < 13 {
            buf[off + 4 + n] = 0x0A; // terminate
        }
        buf
    }

    /// Builds a minimal EDID with a monitor-name descriptor at offset 0x36.
    fn fake_edid_blob_with_model(name: &str) -> Box<[u8; 128]> {
        let mut buf = Box::new([0u8; 128]);
        buf[0..8]
            .copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);

        let off = 0x36;
        buf[off] = 0x00;
        buf[off + 1] = 0x00;
        buf[off + 2] = 0x00;
        buf[off + 3] = 0xFC; // monitor name tag

        let raw = name.as_bytes();
        let n = raw.len().min(13);
        buf[off + 4..off + 4 + n].copy_from_slice(&raw[..n]);
        if n < 13 {
            buf[off + 4 + n] = 0x0A;
        }
        buf
    }

    // ── parse_edid_serial ──────────────────────────────────────────────

    #[test]
    fn parses_edid_serial() {
        let edid = fake_edid_blob("ABC123");
        let got = parse_edid_serial(&edid).expect("should find serial");
        assert_eq!(got, "ABC123");
    }

    #[test]
    fn returns_none_for_edid_without_serial() {
        let edid = fake_edid_blob_with_model("Test Monitor");
        let got = parse_edid_serial(&edid);
        assert!(got.is_none(), "no serial descriptor present");
    }

    #[test]
    fn handles_empty_serial_descriptor() {
        let mut buf = Box::new([0u8; 128]);
        buf[0..8]
            .copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);

        let off = 0x36;
        buf[off] = 0x00;
        buf[off + 1] = 0x00;
        buf[off + 2] = 0x00;
        buf[off + 3] = 0xFF;
        buf[off + 4] = 0x0A; // empty string

        let edid: &[u8; 128] = &*buf;
        let got = parse_edid_serial(edid);
        assert!(got.is_none(), "empty serial yields None");
    }

    #[test]
    fn serial_stops_at_newline() {
        let buf = fake_edid_blob("ABC123");
        let got = parse_edid_serial(&buf).expect("should find serial");
        assert_eq!(got, "ABC123", "trailing 0x0A should be stripped");
    }

    // ── parse_edid_model_name ──────────────────────────────────────────

    #[test]
    fn parses_edid_model_name() {
        let edid = fake_edid_blob_with_model("DELL U2719D");
        let got = parse_edid_model_name(&edid).expect("should find model name");
        assert_eq!(got, "DELL U2719D");
    }

    #[test]
    fn returns_none_for_edid_without_model_name() {
        let edid = fake_edid_blob("serial123");
        let got = parse_edid_model_name(&edid);
        assert!(got.is_none(), "no model name descriptor present");
    }

    #[test]
    fn model_name_stops_at_newline() {
        let edid = fake_edid_blob_with_model("Samsung S24");
        let got = parse_edid_model_name(&edid).expect("should find model name");
        assert_eq!(got, "Samsung S24");
    }
}