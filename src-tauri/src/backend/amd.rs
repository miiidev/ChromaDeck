// ── AMD ADLX native display backend (Phase 5) ──────────────────────────────
// Windows-only, x64-only. Dynamically loads `amdadlx64.dll` at runtime via
// `LoadLibraryW` / `GetProcAddress`. Machines without AMD drivers skip in
// < 1 ms and never panic or fail other backends.
//
// C ABI structs and vtables below are transcribed verbatim from AMD's public
// MIT-licensed ADLX SDK headers (`GPUOpen-LibrariesAndSDKs/ADLX`, `SDK/Include/`):
//   ADLX.h, ADLXDefines.h, ADLXStructures.h, ISystem.h, IDisplays.h,
//   IDisplaySettings.h, IDisplayGamma.h.
// Copyright (c) Advanced Micro Devices, Inc. All rights reserved.
//
// ALL `unsafe` FFI is strictly encapsulated in this module. All policy,
// correlation, math, apply, restore, and drift logic runs through the mockable
// `AmdApi` trait so every path is unit-tested without AMD hardware.

#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    clippy::missing_safety_doc,
    clippy::missing_transmute_annotations
)]

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use super::{
    append_error, AdjustmentCap, AdjustmentCapability, ColorBackend, ADJUSTMENT_BRIGHTNESS,
    ADJUSTMENT_CONTRAST, ADJUSTMENT_GAMMA, ADJUSTMENT_HUE, ADJUSTMENT_RGB_GAINS,
    ADJUSTMENT_SATURATION, ADJUSTMENT_VIBRANCE,
};
use crate::color::{gdi_identity_purge, ApplyResult, ColorApi};
use crate::nvapi::NvColorApi;
use crate::store::Preset;

// ── ADLX C ABI types (verbatim from ADLX SDK headers) ───────────────────────

type ADLX_RESULT = i32;
type adlx_bool = u8;
type adlx_int = i32;
type adlx_uint = u32;
type adlx_long = i32;
type adlx_uint64 = u64;
type adlx_size = usize;

const ADLX_OK: ADLX_RESULT = 0;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct AdlxIntRange {
    pub min_value: adlx_int,
    pub max_value: adlx_int,
    pub step: adlx_int,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
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
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut AdlxIntRange) -> ADLX_RESULT,
    GetHue: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetHue: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsSaturationSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetSaturationRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut AdlxIntRange) -> ADLX_RESULT,
    GetSaturation:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetSaturation: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsBrightnessSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetBrightnessRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut AdlxIntRange) -> ADLX_RESULT,
    GetBrightness:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetBrightness: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsContrastSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetContrastRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut AdlxIntRange) -> ADLX_RESULT,
    GetContrast:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_int) -> ADLX_RESULT,
    SetContrast: unsafe extern "system" fn(*mut IADLXDisplayCustomColor, adlx_int) -> ADLX_RESULT,
    IsTemperatureSupported:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut adlx_bool) -> ADLX_RESULT,
    GetTemperatureRange:
        unsafe extern "system" fn(*mut IADLXDisplayCustomColor, *mut AdlxIntRange) -> ADLX_RESULT,
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

// ── Domain types & mockable FFI seam ────────────────────────────────────────

/// One display enumerated from ADLX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdDisplayInfo {
    /// Zero-based index inside ADLX's current display list (or stable unique id).
    pub unique_id: usize,
    pub name: String,
    /// Derived ChromaDeck `edid_id` from ADLX's EDID hex string.
    pub edid_id: String,
}

/// Live capability & value snapshot for one AMD display's Custom Color + Gamma.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AmdColorState {
    pub hdr_active: bool,
    pub brightness_supported: bool,
    pub brightness_range: AdlxIntRange,
    pub brightness: i32,
    pub contrast_supported: bool,
    pub contrast_range: AdlxIntRange,
    pub contrast: i32,
    pub saturation_supported: bool,
    pub saturation_range: AdlxIntRange,
    pub saturation: i32,
    pub hue_supported: bool,
    pub hue_range: AdlxIntRange,
    pub hue: i32,
    pub gamma_available: bool,
    pub custom_regamma_active: bool,
}

/// Desired Custom Color writes for an `apply` or `restore` pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AmdCustomColorWrite {
    pub brightness: Option<i32>,
    pub contrast: Option<i32>,
    pub saturation: Option<i32>,
    pub hue: Option<i32>,
}

/// Hardware seam so every correlation, mapping, apply, restore, and drift path
/// is unit-tested cleanly without AMD hardware installed.
pub trait AmdApi: Send + Sync {
    fn enumerate_displays(&self) -> Result<Vec<AmdDisplayInfo>, String>;
    fn read_color_state(&self, unique_id: usize) -> Result<AmdColorState, String>;
    fn write_custom_color(
        &self,
        unique_id: usize,
        write: &AmdCustomColorWrite,
    ) -> Result<(), String>;
    fn write_regamma_ramp(&self, unique_id: usize, ramp: &[u16; 768]) -> Result<(), String>;
    fn reset_gamma_ramp(&self, unique_id: usize) -> Result<(), String>;
    fn read_gamma_ramp(&self, unique_id: usize) -> Result<[u16; 768], String>;
}

// ── Pure EDID & transfer math helpers ───────────────────────────────────────

/// Parse ADLX's ASCII hex EDID string into ChromaDeck's canonical `edid_id`
/// using `crate::monitor::edid_id_from_bytes` so keys match byte-for-byte.
pub fn edid_id_from_hex(raw_hex: &str) -> Option<String> {
    let clean: String = raw_hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() < 256 {
        return None;
    }
    let bytes = hex::decode(&clean[..256]).ok()?;
    let block: &[u8; 128] = bytes.as_slice().try_into().ok()?;
    Some(crate::monitor::edid_id_from_bytes(block))
}

/// Neutral midpoint of an `AdlxIntRange`, snapped to `step`.
pub fn range_midpoint(range: AdlxIntRange) -> i32 {
    if range.max_value <= range.min_value {
        return range.min_value;
    }
    let mid = (range.min_value as i64 + range.max_value as i64) / 2;
    snap_to_range(mid as f64, range)
}

fn snap_to_range(raw: f64, range: AdlxIntRange) -> i32 {
    if range.max_value <= range.min_value {
        return range.min_value;
    }
    let step = range.step.max(1) as f64;
    let offset = ((raw - range.min_value as f64) / step).round() * step;
    let snapped = range.min_value as f64 + offset;
    (snapped.round() as i32).clamp(range.min_value, range.max_value)
}

/// Map ChromaDeck's 0..=100 (neutral 50) UI slider onto an ADLX integer range.
pub fn map_ui_0_100_to_range(ui_val: f64, range: AdlxIntRange) -> i32 {
    if range.max_value <= range.min_value {
        return range.min_value;
    }
    let clamped = ui_val.clamp(0.0, 100.0);
    if (clamped - 50.0).abs() < 1e-6 {
        return range_midpoint(range);
    }
    let mid = range_midpoint(range) as f64;
    let raw = if clamped < 50.0 {
        let t = clamped / 50.0;
        range.min_value as f64 + t * (mid - range.min_value as f64)
    } else {
        let t = (clamped - 50.0) / 50.0;
        mid + t * (range.max_value as f64 - mid)
    };
    snap_to_range(raw, range)
}

