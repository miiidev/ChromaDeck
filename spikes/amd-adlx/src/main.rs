// ── Standalone AMD ADLX hardware probe (Phase 5.1) ─────────────────────────
// Portable zero-install diagnostic binary for testing ChromaDeck's ADLX FFI
// and display correlation on AMD Radeon / Ryzen iGPU machines.
//
// Transcribed from AMD's public MIT-licensed ADLX SDK C headers:
//   https://github.com/GPUOpen-LibrariesAndSDKs/ADLX (SDK/Include/)
//   Copyright Advanced Micro Devices, Inc. All rights reserved.
//
// Usage:
//   amd-adlx-spike.exe              # read-only probe (changes nothing)

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::{c_char, c_void, CStr};
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type ADLX_RESULT = i32;
type adlx_bool = u8;
type adlx_int = i32;
type adlx_uint = u32;
type adlx_long = i32;
type adlx_uint64 = u64;
type adlx_size = usize;

const ADLX_OK: ADLX_RESULT = 0;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct ADLX_IntRange {
    minValue: adlx_int,
    maxValue: adlx_int,
    step: adlx_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ADLX_GammaRamp {
    gamma: [u16; 256 * 3],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
struct ADLX_RegammaCoeff {
    coefficientA0: adlx_int,
    coefficientA1: adlx_int,
    coefficientA2: adlx_int,
    coefficientA3: adlx_int,
    gamma: adlx_int,
}

#[repr(C)]
struct IADLXSystem {
    pVtbl: *const IADLXSystemVtbl,
}

#[repr(C)]
struct IADLXSystemVtbl {
    GetHybridGraphicsType: unsafe extern "system" fn(*mut IADLXSystem, *mut i32) -> ADLX_RESULT,
    GetGPUs: unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    QueryInterface:
        unsafe extern "system" fn(*mut IADLXSystem, *const u16, *mut *mut c_void) -> ADLX_RESULT,
    GetDisplaysServices:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut IADLXDisplayServices) -> ADLX_RESULT,
    GetDesktopsServices:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    GetGPUsChangedHandling:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    EnableLog: unsafe extern "system" fn(
        *mut IADLXSystem,
        i32,
        i32,
        *mut c_void,
        *const u16,
    ) -> ADLX_RESULT,
    Get3DSettingsServices:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    GetGPUTuningServices:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    GetPerformanceMonitoringServices:
        unsafe extern "system" fn(*mut IADLXSystem, *mut *mut c_void) -> ADLX_RESULT,
    TotalSystemRAM: unsafe extern "system" fn(*mut IADLXSystem, *mut adlx_uint) -> ADLX_RESULT,
    GetI2C:
        unsafe extern "system" fn(*mut IADLXSystem, *mut c_void, *mut *mut c_void) -> ADLX_RESULT,
}

#[repr(C)]
struct IADLXDisplayServices {
    pVtbl: *const IADLXDisplayServicesVtbl,
}

#[repr(C)]
struct IADLXDisplayServicesVtbl {
    Acquire: unsafe extern "system" fn(*mut IADLXDisplayServices) -> adlx_long,
    Release: unsafe extern "system" fn(*mut IADLXDisplayServices) -> adlx_long,
    QueryInterface: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *const u16,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetNumberOfDisplays:
        unsafe extern "system" fn(*mut IADLXDisplayServices, *mut adlx_uint) -> ADLX_RESULT,
    GetDisplays: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut *mut IADLXDisplayList,
    ) -> ADLX_RESULT,
    Get3DLUT: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetGamut: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetGamma: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut IADLXDisplayGamma,
    ) -> ADLX_RESULT,
    GetDisplayChangedHandling:
        unsafe extern "system" fn(*mut IADLXDisplayServices, *mut *mut c_void) -> ADLX_RESULT,
    GetFreeSync: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetVirtualSuperResolution: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetGPUScaling: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetScalingMode: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetIntegerScaling: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetColorDepth: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetPixelFormat: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetCustomColor: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut IADLXDisplayCustomColor,
    ) -> ADLX_RESULT,
    GetHDCP: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetCustomResolution: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    GetVariBright: unsafe extern "system" fn(
        *mut IADLXDisplayServices,
        *mut IADLXDisplay,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
}