/// Map ChromaDeck's 0..=359° hue (0° neutral) onto an ADLX signed hue range
/// (typically -30..=30). Wraps `> 180°` to negative `(-180..0)`.
pub fn map_hue_deg_to_range(hue_deg: f64, range: AdlxIntRange) -> i32 {
    if range.max_value <= range.min_value {
        return range.min_value;
    }
    let wrapped = ((hue_deg % 360.0) + 360.0) % 360.0;
    let signed = if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    };
    if signed.abs() < 1e-6 {
        return range_midpoint(range);
    }
    let mid = range_midpoint(range) as f64;
    let raw = if signed < 0.0 {
        let t = (signed + 180.0) / 180.0; // 0 at -180, 1 at 0
        range.min_value as f64 + t * (mid - range.min_value as f64)
    } else {
        let t = signed / 180.0; // 0 at 0, 1 at +180
        mid + t * (range.max_value as f64 - mid)
    };
    snap_to_range(raw, range)
}

/// Standard sRGB OETF (linear 0..1 → encoded 0..1).
fn srgb_oetf(linear: f64) -> f64 {
    let x = linear.clamp(0.0, 1.0);
    if x < 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Build an AMD ADLX planar `[u16; 768]` re-gamma LUT (`[R0..255, G0..255, B0..255]`).
/// Anchored to the standard sRGB OETF curve so neutral (`gamma = 1.0`,
/// `rgb_gains = [1, 1, 1]`) matches sRGB, while `gamma > 1.0` lifts midtones
/// and `gamma < 1.0` darkens midtones (matching NVAPI/Intel/GDI direction).
pub fn build_regamma_ramp(gamma: f64, rgb_gains: [f64; 3]) -> [u16; 768] {
    let mut ramp = [0u16; 768];
    let safe_gamma = gamma.clamp(0.3, 2.8);
    let inv_gamma = 1.0 / safe_gamma;

    for ch in 0..3 {
        let gain = rgb_gains[ch].clamp(0.0, 10.0);
        for i in 0..256 {
            let x = i as f64 / 255.0;
            let base = srgb_oetf(x);
            let curved = if base <= 0.0 {
                0.0
            } else {
                base.powf(inv_gamma)
            };
            let scaled = (curved * gain).clamp(0.0, 1.0);
            ramp[ch * 256 + i] = (scaled * 65535.0).round() as u16;
        }
    }
    ramp
}

fn is_neutral_gamma_and_gains(preset: &Preset) -> bool {
    (preset.gamma - 1.0).abs() < 1e-4
        && (preset.rgb_gains[0] - 1.0).abs() < 1e-4
        && (preset.rgb_gains[1] - 1.0).abs() < 1e-4
        && (preset.rgb_gains[2] - 1.0).abs() < 1e-4
}

/// Resolve an EDID key to a unique AMD display. Ambiguous (2+ identical EDIDs)
/// or absent displays fail honestly so we never write to the wrong monitor.
fn resolve_unique_display(
    displays: &[AmdDisplayInfo],
    edid_id: &str,
) -> Result<AmdDisplayInfo, String> {
    let matches: Vec<&AmdDisplayInfo> = displays
        .iter()
        .filter(|d| !d.edid_id.is_empty() && crate::monitor::key_match(&d.edid_id, edid_id))
        .collect();
    match matches.len() {
        0 => Err(format!("no AMD ADLX display matched {edid_id}")),
        1 => Ok(matches[0].clone()),
        n => Err(format!(
            "ambiguous AMD ADLX display correlation for {edid_id} ({n} displays share this EDID)"
        )),
    }
}

// ── Real ADLX FFI session (lazy, process-lifetime, Send+Sync guarded) ───────

struct AdlxRuntime {
    module: HMODULE,
    system: *mut IADLXSystem,
    terminate: ADLXTerminate_Fn,
}

// SAFETY: Every ADLX call is serialized through `AdlxRuntime`'s Mutex.
unsafe impl Send for AdlxRuntime {}

impl AdlxRuntime {
    fn open_dll(dll_path: &str) -> Option<Self> {
        let wide: Vec<u16> = dll_path.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: Standard Win32 DLL load by filename/path. Returns Err in <1 ms when absent.
        let module = unsafe { LoadLibraryW(PCWSTR::from_raw(wide.as_ptr())) }.ok()?;
        if module.is_invalid() {
            return None;
        }
        unsafe {
            let qver: ADLXQueryFullVersion_Fn = std::mem::transmute(GetProcAddress(
                module,
                PCSTR(b"ADLXQueryFullVersion\0".as_ptr()),
            )?);
            let init: ADLXInitialize_Fn = std::mem::transmute(GetProcAddress(
                module,
                PCSTR(b"ADLXInitialize\0".as_ptr()),
            )?);
            let init_compat: Option<ADLXInitialize_Fn> = GetProcAddress(
                module,
                PCSTR(b"ADLXInitializeWithIncompatibleDriver\0".as_ptr()),
            )
            .map(|f| std::mem::transmute(f));
            let terminate: ADLXTerminate_Fn = std::mem::transmute(GetProcAddress(
                module,
                PCSTR(b"ADLXTerminate\0".as_ptr()),
            )?);

            let mut full_ver: adlx_uint64 = 0;
            if qver(&mut full_ver) != ADLX_OK {
                return None;
            }
            let mut system: *mut IADLXSystem = std::ptr::null_mut();
            let mut rc = init(full_ver, &mut system);
            if (rc != ADLX_OK || system.is_null()) && init_compat.is_some() {
                rc = (init_compat.unwrap())(full_ver, &mut system);
            }
            if rc != ADLX_OK || system.is_null() {
                return None;
            }
            Some(AdlxRuntime {
                module,
                system,
                terminate,
            })
        }
    }
}

static ADLX_LOADED: AtomicBool = AtomicBool::new(false);
static ADLX_SESSION: OnceLock<Option<Mutex<AdlxRuntime>>> = OnceLock::new();

fn get_adlx_session() -> Option<&'static Mutex<AdlxRuntime>> {
    ADLX_SESSION
        .get_or_init(|| {
            let rt = AdlxRuntime::open_dll("amdadlx64.dll")?;
            ADLX_LOADED.store(true, Ordering::Release);
            Some(Mutex::new(rt))
        })
        .as_ref()
}

/// True iff `amdadlx64.dll` has already been loaded and initialized in this
/// process. Never triggers a DLL load (used by `BackendRegistry::backend_id_hint`).
pub fn session_loaded() -> bool {
    ADLX_LOADED.load(Ordering::Acquire)
}

unsafe fn enumerate_displays_on(system: *mut IADLXSystem) -> Result<Vec<AmdDisplayInfo>, String> {
    let mut disp_svc: *mut IADLXDisplayServices = std::ptr::null_mut();
    let rc = ((*(*system).pVtbl).GetDisplaysServices)(system, &mut disp_svc);
    if rc != ADLX_OK || disp_svc.is_null() {
        return Err(format!("ADLX GetDisplaysServices failed (rc={rc})"));
    }
    let mut list: *mut IADLXDisplayList = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetDisplays)(disp_svc, &mut list);
    if rc != ADLX_OK || list.is_null() {
        ((*(*disp_svc).pVtbl).Release)(disp_svc);
        return Err(format!("ADLX GetDisplays failed (rc={rc})"));
    }
    let count = ((*(*list).pVtbl).Size)(list);
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let mut disp: *mut IADLXDisplay = std::ptr::null_mut();
        if ((*(*list).pVtbl).At_DisplayList)(list, i, &mut disp) != ADLX_OK || disp.is_null() {
            continue;
        }
        let mut uid: adlx_size = 0;
        let uid_ok = ((*(*disp).pVtbl).UniqueId)(disp, &mut uid) == ADLX_OK;

        let mut name_ptr: *const c_char = std::ptr::null();
        let _ = ((*(*disp).pVtbl).Name)(disp, &mut name_ptr);
        let name = if name_ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
        };

        let mut edid_ptr: *const c_char = std::ptr::null();
        let _ = ((*(*disp).pVtbl).EDID)(disp, &mut edid_ptr);
        let edid_id = if edid_ptr.is_null() {
            String::new()
        } else {
            let raw = CStr::from_ptr(edid_ptr).to_string_lossy();
            edid_id_from_hex(&raw).unwrap_or_default()
        };

        ((*(*disp).pVtbl).Release)(disp);
        if uid_ok {
            out.push(AmdDisplayInfo {
                unique_id: uid,
                name,
                edid_id,
            });
        }
    }
    ((*(*list).pVtbl).Release)(list);
    ((*(*disp_svc).pVtbl).Release)(disp_svc);
    Ok(out)
}

unsafe fn with_display_on<T>(
    system: *mut IADLXSystem,
    unique_id: usize,
    f: impl FnOnce(*mut IADLXDisplayServices, *mut IADLXDisplay) -> Result<T, String>,
) -> Result<T, String> {
    let mut disp_svc: *mut IADLXDisplayServices = std::ptr::null_mut();
    let rc = ((*(*system).pVtbl).GetDisplaysServices)(system, &mut disp_svc);
    if rc != ADLX_OK || disp_svc.is_null() {
        return Err(format!("ADLX GetDisplaysServices failed (rc={rc})"));
    }

    let mut list: *mut IADLXDisplayList = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetDisplays)(disp_svc, &mut list);
    if rc != ADLX_OK || list.is_null() {
        ((*(*disp_svc).pVtbl).Release)(disp_svc);
        return Err(format!("ADLX GetDisplays failed (rc={rc})"));
    }

    let count = ((*(*list).pVtbl).Size)(list);
    let mut found: *mut IADLXDisplay = std::ptr::null_mut();
    for i in 0..count {
        let mut disp: *mut IADLXDisplay = std::ptr::null_mut();
        if ((*(*list).pVtbl).At_DisplayList)(list, i, &mut disp) == ADLX_OK && !disp.is_null() {
            let mut uid: adlx_size = 0;
            if ((*(*disp).pVtbl).UniqueId)(disp, &mut uid) == ADLX_OK && uid == unique_id {
                found = disp;
                break;
            }
            ((*(*disp).pVtbl).Release)(disp);
        }
    }

    let result = if found.is_null() {
        Err(format!(
            "AMD ADLX display unique_id={unique_id} no longer present"
        ))
    } else {
        let res = f(disp_svc, found);
        ((*(*found).pVtbl).Release)(found);
        res
    };

    ((*(*list).pVtbl).Release)(list);
    ((*(*disp_svc).pVtbl).Release)(disp_svc);
    result
}

unsafe fn read_color_state_on(
    disp_svc: *mut IADLXDisplayServices,
    disp: *mut IADLXDisplay,
) -> Result<AmdColorState, String> {
    let mut state = AmdColorState::default();

    let mut cc: *mut IADLXDisplayCustomColor = std::ptr::null_mut();
    if ((*(*disp_svc).pVtbl).GetCustomColor)(disp_svc, disp, &mut cc) == ADLX_OK && !cc.is_null() {
        let v = &*(*cc).pVtbl;
        let mut sup: adlx_bool = 0;
        if (v.IsBrightnessSupported)(cc, &mut sup) == ADLX_OK && sup != 0 {
            state.brightness_supported = true;
            let _ = (v.GetBrightnessRange)(cc, &mut state.brightness_range);
            let _ = (v.GetBrightness)(cc, &mut state.brightness);
        }
        sup = 0;
        if (v.IsContrastSupported)(cc, &mut sup) == ADLX_OK && sup != 0 {
            state.contrast_supported = true;
            let _ = (v.GetContrastRange)(cc, &mut state.contrast_range);
            let _ = (v.GetContrast)(cc, &mut state.contrast);
        }
        sup = 0;
        if (v.IsSaturationSupported)(cc, &mut sup) == ADLX_OK && sup != 0 {
            state.saturation_supported = true;
            let _ = (v.GetSaturationRange)(cc, &mut state.saturation_range);
            let _ = (v.GetSaturation)(cc, &mut state.saturation);
        }
        sup = 0;
        if (v.IsHueSupported)(cc, &mut sup) == ADLX_OK && sup != 0 {
            state.hue_supported = true;
            let _ = (v.GetHueRange)(cc, &mut state.hue_range);
            let _ = (v.GetHue)(cc, &mut state.hue);
        }
        (v.Release)(cc);
    }

    let mut gamma: *mut IADLXDisplayGamma = std::ptr::null_mut();
    if ((*(*disp_svc).pVtbl).GetGamma)(disp_svc, disp, &mut gamma) == ADLX_OK && !gamma.is_null() {
        let gv = &*(*gamma).pVtbl;
        state.gamma_available = true;
        let mut pq: adlx_bool = 0;
        let mut pq_int: adlx_bool = 0;
        let mut custom: adlx_bool = 0;
        let _ = (gv.IsCurrentReGammaPQ)(gamma, &mut pq);
        let _ = (gv.IsCurrentReGammaPQ2084Interim)(gamma, &mut pq_int);
        let _ = (gv.IsCurrentReGammaRamp)(gamma, &mut custom);
        state.hdr_active = pq != 0 || pq_int != 0;
        state.custom_regamma_active = custom != 0;
        (gv.Release)(gamma);
    }

    Ok(state)
}

unsafe fn write_custom_color_on(
    disp_svc: *mut IADLXDisplayServices,
    disp: *mut IADLXDisplay,
    write: &AmdCustomColorWrite,
) -> Result<(), String> {
    let mut cc: *mut IADLXDisplayCustomColor = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetCustomColor)(disp_svc, disp, &mut cc);
    if rc != ADLX_OK || cc.is_null() {
        return Err(format!("ADLX GetCustomColor failed (rc={rc})"));
    }
    let v = &*(*cc).pVtbl;
    let mut err: Option<String> = None;
    if let Some(b) = write.brightness {
        let r = (v.SetBrightness)(cc, b);
        if r != ADLX_OK {
            append_error(&mut err, format!("SetBrightness({b}) rc={r}"));
        }
    }
    if let Some(c) = write.contrast {
        let r = (v.SetContrast)(cc, c);
        if r != ADLX_OK {
            append_error(&mut err, format!("SetContrast({c}) rc={r}"));
        }
    }
    if let Some(s) = write.saturation {
        let r = (v.SetSaturation)(cc, s);
        if r != ADLX_OK {
            append_error(&mut err, format!("SetSaturation({s}) rc={r}"));
        }
    }
    if let Some(h) = write.hue {
        let r = (v.SetHue)(cc, h);
        if r != ADLX_OK {
            append_error(&mut err, format!("SetHue({h}) rc={r}"));
        }
    }
    (v.Release)(cc);
    match err {
        None => Ok(()),
        Some(e) => Err(e),
    }
}