#[repr(C)]
struct IADLXDisplayList {
    pVtbl: *const IADLXDisplayListVtbl,
}

#[repr(C)]
struct IADLXDisplayListVtbl {
    Acquire: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_long,
    Release: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_long,
    QueryInterface: unsafe extern "system" fn(
        *mut IADLXDisplayList,
        *const u16,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    Size: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_uint,
    Empty: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_bool,
    Begin: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_uint,
    End: unsafe extern "system" fn(*mut IADLXDisplayList) -> adlx_uint,
    At: unsafe extern "system" fn(
        *mut IADLXDisplayList,
        adlx_uint,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    Clear: unsafe extern "system" fn(*mut IADLXDisplayList) -> ADLX_RESULT,
    Remove_Back: unsafe extern "system" fn(*mut IADLXDisplayList) -> ADLX_RESULT,
    Add_Back: unsafe extern "system" fn(*mut IADLXDisplayList, *mut c_void) -> ADLX_RESULT,
    At_DisplayList: unsafe extern "system" fn(
        *mut IADLXDisplayList,
        adlx_uint,
        *mut *mut IADLXDisplay,
    ) -> ADLX_RESULT,
    Add_Back_DisplayList:
        unsafe extern "system" fn(*mut IADLXDisplayList, *mut IADLXDisplay) -> ADLX_RESULT,
}

#[repr(C)]
struct IADLXDisplay {
    pVtbl: *const IADLXDisplayVtbl,
}

#[repr(C)]
struct IADLXDisplayVtbl {
    Acquire: unsafe extern "system" fn(*mut IADLXDisplay) -> adlx_long,
    Release: unsafe extern "system" fn(*mut IADLXDisplay) -> adlx_long,
    QueryInterface:
        unsafe extern "system" fn(*mut IADLXDisplay, *const u16, *mut *mut c_void) -> ADLX_RESULT,
    ManufacturerID: unsafe extern "system" fn(*mut IADLXDisplay, *mut adlx_uint) -> ADLX_RESULT,
    DisplayType: unsafe extern "system" fn(*mut IADLXDisplay, *mut i32) -> ADLX_RESULT,
    ConnectorType: unsafe extern "system" fn(*mut IADLXDisplay, *mut i32) -> ADLX_RESULT,
    Name: unsafe extern "system" fn(*mut IADLXDisplay, *mut *const c_char) -> ADLX_RESULT,
    EDID: unsafe extern "system" fn(*mut IADLXDisplay, *mut *const c_char) -> ADLX_RESULT,
    NativeResolution:
        unsafe extern "system" fn(*mut IADLXDisplay, *mut adlx_int, *mut adlx_int) -> ADLX_RESULT,
    RefreshRate: unsafe extern "system" fn(*mut IADLXDisplay, *mut f64) -> ADLX_RESULT,
    PixelClock: unsafe extern "system" fn(*mut IADLXDisplay, *mut adlx_uint) -> ADLX_RESULT,
    ScanType: unsafe extern "system" fn(*mut IADLXDisplay, *mut i32) -> ADLX_RESULT,
    GetGPU: unsafe extern "system" fn(*mut IADLXDisplay, *mut *mut c_void) -> ADLX_RESULT,
    UniqueId: unsafe extern "system" fn(*mut IADLXDisplay, *mut adlx_size) -> ADLX_RESULT,
}

#[repr(C)]
struct IADLXDisplayCustomColor {
    pVtbl: *const IADLXCustomColorVtbl,
}

#[repr(C)]
struct IADLXCustomColorVtbl {
    Acquire: unsafe extern "system" fn(*mut IADLXDisplayCustomColor) -> adlx_long,
    Release: unsafe extern "system" fn(*mut IADLXDisplayCustomColor) -> adlx_long,
    QueryInterface: unsafe extern "system" fn(
        *mut IADLXDisplayCustomColor,
        *const u16,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    IsHueSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetHueRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
    GetHue: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetHue: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsSaturationSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetSaturationRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
    GetSaturation:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetSaturation: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsBrightnessSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetBrightnessRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
    GetBrightness:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetBrightness: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsContrastSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetContrastRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
    GetContrast:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetContrast: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsTemperatureSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetTemperatureRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
    GetTemperature:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetTemperature:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
}

#[repr(C)]
struct IADLXDisplayGamma {
    pVtbl: *const IADLXDisplayGammaVtbl,
}

#[repr(C)]
struct IADLXDisplayGammaVtbl {
    Acquire: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> adlx_long,
    Release: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> adlx_long,
    QueryInterface: unsafe extern "system" fn(
        *mut IADLXDisplayGamma,
        *const u16,
        *mut *mut c_void,
    ) -> ADLX_RESULT,
    IsCurrentReGammaRamp:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentDeGammaRamp:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentRegammaCoefficient:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    GetGammaRamp:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut ADLX_GammaRamp) -> ADLX_RESULT,
    GetGammaCoefficient:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut ADLX_RegammaCoeff) -> ADLX_RESULT,
    IsSupportedReGammaSRGB:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsSupportedReGammaBT709:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsSupportedReGammaPQ:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsSupportedReGammaPQ2084Interim:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsSupportedReGamma36:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentReGammaSRGB:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentReGammaBT709:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentReGammaPQ:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentReGammaPQ2084Interim:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    IsCurrentReGamma36:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *mut adlx_bool) -> ADLX_RESULT,
    SetReGammaSRGB: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
    SetReGammaBT709: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
    SetReGammaPQ: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
    SetReGammaPQ2084Interim: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
    SetReGamma36: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
    SetReGammaCoefficient:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, ADLX_RegammaCoeff) -> ADLX_RESULT,
    SetDeGammaRamp_Memory:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, ADLX_GammaRamp) -> ADLX_RESULT,
    SetDeGammaRamp_File:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *const c_char) -> ADLX_RESULT,
    SetReGammaRamp_Memory:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, ADLX_GammaRamp) -> ADLX_RESULT,
    SetReGammaRamp_File:
        unsafe extern "system" fn(*mut IADLXDisplayGamma, *const c_char) -> ADLX_RESULT,
    ResetGammaRamp: unsafe extern "system" fn(*mut IADLXDisplayGamma) -> ADLX_RESULT,
}