unsafe fn write_regamma_ramp_on(
    disp_svc: *mut IADLXDisplayServices,
    disp: *mut IADLXDisplay,
    ramp: &[u16; 768],
) -> Result<(), String> {
    let mut gamma: *mut IADLXDisplayGamma = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetGamma)(disp_svc, disp, &mut gamma);
    if rc != ADLX_OK || gamma.is_null() {
        return Err(format!("ADLX GetGamma failed (rc={rc})"));
    }
    let gv = &*(*gamma).pVtbl;
    let rc = (gv.SetReGammaRamp_Memory)(gamma, ADLX_GammaRamp { gamma: *ramp });
    (gv.Release)(gamma);
    if rc == ADLX_OK {
        Ok(())
    } else {
        Err(format!("ADLX SetReGammaRamp_Memory failed (rc={rc})"))
    }
}

unsafe fn reset_gamma_ramp_on(
    disp_svc: *mut IADLXDisplayServices,
    disp: *mut IADLXDisplay,
) -> Result<(), String> {
    let mut gamma: *mut IADLXDisplayGamma = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetGamma)(disp_svc, disp, &mut gamma);
    if rc != ADLX_OK || gamma.is_null() {
        return Err(format!("ADLX GetGamma failed (rc={rc})"));
    }
    let gv = &*(*gamma).pVtbl;
    let rc = (gv.ResetGammaRamp)(gamma);
    (gv.Release)(gamma);
    if rc == ADLX_OK {
        Ok(())
    } else {
        Err(format!("ADLX ResetGammaRamp failed (rc={rc})"))
    }
}

unsafe fn read_gamma_ramp_on(
    disp_svc: *mut IADLXDisplayServices,
    disp: *mut IADLXDisplay,
) -> Result<[u16; 768], String> {
    let mut gamma: *mut IADLXDisplayGamma = std::ptr::null_mut();
    let rc = ((*(*disp_svc).pVtbl).GetGamma)(disp_svc, disp, &mut gamma);
    if rc != ADLX_OK || gamma.is_null() {
        return Err(format!("ADLX GetGamma failed (rc={rc})"));
    }
    let gv = &*(*gamma).pVtbl;
    let mut ramp = ADLX_GammaRamp { gamma: [0u16; 768] };
    let rc = (gv.GetGammaRamp)(gamma, &mut ramp);
    (gv.Release)(gamma);
    if rc == ADLX_OK {
        Ok(ramp.gamma)
    } else {
        Err(format!("ADLX GetGammaRamp failed (rc={rc})"))
    }
}

/// Real hardware `AmdApi` backed by the process-global ADLX session.
pub struct RealAmdApi;

impl RealAmdApi {
    fn with_display<T>(
        unique_id: usize,
        f: impl FnOnce(*mut IADLXDisplayServices, *mut IADLXDisplay) -> Result<T, String>,
    ) -> Result<T, String> {
        let session = get_adlx_session().ok_or_else(|| "AMD ADLX unavailable".to_string())?;
        let guard = session
            .lock()
            .map_err(|e| format!("ADLX mutex poisoned: {e}"))?;
        unsafe { with_display_on(guard.system, unique_id, f) }
    }
}

impl AmdApi for RealAmdApi {
    fn enumerate_displays(&self) -> Result<Vec<AmdDisplayInfo>, String> {
        let session = get_adlx_session().ok_or_else(|| "AMD ADLX unavailable".to_string())?;
        let guard = session
            .lock()
            .map_err(|e| format!("ADLX mutex poisoned: {e}"))?;
        unsafe { enumerate_displays_on(guard.system) }
    }

    fn read_color_state(&self, unique_id: usize) -> Result<AmdColorState, String> {
        Self::with_display(unique_id, |s, d| unsafe { read_color_state_on(s, d) })
    }

    fn write_custom_color(
        &self,
        unique_id: usize,
        write: &AmdCustomColorWrite,
    ) -> Result<(), String> {
        Self::with_display(unique_id, |s, d| unsafe { write_custom_color_on(s, d, write) })
    }

    fn write_regamma_ramp(&self, unique_id: usize, ramp: &[u16; 768]) -> Result<(), String> {
        Self::with_display(unique_id, |s, d| unsafe { write_regamma_ramp_on(s, d, ramp) })
    }

    fn reset_gamma_ramp(&self, unique_id: usize) -> Result<(), String> {
        Self::with_display(unique_id, |s, d| unsafe { reset_gamma_ramp_on(s, d) })
    }

    fn read_gamma_ramp(&self, unique_id: usize) -> Result<[u16; 768], String> {
        Self::with_display(unique_id, |s, d| unsafe { read_gamma_ramp_on(s, d) })
    }
}

// ── Correlation & negative-hit TTL cache ────────────────────────────────────

const NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(30);
static NEGATIVE_CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

/// Uncached correlation check used by `BackendRegistry::select` (stays hot-plug accurate).
pub fn correlates_to_amd(edid_id: &str) -> bool {
    let Ok(displays) = RealAmdApi.enumerate_displays() else {
        return false;
    };
    resolve_unique_display(&displays, edid_id).is_ok()
}

/// TTL-cached correlation check used by `enforce::RealStateReader` so non-AMD
/// systems never re-enumerate on every 10 s drift cycle.
pub fn correlates_to_amd_cached(edid_id: &str) -> bool {
    let cache = NEGATIVE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(&since) = map.get(edid_id) {
            if since.elapsed() < NEGATIVE_CACHE_TTL {
                return false;
            }
        }
    }
    let matched = correlates_to_amd(edid_id);
    if !matched {
        if let Ok(mut map) = cache.lock() {
            map.insert(edid_id.to_string(), Instant::now());
        }
    }
    matched
}

// ── Live probe & drift helpers (used by color.rs & enforce.rs) ──────────────

/// Return `(peak_0_to_1, is_default_curve)` for `probe_monitor_cmd` on an AMD display.
pub fn current_gamma_summary(edid_id: &str) -> Result<(f64, bool), String> {
    gamma_summary_with(&RealAmdApi, edid_id)
}

fn gamma_summary_with(amd: &dyn AmdApi, edid_id: &str) -> Result<(f64, bool), String> {
    let displays = amd.enumerate_displays()?;
    let disp = resolve_unique_display(&displays, edid_id)?;
    let state = amd.read_color_state(disp.unique_id)?;
    if !state.custom_regamma_active {
        return Ok((1.0, true));
    }
    let ramp = amd.read_gamma_ramp(disp.unique_id)?;
    let peak = ramp.iter().max().copied().unwrap_or(0) as f64 / 65535.0;
    let neutral = build_regamma_ramp(1.0, [1.0, 1.0, 1.0]);
    Ok((peak, ramp == neutral))
}

/// Check whether a live AMD display has drifted away from `preset`.
pub fn is_drifted_live(edid_id: &str, preset: &Preset) -> bool {
    is_drifted_with(&RealAmdApi, edid_id, preset)
}

pub fn is_drifted_with(amd: &dyn AmdApi, edid_id: &str, preset: &Preset) -> bool {
    let Ok(displays) = amd.enumerate_displays() else {
        return false;
    };
    let Ok(disp) = resolve_unique_display(&displays, edid_id) else {
        return false;
    };
    let Ok(state) = amd.read_color_state(disp.unique_id) else {
        return true;
    };
    if state.hdr_active {
        // Never fight Windows HDR mode in the background loop.
        return false;
    }

    if state.brightness_supported {
        let want = map_ui_0_100_to_range(preset.brightness, state.brightness_range);
        if (state.brightness - want).abs() > state.brightness_range.step.max(1) {
            return true;
        }
    }
    if state.contrast_supported {
        let want = map_ui_0_100_to_range(preset.contrast, state.contrast_range);
        if (state.contrast - want).abs() > state.contrast_range.step.max(1) {
            return true;
        }
    }
    if state.saturation_supported {
        let want = map_ui_0_100_to_range(preset.saturation, state.saturation_range);
        if (state.saturation - want).abs() > state.saturation_range.step.max(1) {
            return true;
        }
    }
    if state.hue_supported {
        let want = map_hue_deg_to_range(preset.hue_deg, state.hue_range);
        if (state.hue - want).abs() > state.hue_range.step.max(1) {
            return true;
        }
    }

    if state.gamma_available {
        let want_neutral = is_neutral_gamma_and_gains(preset);
        if want_neutral {
            if state.custom_regamma_active {
                return true;
            }
        } else {
            if !state.custom_regamma_active {
                return true;
            }
            if let Ok(live) = amd.read_gamma_ramp(disp.unique_id) {
                let want = build_regamma_ramp(preset.gamma, preset.rgb_gains);
                if live != want {
                    return true;
                }
            }
        }
    }

    false
}

// ── AmdBackend (implements ColorBackend) ────────────────────────────────────

const HDR_REASON: &str = "HDR is active on this display — switch to SDR to apply presets";
const VIBRANCE_REASON: &str = "AMD ADLX uses Saturation instead of NVIDIA Digital Vibrance";

pub struct AmdBackend {
    api: Arc<dyn AmdApi>,
}

impl AmdBackend {
    pub fn new() -> Self {
        Self {
            api: Arc::new(RealAmdApi),
        }
    }

    #[cfg(test)]
    pub fn with_api(api: Arc<dyn AmdApi>) -> Self {
        Self { api }
    }

    /// Nominal capability table for a healthy SDR AMD ADLX display (used by
    /// `capabilities_for_backend("amd", _)` in hermetic tests).
    pub fn nominal_probe() -> Vec<AdjustmentCapability> {
        vec![
            AdjustmentCapability {
                adjustment: ADJUSTMENT_GAMMA,
                cap: AdjustmentCap::Supported,
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_BRIGHTNESS,
                cap: AdjustmentCap::Supported,
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_CONTRAST,
                cap: AdjustmentCap::Supported,
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_RGB_GAINS,
                cap: AdjustmentCap::Supported,
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_SATURATION,
                cap: AdjustmentCap::Supported,
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_VIBRANCE,
                cap: AdjustmentCap::Unsupported(VIBRANCE_REASON),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_HUE,
                cap: AdjustmentCap::Supported,
            },
        ]
    }

    fn all_unsupported(reason: &'static str) -> Vec<AdjustmentCapability> {
        [
            ADJUSTMENT_GAMMA,
            ADJUSTMENT_BRIGHTNESS,
            ADJUSTMENT_CONTRAST,
            ADJUSTMENT_RGB_GAINS,
            ADJUSTMENT_SATURATION,
            ADJUSTMENT_VIBRANCE,
            ADJUSTMENT_HUE,
        ]
        .into_iter()
        .map(|adjustment| AdjustmentCapability {
            adjustment,
            cap: AdjustmentCap::Unsupported(reason),
        })
        .collect()
    }
}