type ADLXQueryFullVersion_Fn = unsafe extern "C" fn(*mut adlx_uint64) -> ADLX_RESULT;
type ADLXInitialize_Fn = unsafe extern "C" fn(adlx_uint64, *mut *mut IADLXSystem) -> ADLX_RESULT;
type ADLXTerminate_Fn = unsafe extern "C" fn() -> ADLX_RESULT;

fn edid_id_from_hex(raw_hex: &str) -> Option<String> {
    let clean: String = raw_hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() < 256 {
        return None;
    }
    let bytes = hex::decode(&clean[..256]).ok()?;
    let ed: &[u8; 128] = bytes.as_slice().try_into().ok()?;
    for off in [0x36, 0x48, 0x5A, 0x6C] {
        if ed[off] == 0 && ed[off + 1] == 0 && ed[off + 3] == 0xFF {
            let s: String = ed[off + 4..off + 18]
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect();
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let mfr = u16::from_be_bytes([ed[8], ed[9]]);
    let prod = u16::from_le_bytes([ed[10], ed[11]]);
    let ser = u32::from_le_bytes([ed[12], ed[13], ed[14], ed[15]]);
    Some(format!("EDID:{mfr:04X}-{prod:04X}-{ser:08X}"))
}

fn main() {
    println!("=== ChromaDeck AMD ADLX Hardware Probe ===");
    let dll_name: Vec<u16> = "amdadlx64.dll\0".encode_utf16().collect();
    let module: HMODULE = match unsafe { LoadLibraryW(PCWSTR::from_raw(dll_name.as_ptr())) } {
        Ok(m) if !m.is_invalid() => m,
        Err(e) => {
            println!("amdadlx64.dll not loaded: {e}");
            println!("(Expected on systems without an AMD Radeon / Ryzen GPU driver.)");
            return;
        }
        Ok(_) => {
            println!("amdadlx64.dll returned invalid HMODULE");
            return;
        }
    };

    unsafe {
        let qver: Option<ADLXQueryFullVersion_Fn> =
            GetProcAddress(module, PCSTR(b"ADLXQueryFullVersion\0".as_ptr()))
                .map(|f| std::mem::transmute(f));
        let init: Option<ADLXInitialize_Fn> =
            GetProcAddress(module, PCSTR(b"ADLXInitialize\0".as_ptr()))
                .map(|f| std::mem::transmute(f));
        let term: Option<ADLXTerminate_Fn> =
            GetProcAddress(module, PCSTR(b"ADLXTerminate\0".as_ptr()))
                .map(|f| std::mem::transmute(f));

        let (Some(qver), Some(init), Some(term)) = (qver, init, term) else {
            println!("Missing required ADLX exports in amdadlx64.dll");
            let _ = FreeLibrary(module);
            return;
        };

        let mut full_ver: u64 = 0;
        let r = qver(&mut full_ver);
        println!(
            "ADLXQueryFullVersion -> rc={r}, version={}.{}.{}.{}",
            (full_ver >> 48) & 0xFFFF,
            (full_ver >> 32) & 0xFFFF,
            (full_ver >> 16) & 0xFFFF,
            full_ver & 0xFFFF
        );

        let mut system: *mut IADLXSystem = std::ptr::null_mut();
        let r = init(full_ver, &mut system);
        if r != ADLX_OK || system.is_null() {
            println!("ADLXInitialize failed: rc={r}");
            let _ = FreeLibrary(module);
            return;
        }
        println!("ADLXInitialize -> OK");

        let mut disp_svc: *mut IADLXDisplayServices = std::ptr::null_mut();
        let r = ((*(*system).pVtbl).GetDisplaysServices)(system, &mut disp_svc);
        if r == ADLX_OK && !disp_svc.is_null() {
            let mut list: *mut IADLXDisplayList = std::ptr::null_mut();
            let r = ((*(*disp_svc).pVtbl).GetDisplays)(disp_svc, &mut list);
            if r == ADLX_OK && !list.is_null() {
                let count = ((*(*list).pVtbl).Size)(list);
                println!("Displays found: {count}");
                for i in 0..count {
                    let mut disp: *mut IADLXDisplay = std::ptr::null_mut();
                    if ((*(*list).pVtbl).At_DisplayList)(list, i, &mut disp) != ADLX_OK
                        || disp.is_null()
                    {
                        continue;
                    }
                    let mut name_ptr: *const c_char = std::ptr::null();
                    let _ = ((*(*disp).pVtbl).Name)(disp, &mut name_ptr);
                    let name = if name_ptr.is_null() {
                        "<unknown>".to_string()
                    } else {
                        CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
                    };

                    let mut edid_ptr: *const c_char = std::ptr::null();
                    let _ = ((*(*disp).pVtbl).EDID)(disp, &mut edid_ptr);
                    let edid_hex = if edid_ptr.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(edid_ptr).to_string_lossy().into_owned()
                    };
                    let key = edid_id_from_hex(&edid_hex)
                        .unwrap_or_else(|| "<unparseable EDID>".into());

                    let mut uid: adlx_size = 0;
                    let _ = ((*(*disp).pVtbl).UniqueId)(disp, &mut uid);
                    let mut conn: i32 = 0;
                    let _ = ((*(*disp).pVtbl).ConnectorType)(disp, &mut conn);

                    println!("\nDisplay [{i}]: name=\"{name}\" unique_id={uid} connector={conn} edid_hex_len={} edid_id={key}", edid_hex.len());

                    let mut cc: *mut IADLXDisplayCustomColor = std::ptr::null_mut();
                    if ((*(*disp_svc).pVtbl).GetCustomColor)(disp_svc, disp, &mut cc) == ADLX_OK
                        && !cc.is_null()
                    {
                        let probe_ctrl = |label: &str,
                                          sup_fn: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
                                          rng_fn: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut ADLX_IntRange) -> ADLX_RESULT,
                                          get_fn: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT| {
                            let mut sup: adlx_bool = 0;
                            let mut rng = ADLX_IntRange::default();
                            let mut cur: adlx_int = 0;
                            let s_rc = sup_fn(cc, &mut sup);
                            let r_rc = rng_fn(cc, &mut rng);
                            let g_rc = get_fn(cc, &mut cur);
                            println!(
                                "  CustomColor.{label}: supported={} (rc={s_rc}) range={}..={} step={} (rc={r_rc}) current={cur} (rc={g_rc})",
                                sup != 0, rng.minValue, rng.maxValue, rng.step
                            );
                        };
                        let v = &*(*cc).pVtbl;
                        probe_ctrl("Brightness", v.IsBrightnessSupported, v.GetBrightnessRange, v.GetBrightness);
                        probe_ctrl("Contrast", v.IsContrastSupported, v.GetContrastRange, v.GetContrast);
                        probe_ctrl("Saturation", v.IsSaturationSupported, v.GetSaturationRange, v.GetSaturation);
                        probe_ctrl("Hue", v.IsHueSupported, v.GetHueRange, v.GetHue);
                        probe_ctrl("Temperature", v.IsTemperatureSupported, v.GetTemperatureRange, v.GetTemperature);
                        ((*(*cc).pVtbl).Release)(cc);
                    } else {
                        println!("  CustomColor: unavailable");
                    }

                    let mut gamma: *mut IADLXDisplayGamma = std::ptr::null_mut();
                    if ((*(*disp_svc).pVtbl).GetGamma)(disp_svc, disp, &mut gamma) == ADLX_OK
                        && !gamma.is_null()
                    {
                        let gv = &*(*gamma).pVtbl;
                        let mut pq = 0u8;
                        let mut pq_int = 0u8;
                        let mut srgb = 0u8;
                        let mut custom_ramp = 0u8;
                        let _ = (gv.IsCurrentReGammaPQ)(gamma, &mut pq);
                        let _ = (gv.IsCurrentReGammaPQ2084Interim)(gamma, &mut pq_int);
                        let _ = (gv.IsCurrentReGammaSRGB)(gamma, &mut srgb);
                        let _ = (gv.IsCurrentReGammaRamp)(gamma, &mut custom_ramp);
                        println!(
                            "  Gamma: sRGB={} custom_regamma_ramp={} PQ(HDR)={} PQ2084={}",
                            srgb != 0,
                            custom_ramp != 0,
                            pq != 0,
                            pq_int != 0
                        );
                        ((*(*gamma).pVtbl).Release)(gamma);
                    } else {
                        println!("  Gamma: unavailable");
                    }

                    ((*(*disp).pVtbl).Release)(disp);
                }
                ((*(*list).pVtbl).Release)(list);
            }
            ((*(*disp_svc).pVtbl).Release)(disp_svc);
        }

        let trc = term();
        println!("\nADLXTerminate -> rc={trc}");
        let _ = FreeLibrary(module);
    }
}