impl ColorBackend for AmdBackend {
    fn id(&self) -> &'static str {
        "amd"
    }

    fn probe(&self, edid_id: &str) -> Vec<AdjustmentCapability> {
        let displays = match self.api.enumerate_displays() {
            Ok(d) => d,
            Err(_) => return Self::all_unsupported("AMD ADLX driver unavailable"),
        };
        let disp = match resolve_unique_display(&displays, edid_id) {
            Ok(d) => d,
            Err(_) => {
                return Self::all_unsupported(
                    "Display could not be uniquely correlated to an AMD ADLX output",
                );
            }
        };
        let state = match self.api.read_color_state(disp.unique_id) {
            Ok(s) => s,
            Err(_) => return Self::all_unsupported("AMD ADLX failed to read display capabilities"),
        };
        if state.hdr_active {
            return Self::all_unsupported(HDR_REASON);
        }

        let cap = |ok: bool, msg: &'static str| {
            if ok {
                AdjustmentCap::Supported
            } else {
                AdjustmentCap::Unsupported(msg)
            }
        };

        vec![
            AdjustmentCapability {
                adjustment: ADJUSTMENT_GAMMA,
                cap: cap(state.gamma_available, "ADLX gamma control unavailable on this display"),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_BRIGHTNESS,
                cap: cap(
                    state.brightness_supported,
                    "ADLX reports brightness unsupported on this display",
                ),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_CONTRAST,
                cap: cap(
                    state.contrast_supported,
                    "ADLX reports contrast unsupported on this display",
                ),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_RGB_GAINS,
                cap: cap(
                    state.gamma_available,
                    "ADLX gamma LUT unavailable for RGB gain scaling",
                ),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_SATURATION,
                cap: cap(
                    state.saturation_supported,
                    "ADLX reports saturation unsupported on this display",
                ),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_VIBRANCE,
                cap: AdjustmentCap::Unsupported(VIBRANCE_REASON),
            },
            AdjustmentCapability {
                adjustment: ADJUSTMENT_HUE,
                cap: cap(
                    state.hue_supported,
                    "ADLX reports hue unsupported on this display",
                ),
            },
        ]
    }

    fn apply(
        &self,
        api: &dyn ColorApi,
        _nv: &dyn NvColorApi,
        preset: &Preset,
        target_edid: &str,
    ) -> ApplyResult {
        let mut gamma_applied = false;
        let mut error: Option<String> = None;

        // Stage purity: purge OS GDI gamma ramp to identity first so it never
        // stacks on top of ADLX's re-gamma LUT.
        if let Some(e) = gdi_identity_purge(api, target_edid) {
            append_error(&mut error, e);
        }

        let displays = match self.api.enumerate_displays() {
            Ok(d) => d,
            Err(e) => {
                append_error(&mut error, e);
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };
        let disp = match resolve_unique_display(&displays, target_edid) {
            Ok(d) => d,
            Err(e) => {
                append_error(&mut error, e);
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };
        let state = match self.api.read_color_state(disp.unique_id) {
            Ok(s) => s,
            Err(e) => {
                append_error(&mut error, e);
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };
        if state.hdr_active {
            append_error(&mut error, HDR_REASON.to_string());
            return ApplyResult {
                icc_applied: false,
                gamma_applied: false,
                vibrance_applied: false,
                error,
            };
        }

        // 1. Custom Color (brightness, contrast, saturation, hue)
        let write = AmdCustomColorWrite {
            brightness: state
                .brightness_supported
                .then(|| map_ui_0_100_to_range(preset.brightness, state.brightness_range)),
            contrast: state
                .contrast_supported
                .then(|| map_ui_0_100_to_range(preset.contrast, state.contrast_range)),
            saturation: state
                .saturation_supported
                .then(|| map_ui_0_100_to_range(preset.saturation, state.saturation_range)),
            hue: state
                .hue_supported
                .then(|| map_hue_deg_to_range(preset.hue_deg, state.hue_range)),
        };

        let mut custom_ok = false;
        if write.brightness.is_some()
            || write.contrast.is_some()
            || write.saturation.is_some()
            || write.hue.is_some()
        {
            match self.api.write_custom_color(disp.unique_id, &write) {
                Ok(()) => custom_ok = true,
                Err(e) => append_error(&mut error, format!("amd custom color: {e}")),
            }
        }

        // 2. Gamma + RGB gains via ADLX re-gamma LUT
        if state.gamma_available {
            let gamma_res = if is_neutral_gamma_and_gains(preset) {
                self.api.reset_gamma_ramp(disp.unique_id)
            } else {
                let ramp = build_regamma_ramp(preset.gamma, preset.rgb_gains);
                self.api.write_regamma_ramp(disp.unique_id, &ramp)
            };
            match gamma_res {
                Ok(()) => gamma_applied = true,
                Err(e) => append_error(&mut error, format!("amd gamma: {e}")),
            }
        } else if custom_ok {
            gamma_applied = true;
        }

        ApplyResult {
            icc_applied: false,
            gamma_applied,
            vibrance_applied: false,
            error,
        }
    }

    fn restore(&self, api: &dyn ColorApi, _nv: &dyn NvColorApi, edid_id: &str) -> ApplyResult {
        let mut gamma_applied = false;
        let mut error: Option<String> = None;

        if let Some(e) = gdi_identity_purge(api, edid_id) {
            append_error(&mut error, e);
        }

        let displays = match self.api.enumerate_displays() {
            Ok(d) => d,
            Err(e) => {
                append_error(&mut error, format!("reset: {e}"));
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };
        let disp = match resolve_unique_display(&displays, edid_id) {
            Ok(d) => d,
            Err(e) => {
                append_error(&mut error, format!("reset: {e}"));
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };
        let state = match self.api.read_color_state(disp.unique_id) {
            Ok(s) => s,
            Err(e) => {
                append_error(&mut error, format!("reset: {e}"));
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied: false,
                    vibrance_applied: false,
                    error,
                };
            }
        };

        let neutral_write = AmdCustomColorWrite {
            brightness: state
                .brightness_supported
                .then(|| range_midpoint(state.brightness_range)),
            contrast: state
                .contrast_supported
                .then(|| range_midpoint(state.contrast_range)),
            saturation: state
                .saturation_supported
                .then(|| range_midpoint(state.saturation_range)),
            hue: state
                .hue_supported
                .then(|| range_midpoint(state.hue_range)),
        };

        if neutral_write.brightness.is_some()
            || neutral_write.contrast.is_some()
            || neutral_write.saturation.is_some()
            || neutral_write.hue.is_some()
        {
            if let Err(e) = self.api.write_custom_color(disp.unique_id, &neutral_write) {
                append_error(&mut error, format!("reset custom color: {e}"));
            }
        }

        if state.gamma_available {
            match self.api.reset_gamma_ramp(disp.unique_id) {
                Ok(()) => gamma_applied = true,
                Err(e) => append_error(&mut error, format!("reset gamma: {e}")),
            }
        } else {
            gamma_applied = true;
        }

        // Verify by read-back so a stuck panel never reports false success.
        if let Ok(after) = self.api.read_color_state(disp.unique_id) {
            if let Some(want) = neutral_write.brightness {
                if (after.brightness - want).abs() > state.brightness_range.step.max(1) {
                    append_error(
                        &mut error,
                        format!(
                            "reset: ADLX brightness still {} (expected {want})",
                            after.brightness
                        ),
                    );
                }
            }
            if let Some(want) = neutral_write.saturation {
                if (after.saturation - want).abs() > state.saturation_range.step.max(1) {
                    append_error(
                        &mut error,
                        format!(
                            "reset: ADLX saturation still {} (expected {want})",
                            after.saturation
                        ),
                    );
                }
            }
            if state.gamma_available && after.custom_regamma_active {
                append_error(
                    &mut error,
                    "reset: ADLX reports custom re-gamma ramp still active after ResetGammaRamp"
                        .into(),
                );
            }
        }

        super::verify_gdi_identity(api, edid_id, &mut error);

        ApplyResult {
            icc_applied: false,
            gamma_applied: gamma_applied && error.is_none(),
            vibrance_applied: false,
            error,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::TestRecorder;
    use crate::nvapi::MockNvapi;

    struct MockAmd {
        displays: Vec<AmdDisplayInfo>,
        state: Mutex<AmdColorState>,
        ramp: Mutex<[u16; 768]>,
        last_write: Mutex<Option<AmdCustomColorWrite>>,
        reset_gamma_calls: Mutex<usize>,
        ignore_writes: bool,
    }

    impl MockAmd {
        fn healthy(edid_id: &str) -> Self {
            Self {
                displays: vec![AmdDisplayInfo {
                    unique_id: 42,
                    name: "AMD Ryzen Internal Panel".into(),
                    edid_id: edid_id.into(),
                }],
                state: Mutex::new(AmdColorState {
                    hdr_active: false,
                    brightness_supported: true,
                    brightness_range: AdlxIntRange {
                        min_value: -100,
                        max_value: 100,
                        step: 1,
                    },
                    brightness: 0,
                    contrast_supported: true,
                    contrast_range: AdlxIntRange {
                        min_value: 0,
                        max_value: 200,
                        step: 1,
                    },
                    contrast: 100,
                    saturation_supported: true,
                    saturation_range: AdlxIntRange {
                        min_value: 0,
                        max_value: 200,
                        step: 1,
                    },
                    saturation: 100,
                    hue_supported: true,
                    hue_range: AdlxIntRange {
                        min_value: -30,
                        max_value: 30,
                        step: 1,
                    },
                    hue: 0,
                    gamma_available: true,
                    custom_regamma_active: false,
                }),
                ramp: Mutex::new(build_regamma_ramp(1.0, [1.0, 1.0, 1.0])),
                last_write: Mutex::new(None),
                reset_gamma_calls: Mutex::new(0),
                ignore_writes: false,
            }
        }
    }

    impl AmdApi for MockAmd {
        fn enumerate_displays(&self) -> Result<Vec<AmdDisplayInfo>, String> {
            Ok(self.displays.clone())
        }
        fn read_color_state(&self, _unique_id: usize) -> Result<AmdColorState, String> {
            Ok(self.state.lock().unwrap().clone())
        }
        fn write_custom_color(
            &self,
            _unique_id: usize,
            write: &AmdCustomColorWrite,
        ) -> Result<(), String> {
            *self.last_write.lock().unwrap() = Some(write.clone());
            if !self.ignore_writes {
                let mut s = self.state.lock().unwrap();
                if let Some(b) = write.brightness {
                    s.brightness = b;
                }
                if let Some(c) = write.contrast {
                    s.contrast = c;
                }
                if let Some(sat) = write.saturation {
                    s.saturation = sat;
                }
                if let Some(h) = write.hue {
                    s.hue = h;
                }
            }
            Ok(())
        }
        fn write_regamma_ramp(&self, _unique_id: usize, ramp: &[u16; 768]) -> Result<(), String> {
            if !self.ignore_writes {
                *self.ramp.lock().unwrap() = *ramp;
                self.state.lock().unwrap().custom_regamma_active = true;
            }
            Ok(())
        }
        fn reset_gamma_ramp(&self, _unique_id: usize) -> Result<(), String> {
            *self.reset_gamma_calls.lock().unwrap() += 1;
            if !self.ignore_writes {
                *self.ramp.lock().unwrap() = build_regamma_ramp(1.0, [1.0, 1.0, 1.0]);
                self.state.lock().unwrap().custom_regamma_active = false;
            }
            Ok(())
        }
        fn read_gamma_ramp(&self, _unique_id: usize) -> Result<[u16; 768], String> {
            Ok(*self.ramp.lock().unwrap())
        }
    }

    fn sample_preset() -> Preset {
        Preset {
            id: "p1".into(),
            name: "Warm AMD".into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness: 75.0,
            contrast: 60.0,
            rgb_gains: [1.05, 1.0, 0.9],
            gamma: 1.2,
            vibrance: 50.0,
            saturation: 70.0,
            hue_deg: 330.0, // -30° wrapped
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
            attached_programs: vec![],
            shortcut: None,
            hardware_brightness: None,
        }
    }

    #[test]
    fn abi_struct_sizes_match_adlx_headers() {
        use std::mem::{offset_of, size_of};
        // Verified verbatim against MSVC cl.exe compiling official AMD ADLX SDK C headers.
        assert_eq!(size_of::<AdlxIntRange>(), 12);
        assert_eq!(size_of::<ADLX_GammaRamp>(), 1536);
        assert_eq!(size_of::<ADLX_RegammaCoeff>(), 20);

        assert_eq!(size_of::<IADLXSystemVtbl>(), 96);
        assert_eq!(offset_of!(IADLXSystemVtbl, GetHybridGraphicsType), 0);
        assert_eq!(offset_of!(IADLXSystemVtbl, GetGPUs), 8);
        assert_eq!(offset_of!(IADLXSystemVtbl, QueryInterface), 16);
        assert_eq!(offset_of!(IADLXSystemVtbl, GetDisplaysServices), 24);

        assert_eq!(size_of::<IADLXDisplayServicesVtbl>(), 160);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, Acquire), 0);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, Release), 8);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, GetNumberOfDisplays), 24);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, GetDisplays), 32);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, GetGamma), 56);
        assert_eq!(offset_of!(IADLXDisplayServicesVtbl, GetCustomColor), 128);

        assert_eq!(size_of::<IADLXDisplayListVtbl>(), 104);
        assert_eq!(offset_of!(IADLXDisplayListVtbl, Release), 8);
        assert_eq!(offset_of!(IADLXDisplayListVtbl, Size), 24);
        assert_eq!(offset_of!(IADLXDisplayListVtbl, At_DisplayList), 88);

        assert_eq!(size_of::<IADLXDisplayVtbl>(), 112);
        assert_eq!(offset_of!(IADLXDisplayVtbl, Release), 8);
        assert_eq!(offset_of!(IADLXDisplayVtbl, ManufacturerID), 24);
        assert_eq!(offset_of!(IADLXDisplayVtbl, ConnectorType), 40);
        assert_eq!(offset_of!(IADLXDisplayVtbl, Name), 48);
        assert_eq!(offset_of!(IADLXDisplayVtbl, EDID), 56);
        assert_eq!(offset_of!(IADLXDisplayVtbl, UniqueId), 104);

        assert_eq!(size_of::<IADLXCustomColorVtbl>(), 184);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, Release), 8);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, IsHueSupported), 24);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, SetHue), 48);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, IsSaturationSupported), 56);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, SetSaturation), 80);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, IsBrightnessSupported), 88);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, SetBrightness), 112);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, IsContrastSupported), 120);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, SetContrast), 144);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, IsTemperatureSupported), 152);
        assert_eq!(offset_of!(IADLXCustomColorVtbl, SetTemperature), 176);

        assert_eq!(size_of::<IADLXDisplayGammaVtbl>(), 232);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, Release), 8);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, IsCurrentReGammaRamp), 24);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, GetGammaRamp), 48);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, IsCurrentReGammaSRGB), 104);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, IsCurrentReGammaPQ), 120);
        assert_eq!(
            offset_of!(IADLXDisplayGammaVtbl, IsCurrentReGammaPQ2084Interim),
            128
        );
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, SetReGammaRamp_Memory), 208);
        assert_eq!(offset_of!(IADLXDisplayGammaVtbl, ResetGammaRamp), 224);
    }

    #[test]
    fn edid_id_from_hex_matches_monitor_edid_key() {
        let mut raw = [0u8; 128];
        raw[8] = 0x06; // AUO
        raw[9] = 0xAF;
        raw[10] = 0xAD;
        raw[11] = 0xF7;
        let expected = crate::monitor::edid_id_from_bytes(&raw);
        let hex_str = hex::encode(raw);
        assert_eq!(edid_id_from_hex(&hex_str).as_deref(), Some(expected.as_str()));
        assert_eq!(edid_id_from_hex("too-short"), None);
    }

    #[test]
    fn ui_and_hue_range_mapping_hits_neutral_and_extrema() {
        let b_rng = AdlxIntRange {
            min_value: -100,
            max_value: 100,
            step: 1,
        };
        assert_eq!(map_ui_0_100_to_range(0.0, b_rng), -100);
        assert_eq!(map_ui_0_100_to_range(50.0, b_rng), 0);
        assert_eq!(map_ui_0_100_to_range(75.0, b_rng), 50);
        assert_eq!(map_ui_0_100_to_range(100.0, b_rng), 100);

        let h_rng = AdlxIntRange {
            min_value: -30,
            max_value: 30,
            step: 1,
        };
        assert_eq!(map_hue_deg_to_range(0.0, h_rng), 0);
        assert_eq!(map_hue_deg_to_range(90.0, h_rng), 15);
        assert_eq!(map_hue_deg_to_range(180.0, h_rng), 30);
        assert_eq!(map_hue_deg_to_range(270.0, h_rng), -15);
        assert_eq!(map_hue_deg_to_range(330.0, h_rng), -5);
    }

    #[test]
    fn regamma_ramp_respects_gamma_direction_and_rgb_gains() {
        let neutral = build_regamma_ramp(1.0, [1.0, 1.0, 1.0]);
        let lifted = build_regamma_ramp(1.5, [1.0, 1.0, 1.0]);
        let darkened = build_regamma_ramp(0.7, [1.0, 1.0, 1.0]);
        assert!(lifted[128] > neutral[128], "gamma > 1.0 should lift midtones");
        assert!(darkened[128] < neutral[128], "gamma < 1.0 should darken midtones");

        let tinted = build_regamma_ramp(1.0, [1.0, 1.0, 0.5]);
        assert_eq!(tinted[255], 65535);
        assert!(tinted[2 * 256 + 255] <= 32768);
    }

    #[test]
    fn probe_reports_healthy_and_hdr_verdicts_honestly() {
        let mock = Arc::new(MockAmd::healthy("AMD-PANEL-1"));
        let backend = AmdBackend::with_api(mock.clone());

        let caps = backend.probe("AMD-PANEL-1");
        let get = |name: &str| caps.iter().find(|c| c.adjustment == name).unwrap().cap;
        assert_eq!(get(ADJUSTMENT_GAMMA), AdjustmentCap::Supported);
        assert_eq!(get(ADJUSTMENT_SATURATION), AdjustmentCap::Supported);
        assert_eq!(get(ADJUSTMENT_HUE), AdjustmentCap::Supported);
        assert!(matches!(get(ADJUSTMENT_VIBRANCE), AdjustmentCap::Unsupported(_)));

        // HDR active flips every adjustment to Unsupported with HDR reason.
        mock.state.lock().unwrap().hdr_active = true;
        let hdr_caps = backend.probe("AMD-PANEL-1");
        for cap in hdr_caps {
            assert!(
                matches!(cap.cap, AdjustmentCap::Unsupported(msg) if msg.contains("HDR")),
                "{cap:?}"
            );
        }
    }

    #[test]
    fn apply_restore_and_drift_detection_roundtrip() {
        let mock = Arc::new(MockAmd::healthy("AMD-PANEL-1"));
        let backend = AmdBackend::with_api(mock.clone());
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let preset = sample_preset();

        // Apply non-neutral preset
        let res = backend.apply(&api, &nv, &preset, "AMD-PANEL-1");
        assert!(res.error.is_none(), "{res:?}");
        assert!(res.gamma_applied);
        assert!(!is_drifted_with(mock.as_ref(), "AMD-PANEL-1", &preset));

        // External clobber of saturation is detected as drift
        mock.state.lock().unwrap().saturation = 100;
        assert!(is_drifted_with(mock.as_ref(), "AMD-PANEL-1", &preset));

        // Neutral gamma + gains calls ResetGammaRamp instead of custom LUT
        let mut neutral_g = preset.clone();
        neutral_g.gamma = 1.0;
        neutral_g.rgb_gains = [1.0, 1.0, 1.0];
        let res2 = backend.apply(&api, &nv, &neutral_g, "AMD-PANEL-1");
        assert!(res2.error.is_none());
        assert_eq!(*mock.reset_gamma_calls.lock().unwrap(), 1);
        assert!(!mock.state.lock().unwrap().custom_regamma_active);

        // Restore resets custom color to neutral midpoints and resets gamma
        let rest = backend.restore(&api, &nv, "AMD-PANEL-1");
        assert!(rest.error.is_none(), "{rest:?}");
        assert!(rest.gamma_applied);
        let s = mock.state.lock().unwrap().clone();
        assert_eq!(s.brightness, 0);
        assert_eq!(s.contrast, 100);
        assert_eq!(s.saturation, 100);
        assert_eq!(s.hue, 0);
        assert!(!s.custom_regamma_active);
    }

    #[test]
    fn restore_reports_error_when_readback_remains_stuck() {
        let mut raw = MockAmd::healthy("AMD-PANEL-1");
        raw.state.lock().unwrap().saturation = 180;
        raw.ignore_writes = true;
        let mock = Arc::new(raw);
        let backend = AmdBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let rest = backend.restore(&api, &nv, "AMD-PANEL-1");
        assert!(!rest.gamma_applied);
        assert!(
            rest.error
                .as_deref()
                .unwrap_or_default()
                .contains("saturation still 180"),
            "{rest:?}"
        );
    }

    struct LoadedDllAmdApi(Mutex<AdlxRuntime>);

    impl AmdApi for LoadedDllAmdApi {
        fn enumerate_displays(&self) -> Result<Vec<AmdDisplayInfo>, String> {
            let g = self.0.lock().unwrap();
            unsafe { enumerate_displays_on(g.system) }
        }
        fn read_color_state(&self, unique_id: usize) -> Result<AmdColorState, String> {
            let g = self.0.lock().unwrap();
            unsafe { with_display_on(g.system, unique_id, |s, d| read_color_state_on(s, d)) }
        }
        fn write_custom_color(
            &self,
            unique_id: usize,
            write: &AmdCustomColorWrite,
        ) -> Result<(), String> {
            let g = self.0.lock().unwrap();
            unsafe {
                with_display_on(g.system, unique_id, |s, d| {
                    write_custom_color_on(s, d, write)
                })
            }
        }
        fn write_regamma_ramp(&self, unique_id: usize, ramp: &[u16; 768]) -> Result<(), String> {
            let g = self.0.lock().unwrap();
            unsafe {
                with_display_on(g.system, unique_id, |s, d| write_regamma_ramp_on(s, d, ramp))
            }
        }
        fn reset_gamma_ramp(&self, unique_id: usize) -> Result<(), String> {
            let g = self.0.lock().unwrap();
            unsafe { with_display_on(g.system, unique_id, |s, d| reset_gamma_ramp_on(s, d)) }
        }
        fn read_gamma_ramp(&self, unique_id: usize) -> Result<[u16; 768], String> {
            let g = self.0.lock().unwrap();
            unsafe { with_display_on(g.system, unique_id, |s, d| read_gamma_ramp_on(s, d)) }
        }
    }

    #[test]
    fn real_ffi_roundtrip_against_external_adlx_dll_when_provided() {
        let Ok(dll_path) = std::env::var("CHROMADECK_TEST_ADLX_DLL") else {
            return;
        };
        let rt = AdlxRuntime::open_dll(&dll_path)
            .unwrap_or_else(|| panic!("failed to load test ADLX DLL from {dll_path}"));
        let terminate = rt.terminate;
        let module = rt.module;
        let dll_api = Arc::new(LoadedDllAmdApi(Mutex::new(rt)));
        let backend = AmdBackend::with_api(dll_api.clone());

        let displays = dll_api.enumerate_displays().unwrap();
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].unique_id, 777);
        let edid = displays[0].edid_id.clone();
        assert_eq!(edid, "EDID:06AF-F7AD-00001234");

        // Probe through real C vtables
        let caps = backend.probe(&edid);
        assert_eq!(
            caps.iter()
                .find(|c| c.adjustment == ADJUSTMENT_SATURATION)
                .unwrap()
                .cap,
            AdjustmentCap::Supported
        );

        // Apply non-neutral preset (exercises SetBrightness/Contrast/Saturation/Hue
        // AND by-value 1536-byte ADLX_GammaRamp via SetReGammaRamp_Memory!)
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let preset = sample_preset();
        let res = backend.apply(&api, &nv, &preset, &edid);
        assert!(res.error.is_none(), "{res:?}");
        assert!(res.gamma_applied);

        // Verify all 768 u16 ramp entries survived Rust -> C by-value ABI intact!
        let live_ramp = dll_api.read_gamma_ramp(777).unwrap();
        let expected_ramp = build_regamma_ramp(preset.gamma, preset.rgb_gains);
        assert_eq!(live_ramp, expected_ramp);
        assert!(!is_drifted_with(dll_api.as_ref(), &edid, &preset));

        // Restore resets Custom Color + ResetGammaRamp and passes read-back verification
        let rest = backend.restore(&api, &nv, &edid);
        assert!(rest.error.is_none(), "{rest:?}");
        assert!(rest.gamma_applied);
        let after = dll_api.read_color_state(777).unwrap();
        assert_eq!(after.brightness, 0);
        assert_eq!(after.contrast, 100);
        assert_eq!(after.saturation, 100);
        assert_eq!(after.hue, 0);
        assert!(!after.custom_regamma_active);

        // Drop API and verify ADLXTerminate() == ADLX_OK (0 leaked COM objects!)
        drop(backend);
        drop(dll_api);
        let term_rc = unsafe { terminate() };
        assert_eq!(term_rc, ADLX_OK, "ADLXTerminate reported orphan objects!");
        unsafe {
            let _ = windows::Win32::Foundation::FreeLibrary(module);
        }
    }
}
