// ── Intel IGCL backend: dynamic-load FFI layer (Phase E2) ──────────────────
// Windows-only, x64-only. Loads the Intel Graphics Control Library
// (`ControlLib.dll`, fallback `IntelControlLib.dll`) from the DriverStore at
// runtime. Every absence (no DLL, missing export, init failure, no
// correlation) degrades gracefully — never panic, never guess.
//
// The IGCL C structs below are transcribed from the public Intel SDK header
// `include/igcl_api.h` (https://github.com/intel/drivers.gpu.control-library,
// "Copyright (C) 2025 Intel Corporation", Intel Software License Agreement
// 10.07.21 — header redistribution permitted with notice, "solely for use on
// Intel platforms"). Only the minimal subset actually used is reproduced;
// names are preserved verbatim (including the upstream
// `ctl_pixtx_config_opertaion_type_t` / `CTL_PIXTX_CONFIG_OPERTAION_TYPE_*`
// spelling, and the transcribed `ctl_adapter_display_encoder_properties_t`
// struct). No Intel binaries are reverse-engineered — only exported symbols
// and the public header are used.
// Struct sizes were verified on hardware (see docs/superpowers/specs/
// 2026-10-19-intel-igcl-design.md §10) and are pinned by unit tests below.
//
// ALL `unsafe` lives in this module. Nothing outside it performs FFI or
// dereferences raw pointers.

#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    clippy::missing_safety_doc,
    clippy::missing_transmute_annotations
)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use super::{
    append_error, AdjustmentCap, AdjustmentCapability, ColorBackend, ADJUSTMENT_BRIGHTNESS,
    ADJUSTMENT_CONTRAST, ADJUSTMENT_GAMMA, ADJUSTMENT_HUE, ADJUSTMENT_RGB_GAINS,
    ADJUSTMENT_SATURATION, ADJUSTMENT_VIBRANCE,
};
use crate::color::{gdi_identity_purge, ApplyResult, ColorApi};
use crate::nvapi::NvColorApi;
use crate::store::Preset;

// ── Opaque handle types ─────────────────────────────────────────────────────

type ctl_result_t = u32;
type ctl_api_handle_t = *mut c_void;
type ctl_device_adapter_handle_t = *mut c_void;
type ctl_display_output_handle_t = *mut c_void;

// ── Constants (verbatim upstream names) ─────────────────────────────────────

const CTL_RESULT_SUCCESS: ctl_result_t = 0x0000_0000;

const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE: u32 = 1 << 0;
const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED: u32 = 1 << 1;

/// `ctl_standard_display_feature_flag_t` HDR bit (panel supports HDR).
const CTL_STD_DISPLAY_FEATURE_FLAG_HDR: u32 = 1 << 5;

const CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY: u32 = 0;
const CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT: u32 = 1;

const CTL_PIXTX_BLOCK_TYPE_1D_LUT: u32 = 1;
const CTL_PIXTX_BLOCK_TYPE_3D_LUT: u32 = 2;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX: u32 = 3;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS: u32 = 4;

const CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT: u32 = 1; // note "OPERTAION" typo
const CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM: u32 = 2;

/// `ctl_operation_type_t` READ, used by `ctlPanelDescriptorAccess`.
const CTL_OPERATION_TYPE_READ: u32 = 1;

const fn ctl_make_version(major: u32, minor: u32) -> u32 {
    (major << 16) | (minor & 0x0000_ffff)
}

// ── Transcribed structs (verbatim from spec §10) ────────────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_application_id_t {
    Data1: u32,
    Data2: u16,
    Data3: u16,
    Data4: [u8; 8],
}

#[repr(C)]
struct ctl_init_args_t {
    Size: u32,
    Version: u8,
    AppVersion: u32,
    flags: u32,
    SupportedVersion: u32,
    ApplicationUID: ctl_application_id_t,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_firmware_version_t {
    major_version: u64,
    minor_version: u64,
    build_number: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_adapter_bdf_t {
    bus: u8,
    device: u8,
    function: u8,
}

#[repr(C)]
struct ctl_device_adapter_properties_t {
    Size: u32,
    Version: u8,
    pDeviceID: *mut c_void,
    device_id_size: u32,
    device_type: u32,
    supported_subfunction_flags: u32,
    driver_version: u64,
    firmware_version: ctl_firmware_version_t,
    pci_vendor_id: u32,
    pci_device_id: u32,
    rev_id: u32,
    num_eus_per_sub_slice: u32,
    num_sub_slices_per_slice: u32,
    num_slices: u32,
    name: [i8; 100],
    graphics_adapter_properties: u32,
    Frequency: u32,
    pci_subsys_id: u16,
    pci_subsys_vendor_id: u16,
    adapter_bdf: ctl_adapter_bdf_t,
    num_xe_cores: u32,
    reserved: [i8; 108],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_generic_void_datatype_t {
    pData: *mut c_void,
    size: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
union ctl_os_display_encoder_identifier_t {
    WindowsDisplayEncoderID: u32,
    DisplayEncoderID: ctl_generic_void_datatype_t,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_revision_datatype_t {
    major_version: u8,
    minor_version: u8,
    revision_version: u8,
}

#[repr(C)]
struct ctl_display_timing_t {
    Size: u32,
    Version: u8,
    PixelClock: u64,
    HActive: u32,
    VActive: u32,
    HTotal: u32,
    VTotal: u32,
    HBlank: u32,
    VBlank: u32,
    HSync: u32,
    VSync: u32,
    RefreshRate: f32,
    SignalStandard: u32,
    VicId: u8,
}

#[repr(C)]
struct ctl_display_properties_t {
    Size: u32,
    Version: u8,
    Os_display_encoder_handle: ctl_os_display_encoder_identifier_t,
    Type: u32,
    AttachedDisplayMuxType: u32,
    ProtocolConverterOutput: u32,
    SupportedSpec: ctl_revision_datatype_t,
    SupportedOutputBPCFlags: u32,
    ProtocolConverterType: u32,
    DisplayConfigFlags: u32,
    FeatureEnabledFlags: u32,
    FeatureSupportedFlags: u32,
    AdvancedFeatureEnabledFlags: u32,
    AdvancedFeatureSupportedFlags: u32,
    Display_Timing_Info: ctl_display_timing_t,
    ReservedFields: [u32; 16],
}

#[repr(C)]
struct ctl_adapter_display_encoder_properties_t {
    Size: u32,
    Version: u8,
    Os_display_encoder_handle: ctl_os_display_encoder_identifier_t,
    Type: u32,
    IsOnBoardProtocolConverterOutputPresent: bool,
    SupportedSpec: ctl_revision_datatype_t,
    SupportedOutputBPCFlags: u32,
    EncoderConfigFlags: u32,
    FeatureSupportedFlags: u32,
    AdvancedFeatureSupportedFlags: u32,
    ReservedFields: [u32; 16],
}

#[repr(C)]
struct ctl_edid_management_args_t {
    Size: u32,
    Version: u8,
    OpType: u32,
    EdidType: u32,
    EdidSize: u32,
    pEdidBuf: *mut u8,
    OutFlags: u32,
}

#[repr(C)]
struct ctl_panel_descriptor_access_args_t {
    Size: u32,
    Version: u8,
    OpType: u32, // ctl_operation_type_t: READ = 1
    BlockNumber: u32,
    DescriptorDataSize: u32,
    pDescriptorData: *mut u8,
}

#[repr(C)]
struct ctl_display_feature_reset_t {
    Size: u32,
    Version: u8,
    ResetFeature: u32,
    ResetFeatureStatus: u32,
}

// ── pixtx ───────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_pixtx_color_primaries_t {
    Size: u32,
    Version: u8,
    xR: f64,
    yR: f64,
    xG: f64,
    yG: f64,
    xB: f64,
    yB: f64,
    xW: f64,
    yW: f64,
}

#[repr(C)]
struct ctl_pixtx_pixel_format_t {
    Size: u32,
    Version: u8,
    BitsPerColor: u32,
    IsFloat: bool,
    EncodingType: u32,
    ColorSpace: u32,
    ColorModel: u32,
    ColorPrimaries: ctl_pixtx_color_primaries_t,
    MaxBrightness: f64,
    MinBrightness: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_pixtx_1dlut_config_t {
    Size: u32,
    Version: u8,
    SamplingType: u32,
    NumSamplesPerChannel: u32,
    NumChannels: u32,
    pSampleValues: *mut f64,
    pSamplePositions: *mut f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_pixtx_matrix_config_t {
    Size: u32,
    Version: u8,
    PreOffsets: [f64; 3],
    PostOffsets: [f64; 3],
    Matrix: [[f64; 3]; 3],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_pixtx_3dlut_sample_t {
    Red: f64,
    Green: f64,
    Blue: f64,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct ctl_pixtx_3dlut_config_t {
    Size: u32,
    Version: u8,
    NumSamplesPerChannel: u32,
    pSampleValues: *mut ctl_pixtx_3dlut_sample_t,
}

#[repr(C)]
union ctl_pixtx_config_t {
    OneDLutConfig: ctl_pixtx_1dlut_config_t,
    ThreeDLutConfig: ctl_pixtx_3dlut_config_t,
    MatrixConfig: ctl_pixtx_matrix_config_t,
}

#[repr(C)]
struct ctl_pixtx_block_config_t {
    Size: u32,
    Version: u8,
    BlockId: u32,
    BlockType: u32,
    Config: ctl_pixtx_config_t,
}

#[repr(C)]
struct ctl_pixtx_pipe_get_config_t {
    Size: u32,
    Version: u8,
    QueryType: u32,
    InputPixelFormat: ctl_pixtx_pixel_format_t,
    OutputPixelFormat: ctl_pixtx_pixel_format_t,
    NumBlocks: u32,
    pBlockConfigs: *mut ctl_pixtx_block_config_t,
}

#[repr(C)]
struct ctl_pixtx_pipe_set_config_t {
    Size: u32,
    Version: u8,
    OpertaionType: u32,
    Flags: u32,
    NumBlocks: u32,
    pBlockConfigs: *mut ctl_pixtx_block_config_t,
}

// ── Function pointer types (x64: "system" == "C" calling convention) ────────

type FnCtlInit = unsafe extern "system" fn(*mut ctl_init_args_t, *mut ctl_api_handle_t) -> ctl_result_t;
type FnCtlClose = unsafe extern "system" fn(ctl_api_handle_t) -> ctl_result_t;
type FnCtlEnumerateDevices =
    unsafe extern "system" fn(ctl_api_handle_t, *mut u32, *mut ctl_device_adapter_handle_t) -> ctl_result_t;
type FnCtlEnumerateDisplayOutputs =
    unsafe extern "system" fn(ctl_device_adapter_handle_t, *mut u32, *mut ctl_display_output_handle_t) -> ctl_result_t;
type FnCtlGetDisplayProperties =
    unsafe extern "system" fn(ctl_display_output_handle_t, *mut ctl_display_properties_t) -> ctl_result_t;
type FnCtlPixelTransformationGetConfig =
    unsafe extern "system" fn(ctl_display_output_handle_t, *mut ctl_pixtx_pipe_get_config_t) -> ctl_result_t;
type FnCtlPixelTransformationSetConfig =
    unsafe extern "system" fn(ctl_display_output_handle_t, *mut ctl_pixtx_pipe_set_config_t) -> ctl_result_t;
type FnCtlPanelDescriptorAccess =
    unsafe extern "system" fn(ctl_display_output_handle_t, *mut ctl_panel_descriptor_access_args_t) -> ctl_result_t;

// ── Resolved function-pointer table ─────────────────────────────────────────

/// Resolved IGCL entry points, held for the process lifetime alongside the
/// library handle (never freed, mirroring `nvapi::NvapiFns`).
struct IgclFns {
    _lib: HMODULE,
    init: FnCtlInit,
    close: FnCtlClose,
    enumerate_devices: FnCtlEnumerateDevices,
    enumerate_display_outputs: FnCtlEnumerateDisplayOutputs,
    get_display_properties: FnCtlGetDisplayProperties,
    pixtx_get_config: FnCtlPixelTransformationGetConfig,
    pixtx_set_config: FnCtlPixelTransformationSetConfig,
    panel_descriptor_access: FnCtlPanelDescriptorAccess,
}

/// An initialized IGCL session: the API handle plus the enumerated device
/// adapters. Output handles are re-enumerated per call (hot-plug safe); the
/// raw handles remain valid for the process lifetime because the library is
/// never unloaded and the API handle is never closed.
struct IgclSession {
    fns: IgclFns,
    api: ctl_api_handle_t,
    devices: Vec<ctl_device_adapter_handle_t>,
}

// SAFETY: loaded once; handles/pointers are only ever read (never mutated)
// after construction and the underlying driver objects live for the process
// lifetime (library is never freed, API handle never closed).
unsafe impl Send for IgclSession {}
unsafe impl Sync for IgclSession {}

/// Process-wide IGCL session cell.
///
/// A failed load is retried after a short backoff instead of being cached for
/// the process lifetime: the driver may not be ready during early startup, and
/// a transient LoadLibrary/Init failure must not permanently disable Intel
/// handling. The mutex also makes the load attempt single-flight.
static SESSION: OnceLock<Mutex<SessionCell>> = OnceLock::new();

/// Non-blocking readiness flag. Set exactly once, when a session load
/// succeeds; never cleared (the session is leaked for the process lifetime).
/// `session_loaded` reads this atomically instead of locking `SESSION`, so
/// quit classification (`backend_id_hint`) never waits on an in-flight driver
/// load (L-5).
static SESSION_READY: AtomicBool = AtomicBool::new(false);

enum SessionCell {
    /// No load attempted yet.
    Uninitialized,
    /// Loaded successfully; leaked for the process lifetime.
    Ready(&'static IgclSession),
    /// Last attempt failed; do not retry before this instant.
    RetryAfter(Instant),
}

/// Minimum delay between failed IGCL load attempts.
const SESSION_RETRY_BACKOFF: Duration = Duration::from_secs(5);

/// Whether a retry is due at `now` for a failure recorded with `retry_at`.
fn retry_due(now: Instant, retry_at: Instant) -> bool {
    now >= retry_at
}

/// Zero-initialize a plain C struct (no `Drop`, no references). Used for the
/// size-tagged IGCL structs whose `.Size`/`.Version` fields are set next.
fn zeroed<T>() -> T {
    // SAFETY: only ever instantiated for `#[repr(C)]` POD-style structs with
    // no invalid bit patterns (no `Drop`, no references, no `bool` issues
    // because we overwrite them or they default to `false`).
    unsafe { std::mem::zeroed() }
}

// ── Loading ─────────────────────────────────────────────────────────────────

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Candidate DLL paths, in probe order. The real IGCL runtime DLLs live in the
/// DriverStore alongside their sibling dependencies, so they must be loaded by
/// full path. Bare-name fallbacks come last. `intel_gfx_api-x64.dll` is NOT a
/// candidate (it does not export `ctlInit`).
fn candidate_dll_paths() -> Vec<String> {
    let mut paths = Vec::new();
    let names = ["ControlLib.dll", "IntelControlLib.dll"];
    let repo = std::path::Path::new(r"C:\Windows\System32\DriverStore\FileRepository");
    if let Ok(entries) = std::fs::read_dir(repo) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let dn = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if !dn.starts_with("iigd") {
                continue;
            }
            for n in names {
                let c = p.join(n);
                if c.exists() {
                    paths.push(c.to_string_lossy().to_string());
                }
            }
        }
    }
    paths.push("ControlLib.dll".into());
    paths.push("IntelControlLib.dll".into());
    paths
}

/// Load a library by full path. `None` on any failure.
fn load_lib(path: &str) -> Option<HMODULE> {
    let w = wide(path);
    // SAFETY: LoadLibraryW with a valid wide string.
    let lib = unsafe { LoadLibraryW(PCWSTR::from_raw(w.as_ptr())) };
    match lib {
        Ok(h) if !h.is_invalid() => Some(h),
        _ => None,
    }
}

/// Resolve one exported symbol as a function pointer.
unsafe fn resolve<F>(lib: HMODULE, name: &str) -> Option<F> {
    let mut b = name.as_bytes().to_vec();
    b.push(0);
    let ptr = GetProcAddress(lib, PCSTR::from_raw(b.as_ptr()))?;
    Some(std::mem::transmute_copy(&ptr))
}

/// Resolve the mandatory symbol set. Any missing export → `None`.
unsafe fn build_fns(lib: HMODULE) -> Option<IgclFns> {
    Some(IgclFns {
        _lib: lib,
        init: resolve(lib, "ctlInit")?,
        close: resolve(lib, "ctlClose")?,
        enumerate_devices: resolve(lib, "ctlEnumerateDevices")?,
        enumerate_display_outputs: resolve(lib, "ctlEnumerateDisplayOutputs")?,
        get_display_properties: resolve(lib, "ctlGetDisplayProperties")?,
        pixtx_get_config: resolve(lib, "ctlPixelTransformationGetConfig")?,
        pixtx_set_config: resolve(lib, "ctlPixelTransformationSetConfig")?,
        panel_descriptor_access: resolve(lib, "ctlPanelDescriptorAccess")?,
    })
}

/// Load the IGCL library and resolve the function table. `None` on ANY
/// failure — callers degrade to GDI/NVAPI, never panic.
fn load_igcl() -> Option<IgclFns> {
    for path in candidate_dll_paths() {
        let lib = match load_lib(&path) {
            Some(h) => h,
            None => continue,
        };
        match unsafe { build_fns(lib) } {
            Some(fns) => return Some(fns),
            None => {
                // SAFETY: freeing a library we just loaded is safe.
                unsafe {
                    let _ = FreeLibrary(lib);
                }
            }
        }
    }
    None
}

/// Initialize IGCL and enumerate device adapters. `None` on init failure.
fn load_session() -> Option<IgclSession> {
    let fns = load_igcl()?;

    let mut init = zeroed::<ctl_init_args_t>();
    init.Size = std::mem::size_of::<ctl_init_args_t>() as u32;
    init.Version = 0;
    init.AppVersion = ctl_make_version(1, 1);
    init.flags = 0;
    init.ApplicationUID = ctl_application_id_t {
        Data1: 0,
        Data2: 0,
        Data3: 0,
        Data4: [0; 8],
    };

    let mut api: ctl_api_handle_t = std::ptr::null_mut();
    // SAFETY: init writes the API handle on success; args/sizes per header.
    let r = unsafe { (fns.init)(&mut init, &mut api) };
    if r != CTL_RESULT_SUCCESS || api.is_null() {
        return None;
    }

    let devices = match unsafe { enumerate_devices(&fns, api) } {
        Some(d) => d,
        None => {
            // SAFETY: close the API handle we just opened.
            unsafe {
                let _ = (fns.close)(api);
            }
            return None;
        }
    };

    Some(IgclSession { fns, api, devices })
}

fn session_cell() -> &'static Mutex<SessionCell> {
    SESSION.get_or_init(|| Mutex::new(SessionCell::Uninitialized))
}

/// Lazily-initialized session. `None` when the Intel driver is absent or the
/// last attempt failed and the retry backoff has not elapsed.
fn session() -> Option<&'static IgclSession> {
    let mut cell = session_cell()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match &*cell {
        SessionCell::Ready(s) => return Some(*s),
        SessionCell::RetryAfter(retry_at) if !retry_due(Instant::now(), *retry_at) => return None,
        SessionCell::Uninitialized | SessionCell::RetryAfter(_) => {}
    }

    match load_session() {
        Some(s) => {
            // Leak: the session lives for the process lifetime, like the
            // previous `OnceLock` storage.
            let leaked: &'static IgclSession = Box::leak(Box::new(s));
            *cell = SessionCell::Ready(leaked);
            SESSION_READY.store(true, Ordering::SeqCst);
            Some(leaked)
        }
        None => {
            *cell = SessionCell::RetryAfter(Instant::now() + SESSION_RETRY_BACKOFF);
            None
        }
    }
}

/// Whether the IGCL session is already live. Does NOT attempt a load and does
/// NOT block on an in-flight load — it reads a non-blocking atomic flag, so
/// cheap classification paths (e.g. quit logging) never stall on a driver
/// probe (L-5). Returns `false` while a load is in progress on another thread;
/// that is the intended behaviour for never-stall callers.
pub(crate) fn session_loaded() -> bool {
    SESSION_READY.load(Ordering::SeqCst)
}

// ── Low-level FFI helpers (all `unsafe`, this module only) ──────────────────

unsafe fn enumerate_devices(
    fns: &IgclFns,
    api: ctl_api_handle_t,
) -> Option<Vec<ctl_device_adapter_handle_t>> {
    let mut count: u32 = 0;
    let r = (fns.enumerate_devices)(api, &mut count, std::ptr::null_mut());
    if r != CTL_RESULT_SUCCESS {
        return None;
    }
    if count == 0 {
        return Some(Vec::new());
    }
    let mut devs = vec![std::ptr::null_mut::<c_void>(); count as usize];
    let r = (fns.enumerate_devices)(api, &mut count, devs.as_mut_ptr());
    if r != CTL_RESULT_SUCCESS {
        return None;
    }
    devs.truncate(count as usize);
    Some(devs)
}

unsafe fn enumerate_outputs(
    fns: &IgclFns,
    dev: ctl_device_adapter_handle_t,
) -> Result<Vec<ctl_display_output_handle_t>, String> {
    let mut count: u32 = 0;
    let r = (fns.enumerate_display_outputs)(dev, &mut count, std::ptr::null_mut());
    if r != CTL_RESULT_SUCCESS {
        return Err(format!("ctlEnumerateDisplayOutputs failed ({})", result_str(r)));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut outs = vec![std::ptr::null_mut::<c_void>(); count as usize];
    let r = (fns.enumerate_display_outputs)(dev, &mut count, outs.as_mut_ptr());
    if r != CTL_RESULT_SUCCESS {
        return Err(format!("ctlEnumerateDisplayOutputs failed ({})", result_str(r)));
    }
    outs.truncate(count as usize);
    Ok(outs)
}

unsafe fn display_props(
    fns: &IgclFns,
    out: ctl_display_output_handle_t,
) -> Result<ctl_display_properties_t, String> {
    let mut p = std::mem::zeroed::<ctl_display_properties_t>();
    p.Size = std::mem::size_of::<ctl_display_properties_t>() as u32;
    let r = (fns.get_display_properties)(out, &mut p);
    if r != CTL_RESULT_SUCCESS {
        return Err(format!("ctlGetDisplayProperties failed ({})", result_str(r)));
    }
    Ok(p)
}

/// Read the raw 128-byte base EDID via `ctlPanelDescriptorAccess` (the
/// primary EDID path — `ctlEdidManagement` READ fails on eDP, per spec §5).
unsafe fn read_edid(fns: &IgclFns, out: ctl_display_output_handle_t) -> Option<[u8; 128]> {
    let pda = fns.panel_descriptor_access;
    let mut args = std::mem::zeroed::<ctl_panel_descriptor_access_args_t>();
    args.Size = std::mem::size_of::<ctl_panel_descriptor_access_args_t>() as u32;
    args.OpType = CTL_OPERATION_TYPE_READ;
    args.BlockNumber = 0;
    let r1 = (pda)(out, &mut args);
    if r1 != CTL_RESULT_SUCCESS || args.DescriptorDataSize == 0 {
        return None;
    }
    let mut buf = vec![0u8; args.DescriptorDataSize as usize];
    args.pDescriptorData = buf.as_mut_ptr();
    let r2 = (pda)(out, &mut args);
    if r2 != CTL_RESULT_SUCCESS {
        return None;
    }
    let mut edid = [0u8; 128];
    let n = buf.len().min(128);
    edid[..n].copy_from_slice(&buf[..n]);
    Some(edid)
}

/// Enumerate all ACTIVE Intel outputs, each with its EDID-derived `edid_id`
/// (`None` when the EDID is unreadable). Correlation is never guessed.
unsafe fn active_outputs(
    fns: &IgclFns,
    devices: &[ctl_device_adapter_handle_t],
) -> Result<Vec<(ctl_display_output_handle_t, Option<String>)>, String> {
    let mut out = Vec::new();
    for &dev in devices {
        // One flaky adapter must not hide every healthy Intel output: skip and
        // log the device, then keep enumerating the rest. Propagating here
        // would make `correlates_to_intel` false for the whole Intel adapter
        // and silently route a good panel through the GDI fallback.
        let outputs = match enumerate_outputs(fns, dev) {
            Ok(outputs) => outputs,
            Err(e) => {
                eprintln!("Intel: skipping device during output enumeration: {e}");
                continue;
            }
        };
        for h in outputs {
            let props = match display_props(fns, h) {
                Ok(props) => props,
                Err(e) => {
                    eprintln!("Intel: skipping output during display-property read: {e}");
                    continue;
                }
            };
            if props.DisplayConfigFlags & CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE == 0 {
                continue;
            }
            let id = read_edid(fns, h).map(|ed| crate::monitor::edid_id_from_bytes(&ed));
            out.push((h, id));
        }
    }
    Ok(out)
}

/// Query the pixtx pipeline CAPABILITY: block ids, types, sample counts.
unsafe fn pixtx_capability(
    fns: &IgclFns,
    out: ctl_display_output_handle_t,
) -> Result<Vec<IgclBlockInfo>, String> {
    let mut get = std::mem::zeroed::<ctl_pixtx_pipe_get_config_t>();
    get.Size = std::mem::size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
    get.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY;
    get.pBlockConfigs = std::ptr::null_mut();
    // Pass 1 fills NumBlocks; the return may be SUCCESS or
    // CTL_RESULT_ERROR_INSUFFICIENT_PIXTX_BLOCK_CONFIG_MEMORY (both valid).
    let _ = (fns.pixtx_get_config)(out, &mut get);
    let n = get.NumBlocks as usize;
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut blocks: Vec<ctl_pixtx_block_config_t> = (0..n)
        .map(|_| {
            let mut b = std::mem::zeroed::<ctl_pixtx_block_config_t>();
            b.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
            b
        })
        .collect();
    get.pBlockConfigs = blocks.as_mut_ptr();
    get.NumBlocks = n as u32;
    let r2 = (fns.pixtx_get_config)(out, &mut get);
    if r2 != CTL_RESULT_SUCCESS {
        return Err(format!("ctlPixelTransformationGetConfig failed ({})", result_str(r2)));
    }
    Ok(blocks
        .iter()
        .map(|b| IgclBlockInfo {
            block_id: b.BlockId,
            block_type: b.BlockType,
            samples_per_channel: unsafe { b.Config.OneDLutConfig.NumSamplesPerChannel },
            channels: unsafe { b.Config.OneDLutConfig.NumChannels },
        })
        .collect())
}

/// Set a custom pixtx pipeline (ordered block list).
unsafe fn set_pixtx(
    fns: &IgclFns,
    out: ctl_display_output_handle_t,
    blocks: &[PixtxBlock],
) -> Result<(), String> {
    // Sample storage must outlive the FFI call (block configs hold pointers
    // into it), so it is declared before the block-config vector.
    let mut sample_storage: Vec<Vec<f64>> = Vec::with_capacity(blocks.len());
    let mut ffi_blocks: Vec<ctl_pixtx_block_config_t> = Vec::with_capacity(blocks.len());

    for b in blocks {
        match b {
            PixtxBlock::OneDLut {
                block_id,
                samples_per_channel,
                channels,
                samples,
            } => {
                sample_storage.push(samples.clone());
                let mut bc = std::mem::zeroed::<ctl_pixtx_block_config_t>();
                bc.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
                bc.BlockId = *block_id;
                bc.BlockType = CTL_PIXTX_BLOCK_TYPE_1D_LUT;
                bc.Config.OneDLutConfig.Size = std::mem::size_of::<ctl_pixtx_1dlut_config_t>() as u32;
                bc.Config.OneDLutConfig.SamplingType = 0;
                bc.Config.OneDLutConfig.NumSamplesPerChannel = *samples_per_channel;
                bc.Config.OneDLutConfig.NumChannels = *channels;
                bc.Config.OneDLutConfig.pSampleValues = sample_storage.last_mut().unwrap().as_mut_ptr();
                bc.Config.OneDLutConfig.pSamplePositions = std::ptr::null_mut();
                ffi_blocks.push(bc);
            }
            PixtxBlock::Matrix {
                block_id,
                block_type,
                matrix,
            } => {
                let mut bc = std::mem::zeroed::<ctl_pixtx_block_config_t>();
                bc.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
                bc.BlockId = *block_id;
                bc.BlockType = *block_type;
                bc.Config.MatrixConfig.Size = std::mem::size_of::<ctl_pixtx_matrix_config_t>() as u32;
                bc.Config.MatrixConfig.PreOffsets = [0.0; 3];
                bc.Config.MatrixConfig.PostOffsets = [0.0; 3];
                bc.Config.MatrixConfig.Matrix = *matrix;
                ffi_blocks.push(bc);
            }
        }
    }

    let mut set = std::mem::zeroed::<ctl_pixtx_pipe_set_config_t>();
    set.Size = std::mem::size_of::<ctl_pixtx_pipe_set_config_t>() as u32;
    set.OpertaionType = CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM;
    set.Flags = 0;
    set.NumBlocks = ffi_blocks.len() as u32;
    set.pBlockConfigs = ffi_blocks.as_mut_ptr();
    let r = (fns.pixtx_set_config)(out, &mut set);
    if r != CTL_RESULT_SUCCESS {
        return Err(format!("ctlPixelTransformationSetConfig failed ({})", result_str(r)));
    }
    Ok(())
}

/// Restore default pixtx (`RESTORE_DEFAULT`, `NumBlocks=0`) — the ONLY
/// supported reset on this driver (`ctlDisplayFeatureReset` is not exported).
unsafe fn restore_pixtx(fns: &IgclFns, out: ctl_display_output_handle_t) -> Result<(), String> {
    let mut set = std::mem::zeroed::<ctl_pixtx_pipe_set_config_t>();
    set.Size = std::mem::size_of::<ctl_pixtx_pipe_set_config_t>() as u32;
    set.OpertaionType = CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT;
    set.Flags = 0;
    set.NumBlocks = 0;
    set.pBlockConfigs = std::ptr::null_mut();
    let r = (fns.pixtx_set_config)(out, &mut set);
    if r != CTL_RESULT_SUCCESS {
        return Err(format!("ctlPixelTransformationSetConfig(RESTORE_DEFAULT) failed ({})", result_str(r)));
    }
    Ok(())
}

/// Read the CURRENT samples of one 1D-LUT pixtx block. Mirrors the Intel
/// `Color_Sample_App` / spike CURRENT protocol (spec §10): one block at a
/// time, `QueryType=CURRENT`, `NumBlocks=1`, with the sample buffer
/// pre-allocated to `samples_per_channel * channels` doubles in
/// R-then-G-then-B order. (Matrix blocks cannot be read back on this driver —
/// spec §6/§10 — so this is 1D-LUT only.)
unsafe fn read_pixtx_1dlut(
    fns: &IgclFns,
    out: ctl_display_output_handle_t,
    block_id: u32,
    samples_per_channel: u32,
    channels: u32,
) -> Result<Vec<f64>, String> {
    let n = samples_per_channel as usize * channels.max(1) as usize;
    let mut samples = vec![0.0f64; n.max(1)];

    let mut block = std::mem::zeroed::<ctl_pixtx_block_config_t>();
    block.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
    block.BlockId = block_id;
    block.BlockType = CTL_PIXTX_BLOCK_TYPE_1D_LUT;
    // NOTE: inner `OneDLutConfig.Size`/`pSamplePositions` are left zeroed to
    // match the spike's proven CURRENT read-back path (spec §10) — the driver
    // fills `pSampleValues` and does not require the inner Size tag here.
    block.Config.OneDLutConfig.SamplingType = 0;
    block.Config.OneDLutConfig.NumSamplesPerChannel = samples_per_channel;
    block.Config.OneDLutConfig.NumChannels = channels;
    block.Config.OneDLutConfig.pSampleValues = samples.as_mut_ptr();

    let mut get = std::mem::zeroed::<ctl_pixtx_pipe_get_config_t>();
    get.Size = std::mem::size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
    get.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT;
    get.NumBlocks = 1;
    get.pBlockConfigs = &mut block;

    let r = (fns.pixtx_get_config)(out, &mut get);
    if r != CTL_RESULT_SUCCESS {
        return Err(format!(
            "ctlPixelTransformationGetConfig(CURRENT 1D-LUT {block_id}) failed ({})",
            result_str(r)
        ));
    }
    Ok(samples)
}

fn result_str(r: ctl_result_t) -> String {
    let name = match r {
        0x0000_0000 => "SUCCESS",
        0x4000_000b => "INVALID_ARGUMENT",
        0x4000_000d => "INVALID_NULL_HANDLE",
        0x4000_000e => "INVALID_NULL_POINTER",
        0x4000_000f => "INVALID_SIZE",
        0x4000_0014 => "DATA_NOT_FOUND",
        0x4000_0015 => "NOT_IMPLEMENTED",
        0x4000_0016 => "OS_CALL",
        0x4000_0017 => "KMD_CALL",
        0x4800_0003 => "DISPLAY_NOT_ATTACHED",
        0x4800_0004 => "DISPLAY_NOT_ACTIVE",
        0x4800_000b => "INVALID_PIXTX_BLOCK_TYPE",
        0x4800_000d => "INSUFFICIENT_PIXTX_BLOCK_CONFIG_MEMORY",
        0x4800_0010 => "3DLUT_NOT_SUPPORTED_IN_HDR",
        _ => "UNKNOWN",
    };
    format!("0x{r:08X} {name}")
}

// ── Safe seam (the injectable trait) ────────────────────────────────────────

/// Opaque token for a correlated Intel display output. Constructed only by an
/// `IgclApi` implementation; never dereferenced outside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgclOutput(pub(crate) ctl_display_output_handle_t);

/// One pixtx block from the CAPABILITY query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgclBlockInfo {
    pub block_id: u32,
    pub block_type: u32,
    pub samples_per_channel: u32,
    pub channels: u32,
}

/// Per-output probe result: HDR flag + the pixtx block capability.
#[derive(Debug, Clone, PartialEq)]
pub struct IgclProbe {
    pub hdr: bool,
    pub blocks: Vec<IgclBlockInfo>,
}

/// A composed pixtx block to SET. `OneDLut.samples` is `samples_per_channel *
/// channels` normalized `f64` values in R-then-G-then-B order.
#[derive(Debug, Clone, PartialEq)]
pub enum PixtxBlock {
    OneDLut {
        block_id: u32,
        samples_per_channel: u32,
        channels: u32,
        samples: Vec<f64>,
    },
    Matrix {
        block_id: u32,
        block_type: u32,
        matrix: [[f64; 3]; 3],
    },
}

/// Safe, injectable IGCL surface. The production implementation (`RealIgcl`)
/// is the only code that touches raw FFI; tests inject a mock that never
/// loads the real DLL.
pub trait IgclApi {
    /// Correlate `edid_id` to a unique active Intel output. `Err(reason)`
    /// when there is no match or the match is ambiguous — never guess.
    fn resolve(&self, edid_id: &str) -> Result<IgclOutput, String>;

    /// Read pixtx capability + display properties for an output.
    fn probe(&self, output: IgclOutput) -> Result<IgclProbe, String>;

    /// Apply a custom pixtx pipeline (ordered block list).
    fn set_pipeline(&self, output: IgclOutput, blocks: &[PixtxBlock]) -> Result<(), String>;

    /// Restore default pixtx (RESTORE_DEFAULT, NumBlocks=0).
    fn restore_default(&self, output: IgclOutput) -> Result<(), String>;

    /// Read the CURRENT samples of one 1D-LUT pixtx block (one block at a
    /// time). Matrix blocks are NOT readable on this driver (spec §6/§10).
    fn read_1dlut(
        &self,
        output: IgclOutput,
        block_id: u32,
        samples_per_channel: u32,
        channels: u32,
    ) -> Result<Vec<f64>, String>;
}

/// Production IGCL implementation.
pub struct RealIgcl;

static REAL_IGCL: RealIgcl = RealIgcl;

impl IgclApi for RealIgcl {
    fn resolve(&self, edid_id: &str) -> Result<IgclOutput, String> {
        let s = session().ok_or_else(|| "Intel IGCL unavailable (driver not loaded)".to_string())?;
        let outputs = unsafe { active_outputs(&s.fns, &s.devices)? };
        let ids: Vec<Option<String>> = outputs.iter().map(|(_, id)| id.clone()).collect();
        let idx = correlate(&ids, edid_id)?;
        Ok(IgclOutput(outputs[idx].0))
    }

    fn probe(&self, output: IgclOutput) -> Result<IgclProbe, String> {
        let s = session().ok_or_else(|| "Intel IGCL unavailable (driver not loaded)".to_string())?;
        let props = unsafe { display_props(&s.fns, output.0)? };
        // HDR is classified from the ENABLED flag, not the SUPPORTED flag
        // (see `hdr_from_props`). A panel that merely *supports* HDR while
        // running SDR must not disable the backend.
        let hdr = hdr_from_props(&props);
        let blocks = unsafe { pixtx_capability(&s.fns, output.0)? };
        Ok(IgclProbe { hdr, blocks })
    }

    fn set_pipeline(&self, output: IgclOutput, blocks: &[PixtxBlock]) -> Result<(), String> {
        let s = session().ok_or_else(|| "Intel IGCL unavailable (driver not loaded)".to_string())?;
        unsafe { set_pixtx(&s.fns, output.0, blocks) }
    }

    fn restore_default(&self, output: IgclOutput) -> Result<(), String> {
        let s = session().ok_or_else(|| "Intel IGCL unavailable (driver not loaded)".to_string())?;
        unsafe { restore_pixtx(&s.fns, output.0) }
    }

    fn read_1dlut(
        &self,
        output: IgclOutput,
        block_id: u32,
        samples_per_channel: u32,
        channels: u32,
    ) -> Result<Vec<f64>, String> {
        let s = session().ok_or_else(|| "Intel IGCL unavailable (driver not loaded)".to_string())?;
        unsafe { read_pixtx_1dlut(&s.fns, output.0, block_id, samples_per_channel, channels) }
    }
}

/// Whether `edid_id` correlates to a (unique) active Intel output. Drives
/// backend selection; never guesses — ambiguous/absent → `false`.
pub(crate) fn correlates_to_intel(edid_id: &str) -> bool {
    REAL_IGCL.resolve(edid_id).is_ok()
}

// ── Negative-verdict cache for the drift loop (L-4) ─────────────────────────
//
// On a machine with no Intel driver, `correlates_to_intel` → `session()` fails
// and re-runs the DriverStore scan on a 5s backoff. The drift loop calls it
// every ~10s per pinned display, so the scan would repeat roughly every other
// cycle. This cache holds a *negative* verdict per EDID for a much longer TTL
// so the hot path stays cheap. Backend selection (`BackendRegistry::select`)
// does NOT use this cache — it must remain hot-plug correct.

/// How long a negative correlation verdict is trusted before a re-probe.
const NEGATIVE_CORRELATION_TTL: Duration = Duration::from_secs(5 * 60);

static NEGATIVE_CORRELATION_CACHE: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn negative_cache() -> &'static Mutex<HashMap<String, Instant>> {
    NEGATIVE_CORRELATION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Pure TTL check (unit-testable without FFI): is `edid_id`'s negative verdict
/// still active at `now`?
fn negative_hit(entries: &HashMap<String, Instant>, edid_id: &str, now: Instant) -> bool {
    entries.get(edid_id).map(|expiry| now < *expiry).unwrap_or(false)
}

/// `correlates_to_intel` with a per-EDID negative-verdict cache. A negative
/// verdict is reused for `NEGATIVE_CORRELATION_TTL`; a positive verdict clears
/// any stale negative entry. There is no WM_DISPLAYCHANGE topology signal wired
/// in this app, so the TTL bounds staleness (a newly-connected Intel display is
/// correctly recognized within one TTL — harmless for the drift loop, which
/// only *skips* re-applying Intel displays).
pub(crate) fn correlates_to_intel_cached(edid_id: &str) -> bool {
    let now = Instant::now();
    if let Ok(cache) = negative_cache().lock() {
        if negative_hit(&cache, edid_id, now) {
            return false;
        }
    }
    let positive = correlates_to_intel(edid_id);
    if let Ok(mut cache) = negative_cache().lock() {
        if positive {
            cache.remove(edid_id);
        } else {
            cache.insert(edid_id.to_string(), now + NEGATIVE_CORRELATION_TTL);
        }
    }
    positive
}

/// Read the CURRENT pixtx 1D-LUT state for `edid_id` as `(peak, identity)`.
///
/// The UI Probe uses this instead of the unrelated OS GDI ramp: an Intel
/// display's colour is driven by pixtx, so reading the GDI ramp reports
/// "identity" while the panel is actually tinted. `peak` is the largest
/// normalized sample; `identity` is true only when every channel is the
/// linear identity curve. All FFI stays in this module.
pub fn current_1dlut_summary(edid_id: &str) -> Result<(f64, bool), String> {
    let output = REAL_IGCL.resolve(edid_id)?;
    let info = REAL_IGCL.probe(output)?;
    let lut = info
        .blocks
        .iter()
        .filter(|b| b.block_type == CTL_PIXTX_BLOCK_TYPE_1D_LUT)
        .max_by_key(|b| b.channels)
        .copied()
        .ok_or_else(|| "no 1D LUT block in the pixtx pipeline".to_string())?;

    let samples = REAL_IGCL.read_1dlut(
        output,
        lut.block_id,
        lut.samples_per_channel,
        lut.channels,
    )?;

    let channels = lut.channels.max(1) as usize;
    let per = lut.samples_per_channel as usize;
    if per == 0 || samples.len() < per * channels {
        return Err(format!(
            "pixtx 1D LUT read returned {} samples; expected {}",
            samples.len(),
            per * channels
        ));
    }

    let peak = samples
        .iter()
        .copied()
        .fold(0.0f64, f64::max)
        .clamp(0.0, 1.0);
    let denom = (per.saturating_sub(1)).max(1) as f64;
    let identity = (0..channels).all(|c| {
        (0..per).all(|i| {
            let expected = i as f64 / denom;
            (samples[c * per + i] - expected).abs() <= 1e-9
        })
    });

    Ok((peak, identity))
}

// ── Pure correlation decision (unit-testable, never guesses) ────────────────

/// Find the unique index of the active output whose `edid_id` matches
/// `target`. `Err` when zero or more than one match — identical-twin panels
/// (same EDID on two outputs) are reported ambiguous rather than guessed.
pub(crate) fn correlate(outputs: &[Option<String>], target: &str) -> Result<usize, String> {
    let mut it = outputs
        .iter()
        .enumerate()
        .filter(|(_, id)| id.as_deref() == Some(target));
    match (it.next(), it.next()) {
        (Some((idx, _)), None) => Ok(idx),
        (None, _) => Err(format!("no active Intel output correlates to display {target}")),
        (Some(_), Some(_)) => Err(format!("ambiguous: multiple Intel outputs share EDID {target}")),
    }
}

/// Classify HDR from the *enabled* feature flag, not the *supported* flag.
///
/// `CTL_STD_DISPLAY_FEATURE_FLAG_HDR` appears in both `FeatureSupportedFlags`
/// ("this panel is HDR-capable") and `FeatureEnabledFlags` ("HDR is currently
/// active"). Classifying from the supported flag — the original bug — disables
/// the whole Intel backend on every HDR-*capable* panel running SDR. Only the
/// enabled flag reflects the panel's *current* mode: supported-but-not-enabled
/// must leave adjustments available.
///
/// QueryDisplayConfig's advanced-color state would be a redundant cross-check;
/// the `windows` crate's `Win32_Graphics_Display` feature (where
/// `QueryDisplayConfig`/`DISPLAYCONFIG_*` live) is not enabled and adding it is
/// out of scope, so the IGCL enabled-flags signal is the authoritative source.
pub(crate) fn hdr_enabled(feature_enabled_flags: u32) -> bool {
    (feature_enabled_flags & CTL_STD_DISPLAY_FEATURE_FLAG_HDR) != 0
}

/// Classify HDR from a `ctl_display_properties_t`'s *enabled* field (never the
/// *supported* field). Extracted from the production call site so the field
/// read is pinned by a unit test: reverting `probe` to
/// `FeatureSupportedFlags` would now fail the test instead of silently staying
/// green (L-1).
fn hdr_from_props(props: &ctl_display_properties_t) -> bool {
    hdr_enabled(props.FeatureEnabledFlags)
}

// ── ColorBackend probe reasons (static — `AdjustmentCap` carries &'static str) ──

const REASON_NO_CORRELATION: &str = "Intel IGCL cannot correlate this display to an Intel output";
const REASON_NO_PIXTX: &str = "Intel pixtx pipeline lacks the required block for this display";
const REASON_NO_VIBRANCE: &str = "Intel IGCL has no vibrance control (saturation is the closest analogue)";
const REASON_HDR: &str = "display is in HDR mode (Intel pixtx LUTs unsupported in HDR)";

// ── UI-unit → normalized-value mapping (documented) ─────────────────────────
//
// ChromaDeck presets use these units (neutral in parens):
//   brightness, contrast : UI 0–100 (50)   → linear gain via `ui_to_gain`
//   gamma                : exponent (1.0)  → used directly in `x^gamma`
//   rgb_gains            : [f64;3] (1.0)   → matrix diagonal
//   saturation           : UI 0–100 (50)   → factor via `saturation_factor`
//   hue_deg              : degrees (0)     → radians inside the rotation
//
// `ui_to_gain` matches the GDI backend's gain model (0.8 + 0.004·ui) so the
// Intel and GDI paths render the same neutral (50 → 1.0) and the same ends.

/// Rec.709 luma coefficients for the luma-lerp saturation matrix.
const LUMA_R: f64 = 0.2126;
const LUMA_G: f64 = 0.7152;
const LUMA_B: f64 = 0.0722;

/// Map a ChromaDeck brightness/contrast UI value (0–100, neutral 50) to a
/// linear gain (neutral 50 → 1.0; range 0.8 → 1.2). Matches the GDI model.
pub(crate) fn ui_to_gain(ui: f64) -> f64 {
    0.8 + 0.004 * ui
}

/// Map ChromaDeck saturation UI (0–100, neutral 50) to a saturation factor
/// (neutral 50 → 1.0, 0 → grayscale, 100 → 2.0). Documented deviation from
/// Intel's Color_Sample_App example (0.75–1.25): that range over-compresses
/// the slider; ChromaDeck exposes the full 0–2.0 its editor already offers.
pub(crate) fn saturation_factor(ui_saturation: f64) -> f64 {
    ui_saturation / 50.0
}

// ── 1D LUT + matrix builders (pure, golden-value testable) ──────────────────

/// Build a channel-uniform 1D LUT curve with `samples` entries, normalized
/// [0,1]. Mirrors the GDI `build_gamma_ramp` formula — minus the per-channel
/// gains (which live in the matrix) and minus the GDI `SetDeviceGammaRamp`
/// peak floor (a GDI-specific quirk the pixtx path does not need):
///   lut(i) = clamp( clamp((x^gamma − 0.5)·contrast_gain + 0.5, 0, 1) · brightness_gain, 0, 1 )
///   x = i / (samples − 1)
pub(crate) fn build_gamma_lut(
    samples: u32,
    brightness_gain: f64,
    contrast_gain: f64,
    gamma: f64,
) -> Vec<f64> {
    let denom = (samples - 1) as f64;
    let mut lut = Vec::with_capacity(samples as usize);
    for i in 0..samples {
        let x = i as f64 / denom;
        let mut v = x.powf(gamma);
        v = (v - 0.5) * contrast_gain + 0.5;
        v *= brightness_gain;
        lut.push(v.clamp(0.0, 1.0));
    }
    lut
}

/// White-balance matrix: `diag(rgb_gains)`. Identity at gains [1,1,1].
pub(crate) fn gains_matrix(rgb_gains: [f64; 3]) -> [[f64; 3]; 3] {
    [
        [rgb_gains[0], 0.0, 0.0],
        [0.0, rgb_gains[1], 0.0],
        [0.0, 0.0, rgb_gains[2]],
    ]
}

/// Luma-lerp saturation matrix (Rec.709). `s = 1.0` is identity; `s = 0.0`
/// collapses each channel to its luma (grayscale).
pub(crate) fn saturation_matrix(s: f64) -> [[f64; 3]; 3] {
    let i = 1.0 - s;
    [
        [i * LUMA_R + s, i * LUMA_G, i * LUMA_B],
        [i * LUMA_R, i * LUMA_G + s, i * LUMA_B],
        [i * LUMA_R, i * LUMA_G, i * LUMA_B + s],
    ]
}

/// YIQ-style hue rotation (the W3C/SVG `hue-rotate` matrix, Rec.709 weights):
/// rotates chroma around the luma axis by `hue_deg`. Luma-preserving (each row
/// sums to 1.0) and identity at 0°.
pub(crate) fn hue_rotation_matrix(hue_deg: f64) -> [[f64; 3]; 3] {
    let theta = hue_deg.to_radians();
    let c = theta.cos();
    let s = theta.sin();
    [
        [
            0.213 + c * 0.787 - s * 0.213,
            0.715 - c * 0.715 - s * 0.715,
            0.072 - c * 0.072 + s * 0.928,
        ],
        [
            0.213 - c * 0.213 + s * 0.143,
            0.715 + c * 0.285 + s * 0.140,
            0.072 - c * 0.072 - s * 0.283,
        ],
        [
            0.213 - c * 0.213 - s * 0.787,
            0.715 - c * 0.715 + s * 0.715,
            0.072 + c * 0.928 + s * 0.072,
        ],
    ]
}

/// 3×3 matrix multiply (column-vector convention: `(a·b)·v` applies `b` first).
pub(crate) fn mat_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut m = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// Compose the color matrix in documented order. Column-vector convention:
/// `M = H · S · G` applies gains (white balance) first, then saturation, then
/// hue rotation: `rgb_out = H · (S · (G · rgb_in))`.
pub(crate) fn compose_color_matrix(
    rgb_gains: [f64; 3],
    saturation_ui: f64,
    hue_deg: f64,
) -> [[f64; 3]; 3] {
    let g = gains_matrix(rgb_gains);
    let s = saturation_matrix(saturation_factor(saturation_ui));
    let h = hue_rotation_matrix(hue_deg);
    mat_mul(&mat_mul(&h, &s), &g)
}

/// The 3×3 identity matrix (neutral colour, no gain/saturation/hue transform).
const IDENTITY_3X3: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Whether a composed 3×3 matrix is the identity (within float noise).
fn matrix_is_identity(m: &[[f64; 3]; 3]) -> bool {
    let mut identity = true;
    for (i, row) in m.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            let expected = if i == j { 1.0 } else { 0.0 };
            if (value - expected).abs() > 1e-12 {
                identity = false;
            }
        }
    }
    identity
}

/// The preferred 3×3 matrix block (`3X3_MATRIX_AND_OFFSETS` over
/// `3X3_MATRIX`) from a probe, if the display exposes one.
fn matrix_block_of(info: &IgclProbe) -> Option<IgclBlockInfo> {
    info.blocks
        .iter()
        .filter(|b| {
            b.block_type == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS
                || b.block_type == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX
        })
        .max_by_key(|b| b.block_type)
        .copied()
}

/// Compose a preset into the pixtx SET block list.
///
/// Composition (documented):
///   gamma/brightness/contrast → per-channel 1D LUT (the pixtx gamma-encode
///       block; 2049 samples/ch, normalized [0,1], channel-uniform curve).
///   rgb_gains → matrix diagonal; saturation → luma-lerp 3×3; hue → YIQ
///       rotation 3×3, combined as ONE matrix `M = H·S·G`.
///   The color matrix targets the 3×3 matrix block in the LINEAR position —
///   preferred `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS` (which sits after
///   the degamma 1D LUT and before the gamma-encode 1D LUT in the pipeline),
///   falling back to the trailing `CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX` when only
///   that is present. SET block order follows the pipeline: [Matrix, 1D LUT].
///
/// Partial pipelines are composed from whichever blocks the display exposes,
/// because `verdicts_for` advertises per-family support:
///   - no matrix block + identity composed matrix (no rgb_gains/saturation/hue
///     deviation) → LUT-only pipeline;
///   - no 1D LUT block + no gamma-family deviation → matrix-only pipeline.
///
/// If the preset requests adjustments the missing block would carry, return
/// `Err` rather than silently dropping them. `samples_per_channel` must be at
/// least 2 (a 1-sample LUT would divide by zero in `build_gamma_lut`).
pub(crate) fn compose_pipeline(preset: &Preset, info: &IgclProbe) -> Result<Vec<PixtxBlock>, String> {
    let lut = info
        .blocks
        .iter()
        .filter(|b| b.block_type == CTL_PIXTX_BLOCK_TYPE_1D_LUT)
        .max_by_key(|b| b.channels)
        .copied();

    let matrix_block = matrix_block_of(info);

    let matrix = compose_color_matrix(preset.rgb_gains, preset.saturation, preset.hue_deg);
    let matrix_identity = matrix_is_identity(&matrix);

    let gamma_requested = preset.brightness != 50.0
        || preset.contrast != 50.0
        || (preset.gamma - 1.0).abs() > 1e-12;

    let mut blocks = Vec::with_capacity(2);

    if let Some(block) = matrix_block {
        // The matrix is ALWAYS positively written — even the identity — so a
        // prior saturation/hue/gains apply (a non-identity matrix) is
        // neutralized when a neutral preset is applied. `SET_CUSTOM` preserves
        // unspecified blocks, so omitting the identity matrix here would leave
        // the old matrix active (M-1). The identity write is verification by
        // construction: the matrix block cannot be read back on this driver
        // (spec §6/§10).
        blocks.push(PixtxBlock::Matrix {
            block_id: block.block_id,
            block_type: block.block_type,
            matrix,
        });
    } else if !matrix_identity {
        return Err(
            "no 3x3 matrix block in the pixtx pipeline (preset requires rgb_gains/saturation/hue)"
                .to_string(),
        );
    }

    match lut {
        Some(block) => {
            if block.samples_per_channel < 2 {
                return Err(format!(
                    "1D LUT block has {} samples per channel; at least 2 required",
                    block.samples_per_channel
                ));
            }
            let channels = block.channels.max(1) as usize;
            let curve = build_gamma_lut(
                block.samples_per_channel,
                ui_to_gain(preset.brightness),
                ui_to_gain(preset.contrast),
                preset.gamma,
            );
            let mut samples = Vec::with_capacity(curve.len() * channels);
            for _ in 0..channels {
                samples.extend_from_slice(&curve);
            }
            blocks.push(PixtxBlock::OneDLut {
                block_id: block.block_id,
                samples_per_channel: block.samples_per_channel,
                channels: channels as u32,
                samples,
            });
        }
        None if gamma_requested => {
            return Err(
                "no 1D LUT block in the pixtx pipeline (preset requires gamma/brightness/contrast)"
                    .to_string(),
            );
        }
        None => {}
    }

    if blocks.is_empty() {
        return Err("preset is neutral and the pixtx pipeline exposes no writable block".to_string());
    }

    Ok(blocks)
}

// ── Pre-apply 1D LUT capture (session-scoped, in-memory only) ───────────────
//
// `ctlPixelTransformationSetConfig(RESTORE_DEFAULT, NumBlocks=0)` resets the
// 3×3 matrix but does NOT restore the 3-channel gamma-encode 1D LUT on this
// driver (spec §13.4). To return a panel to its pre-apply state reliably we
// capture the 1D LUT block we are about to overwrite on every apply, then on
// restore re-SET_CUSTOM that capture and verify it by read-back. The capture
// is keyed by `edid_id` and held only in memory — pixtx is runtime-only driver
// state (a reboot returns the panel to factory default), so nothing is
// persisted to disk.

/// A captured pre-apply 1D LUT for one display: enough to re-SET_CUSTOM the
/// encode block back to its exact pre-apply curve on restore.
#[derive(Clone)]
struct CapturedLut {
    block_id: u32,
    samples_per_channel: u32,
    channels: u32,
    samples: Vec<f64>,
}

/// Session-scoped capture table, shared across `IntelBackend` instances (each
/// `BackendRegistry::new()` constructs a fresh backend, so the table cannot
/// live on the instance).
static CAPTURED_LUTS: OnceLock<Mutex<HashMap<String, CapturedLut>>> = OnceLock::new();

fn captured_luts() -> &'static Mutex<HashMap<String, CapturedLut>> {
    CAPTURED_LUTS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── M-4: untrusted pre-apply captures (restart-without-reboot) ──────────────
//
// A persisted applied/pin record survives an app restart but NOT the driver's
// pixtx state: after a restart-without-reboot the panel still carries the
// *previous* ChromaDeck session's applied curve, not the factory default.
// Capturing that leftover as the "pre-apply" baseline would make a later Reset
// replay (and falsely "verify") a non-factory curve. At startup `run_startup`
// marks every Intel-correlated EDID with such a record; `apply` then refuses
// to capture a non-identity (i.e. non-factory, spec §14.3) pre-apply LUT and
// tells the user to reboot. If the LUT is verifiably identity, the mark is
// cleared and the session proceeds normally.

/// EDIDs whose pre-apply capture is NOT trusted this session.
static CAPTURE_UNTRUSTED: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();

fn capture_untrusted() -> &'static Mutex<std::collections::HashSet<String>> {
    CAPTURE_UNTRUSTED.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// Mark an EDID's capture as untrusted for this session (startup, M-4).
pub(crate) fn mark_capture_untrusted(edid_id: &str) {
    if let Ok(mut set) = capture_untrusted().lock() {
        set.insert(edid_id.to_string());
    }
}

fn is_capture_untrusted(edid_id: &str) -> bool {
    capture_untrusted()
        .lock()
        .map(|set| set.contains(edid_id))
        .unwrap_or(false)
}

fn clear_capture_untrusted(edid_id: &str) {
    if let Ok(mut set) = capture_untrusted().lock() {
        set.remove(edid_id);
    }
}

/// Serializes full IGCL apply/restore sequences (not just the capture table).
/// One driver session and one pixtx pipeline per output are process-global;
/// concurrent apply + reapply/startup/loop would otherwise interleave
/// read→store→set or restore→set→read and race on the same state. A single
/// global lock is correct for the IGCL session; per-EDID granularity can come
/// later if apply throughput ever matters.
/// Displays whose last successful apply wrote a matrix-only pipeline (no 1D
/// LUT block was available/touched). Restore can then reset the matrix and
/// report success without a LUT capture, because no LUT state was changed.
static MATRIX_ONLY_APPLIES: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();

fn matrix_only_applies() -> &'static Mutex<std::collections::HashSet<String>> {
    MATRIX_ONLY_APPLIES.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn mark_matrix_only(edid_id: &str) {
    if let Ok(mut set) = matrix_only_applies().lock() {
        set.insert(edid_id.to_string());
    }
}

fn clear_matrix_only(edid_id: &str) {
    if let Ok(mut set) = matrix_only_applies().lock() {
        set.remove(edid_id);
    }
}

fn is_matrix_only(edid_id: &str) -> bool {
    matrix_only_applies()
        .lock()
        .map(|set| set.contains(edid_id))
        .unwrap_or(false)
}

static PIXTX_SEQUENCE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Hold this across a complete apply or restore sequence. Poisoning is
/// recovered rather than propagated: a prior panic must not permanently
/// disable Intel handling.
fn pipeline_lock() -> std::sync::MutexGuard<'static, ()> {
    PIXTX_SEQUENCE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Store a capture. `Err` only when the table mutex is poisoned (a prior panic
/// while holding it) — callers treat that as "cannot guarantee restore" and
/// refuse to apply unrestorable state.
fn store_capture(edid_id: &str, lut: CapturedLut) -> Result<(), String> {
    captured_luts()
        .lock()
        .map_err(|_| "capture table poisoned".to_string())?
        .insert(edid_id.to_string(), lut);
    Ok(())
}

/// Look up the capture for a display. `None` when absent (no apply this
/// session) or the table is poisoned — both mean "cannot verifiably restore".
fn get_capture(edid_id: &str) -> Option<CapturedLut> {
    captured_luts().lock().ok()?.get(edid_id).cloned()
}

/// Drop the capture for a display after a *verified* successful restore, so a
/// later apply re-captures the current (restored) state rather than a stale
/// pre-apply curve. Best-effort: a poisoned table is not fatal here because
/// the restore already succeeded.
fn clear_capture(edid_id: &str) {
    if let Ok(mut table) = captured_luts().lock() {
        table.remove(edid_id);
    }
}

/// Tight epsilon for read-back verification of a restored 1D LUT against the
/// captured pre-apply curve. A genuine non-restore is orders of magnitude
/// larger: RESTORE_DEFAULT leaves the last SET_CUSTOM value (identity), whose
/// low samples differ from the native limited-range encode by ~2.5e-2.
const LUT_VERIFY_EPSILON: f64 = 1e-9;

/// Largest absolute sample-wise difference between two LUT buffers.
fn max_delta(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f64, f64::max)
}

/// Whether two LUT buffers are byte-close (same length, within epsilon).
fn luts_match(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && max_delta(a, b) <= LUT_VERIFY_EPSILON
}

/// Whether a 1D-LUT buffer is the linear identity curve — the driver's factory
/// default for the 3ch gamma-encode block on this panel (spec §14.3). Used by
/// M-4 to decide whether a persisted-record display's pre-apply state is the
/// factory default (safe to capture) or a leftover curve (refuse).
fn lut_is_identity(samples: &[f64], samples_per_channel: u32, channels: u32) -> bool {
    let channels = channels.max(1) as usize;
    let per = samples_per_channel as usize;
    if per == 0 || samples.len() < per * channels {
        return false;
    }
    let denom = per.saturating_sub(1).max(1) as f64;
    (0..channels).all(|c| {
        (0..per).all(|i| {
            let expected = i as f64 / denom;
            (samples[c * per + i] - expected).abs() <= LUT_VERIFY_EPSILON
        })
    })
}

// ── IntelBackend (ColorBackend impl) ────────────────────────────────────────

/// Compose the restore pipeline for a resolved output: an explicit identity
/// matrix (when the display exposes a matrix block) plus the captured
/// pre-apply 1D LUT (when present). The matrix is written positively — by
/// construction, since the matrix block cannot be read back on this driver
/// (spec §6/§10) — while the LUT is verified by read-back in `restore`.
fn restore_blocks(info: &IgclProbe, captured: Option<&CapturedLut>) -> Vec<PixtxBlock> {
    let mut blocks = Vec::with_capacity(2);
    if let Some(mb) = matrix_block_of(info) {
        blocks.push(PixtxBlock::Matrix {
            block_id: mb.block_id,
            block_type: mb.block_type,
            matrix: IDENTITY_3X3,
        });
    }
    if let Some(c) = captured {
        blocks.push(PixtxBlock::OneDLut {
            block_id: c.block_id,
            samples_per_channel: c.samples_per_channel,
            channels: c.channels,
            samples: c.samples.clone(),
        });
    }
    blocks
}

/// Intel IGCL colour backend: pixtx 1D LUT (gamma family) + 3×3 matrix
/// (rgb_gains/saturation/hue). Vibrance is not an IGCL control.
pub struct IntelBackend {
    api: &'static dyn IgclApi,
}

impl IntelBackend {
    /// Production backend bound to the lazily-loaded real IGCL session.
    pub fn new() -> Self {
        IntelBackend { api: &REAL_IGCL }
    }

    /// Nominal (no-live-probe) capability table for a healthy Intel display.
    /// Used by the capability resolver's pure "intel" arm; the live
    /// HDR/pixtx probe refines it via `ColorBackend::probe`.
    pub fn nominal_probe() -> Vec<AdjustmentCapability> {
        verdicts_for(true, true, false)
    }

    /// Test seam: bind a mock FFI layer (never loads the real DLL).
    #[cfg(test)]
    pub fn with_api(api: &'static dyn IgclApi) -> Self {
        IntelBackend { api }
    }
}

/// Build the per-adjustment verdict table from live pixtx/display capability.
/// HDR (when detectable) disables everything; otherwise the 1D-LUT block gates
/// the gamma family and the 3×3-matrix block gates rgb_gains/saturation/hue.
fn verdicts_for(has_lut: bool, has_matrix: bool, hdr: bool) -> Vec<AdjustmentCapability> {
    if hdr {
        return all_unsupported(REASON_HDR);
    }
    let gamma_family = if has_lut {
        AdjustmentCap::Supported
    } else {
        AdjustmentCap::Unsupported(REASON_NO_PIXTX)
    };
    let matrix_family = if has_matrix {
        AdjustmentCap::Supported
    } else {
        AdjustmentCap::Unsupported(REASON_NO_PIXTX)
    };
    vec![
        AdjustmentCapability { adjustment: ADJUSTMENT_GAMMA, cap: gamma_family },
        AdjustmentCapability { adjustment: ADJUSTMENT_BRIGHTNESS, cap: gamma_family },
        AdjustmentCapability { adjustment: ADJUSTMENT_CONTRAST, cap: gamma_family },
        AdjustmentCapability { adjustment: ADJUSTMENT_RGB_GAINS, cap: matrix_family },
        AdjustmentCapability { adjustment: ADJUSTMENT_SATURATION, cap: matrix_family },
        AdjustmentCapability {
            adjustment: ADJUSTMENT_VIBRANCE,
            cap: AdjustmentCap::Unsupported(REASON_NO_VIBRANCE),
        },
        AdjustmentCapability { adjustment: ADJUSTMENT_HUE, cap: matrix_family },
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
    .iter()
    .map(|&a| AdjustmentCapability {
        adjustment: a,
        cap: AdjustmentCap::Unsupported(reason),
    })
    .collect()
}

impl ColorBackend for IntelBackend {
    fn id(&self) -> &'static str {
        "intel"
    }

    fn probe(&self, edid_id: &str) -> Vec<AdjustmentCapability> {
        match self.api.resolve(edid_id) {
            // Uncorrelated (incl. ambiguous) → never claim support.
            Err(_) => all_unsupported(REASON_NO_CORRELATION),
            Ok(output) => match self.api.probe(output) {
                Err(_) => all_unsupported(REASON_NO_PIXTX),
                Ok(info) => {
                    let has_lut = info
                        .blocks
                        .iter()
                        .any(|b| b.block_type == CTL_PIXTX_BLOCK_TYPE_1D_LUT);
                    let has_matrix = info.blocks.iter().any(|b| {
                        b.block_type == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX
                            || b.block_type == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS
                    });
                    verdicts_for(has_lut, has_matrix, info.hdr)
                }
            },
        }
    }

    fn apply(
        &self,
        api: &dyn ColorApi,
        _nv: &dyn NvColorApi,
        preset: &Preset,
        edid_id: &str,
    ) -> ApplyResult {
        // Serialize the whole read → store → set sequence. IGCL/pixtx state is
        // process-global driver state: a concurrent apply/restore (reapply,
        // startup, drift loop) must not interleave its capture or writes with
        // this one's.
        let _sequence = pipeline_lock();

        let mut gamma_applied = false;
        let mut error: Option<String> = None;

        // Purge the OS GDI LUT to identity first (mirrors NvapiBackend): a
        // stale GDI ramp from the pre-Intel path must not stack on top of the
        // Intel pixtx pipeline. Best-effort; failure is reported, not fatal.
        if let Some(e) = gdi_identity_purge(api, edid_id) {
            append_error(&mut error, e);
        }

        let mut wrote_lut = false;
        let result = (|| -> Result<(), String> {
            let output = self.api.resolve(edid_id)?;
            let info = self.api.probe(output)?;
            if info.hdr {
                return Err("display is in HDR mode; Intel pixtx LUTs unsupported in HDR".into());
            }
            let blocks = compose_pipeline(preset, &info)?;
            wrote_lut = blocks.iter().any(|b| matches!(b, PixtxBlock::OneDLut { .. }));

            // Find the 1D-LUT block we are about to overwrite, if the composed
            // pipeline has one. A matrix-only pipeline (display has no LUT
            // block, or the preset is LUT-neutral) needs no capture: the
            // matrix is reset by RESTORE_DEFAULT.
            let lut = blocks.iter().find_map(|b| match b {
                PixtxBlock::OneDLut { block_id, samples_per_channel, channels, .. } => {
                    Some((*block_id, *samples_per_channel, *channels))
                }
                _ => None,
            });

            // Capture the pre-apply LUT ONLY when no capture exists for this
            // display in this session. A second apply (preset switch, re-apply,
            // drift reapply, startup reapply) must NOT overwrite the original
            // capture: that would make restore return the panel to ChromaDeck's
            // own prior curve instead of its true pre-ChromaDeck state (C1).
            // If capture or its storage fails and we have no existing capture,
            // refuse to apply state we could never restore — report honestly
            // (product principle: reliable restoration over silent apply).
            if let Some((block_id, samples_per_channel, channels)) = lut {
                if get_capture(edid_id).is_none() {
                    let captured = self.api.read_1dlut(output, block_id, samples_per_channel, channels).map_err(|e| {
                        format!("capture pre-apply 1D LUT failed; refusing to apply unrestorable state: {e}")
                    })?;
                    // M-4: if this display started the session with a persisted
                    // applied/pin record, the pre-apply LUT may be ChromaDeck's
                    // own leftover curve (restart-without-reboot), not the
                    // driver factory default. Only capture it when it is
                    // verifiably the factory default (identity, spec §14.3);
                    // otherwise refuse so the user reboots to reach the factory
                    // default instead of establishing a bad Reset baseline.
                    if is_capture_untrusted(edid_id)
                        && !lut_is_identity(&captured, samples_per_channel, channels)
                    {
                        return Err(
                            "pre-apply pixtx state is not the driver's factory default (leftover \
                             from a prior ChromaDeck session); reboot to reach the factory \
                             default, then apply"
                                .into(),
                        );
                    }
                    clear_capture_untrusted(edid_id);
                    store_capture(
                        edid_id,
                        CapturedLut {
                            block_id,
                            samples_per_channel,
                            channels,
                            samples: captured,
                        },
                    )?;
                }
            }

            self.api.set_pipeline(output, &blocks)
        })();

        match result {
            Ok(()) => {
                gamma_applied = true;
                // Track which kind of pipeline is live so restore knows
                // whether a LUT capture is required.
                if wrote_lut {
                    clear_matrix_only(edid_id);
                } else {
                    mark_matrix_only(edid_id);
                }
            }
            Err(e) => append_error(&mut error, format!("intel: {e}")),
        }

        ApplyResult {
            icc_applied: false,
            gamma_applied,
            vibrance_applied: false,
            error,
        }
    }

    fn restore(&self, api: &dyn ColorApi, _nv: &dyn NvColorApi, edid_id: &str) -> ApplyResult {
        // Serialize the whole restore-default → re-SET → read-back sequence
        // against apply and other restores (process-global IGCL state).
        let _sequence = pipeline_lock();

        let mut gamma_applied = false;
        let mut error: Option<String> = None;

        if let Some(e) = gdi_identity_purge(api, edid_id) {
            append_error(&mut error, e);
        }

        // Restore needs the pre-apply capture (or a matrix-only apply) to know
        // what it may verifiably restore. Without either (app restarted with no
        // apply this session, or a poisoned table) we cannot verifiably return
        // the panel to its pre-apply state — report an honest failure, never a
        // fabricated success.
        let captured = match get_capture(edid_id) {
            Some(c) => Some(c),
            None if is_matrix_only(edid_id) => None, // matrix-only: no LUT was touched
            None => {
                // No capture and no matrix-only marker. Still best-effort reset
                // the matrix via RESTORE_DEFAULT (the one block it genuinely
                // resets), but the 1D LUT is NOT restored — do not claim success.
                let reset = self.api.resolve(edid_id).and_then(|o| self.api.restore_default(o));
                if let Err(e) = reset {
                    append_error(&mut error, format!("intel reset: {e}"));
                }
                append_error(
                    &mut error,
                    "intel reset: no pre-apply 1D LUT capture for this display in this session; \
                     cannot verifiably restore the panel (re-apply then reset, or reboot)"
                        .to_string(),
                );
                return ApplyResult {
                    icc_applied: false,
                    gamma_applied,
                    vibrance_applied: false,
                    error,
                };
            }
        };

        let result = (|| -> Result<(), String> {
            let output = self.api.resolve(edid_id)?;
            let info = self.api.probe(output)?;

            // RESTORE_DEFAULT is the only full reset (it also clears the 3D
            // LUT); it does NOT restore the 1D LUT (§13.4) and its matrix reset
            // is unverified. We therefore positively re-write the matrix to
            // identity (verification by construction — the matrix block cannot
            // be read back on this driver, spec §6/§10) and re-SET the captured
            // 1D LUT, which IS verified by read-back below.
            self.api.restore_default(output)?;
            let blocks = restore_blocks(&info, captured.as_ref());
            self.api.set_pipeline(output, &blocks)?;

            if let Some(c) = &captured {
                // Verify by read-back: only report success when the panel's
                // live LUT matches the captured pre-apply curve within epsilon.
                let read_back = self.api.read_1dlut(
                    output,
                    c.block_id,
                    c.samples_per_channel,
                    c.channels,
                )?;
                if !luts_match(&read_back, &c.samples) {
                    return Err(format!(
                        "restored 1D LUT read-back does not match the captured pre-apply curve (max delta {:.3e})",
                        max_delta(&read_back, &c.samples)
                    ));
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                gamma_applied = true;
                if captured.is_some() {
                    // Verified restore: drop the capture so a later apply
                    // re-captures the current (restored) state — never a stale
                    // pre-apply curve (C1).
                    clear_capture(edid_id);
                } else {
                    clear_matrix_only(edid_id);
                }
            }
            Err(e) => append_error(&mut error, format!("intel reset: {e}")),
        }

        ApplyResult {
            icc_applied: false,
            gamma_applied,
            vibrance_applied: false,
            error,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Transcribed struct sizes (pin the spec §10 hardware-verified layout)

    #[test]
    fn struct_sizes_match_hardware_proven_values() {
        use std::mem::size_of;
        assert_eq!(size_of::<ctl_init_args_t>(), 36);
        assert_eq!(size_of::<ctl_device_adapter_properties_t>(), 320);
        assert_eq!(size_of::<ctl_display_properties_t>(), 200);
        assert_eq!(size_of::<ctl_adapter_display_encoder_properties_t>(), 112);
        assert_eq!(size_of::<ctl_edid_management_args_t>(), 40);
        assert_eq!(size_of::<ctl_panel_descriptor_access_args_t>(), 32);
        assert_eq!(size_of::<ctl_display_timing_t>(), 64);
        assert_eq!(size_of::<ctl_os_display_encoder_identifier_t>(), 16);
        assert_eq!(size_of::<ctl_generic_void_datatype_t>(), 16);
        assert_eq!(size_of::<ctl_pixtx_pixel_format_t>(), 120);
        assert_eq!(size_of::<ctl_pixtx_1dlut_config_t>(), 40);
        assert_eq!(size_of::<ctl_pixtx_matrix_config_t>(), 128);
        assert_eq!(size_of::<ctl_pixtx_config_t>(), 128);
        assert_eq!(size_of::<ctl_pixtx_block_config_t>(), 144);
        assert_eq!(size_of::<ctl_pixtx_pipe_get_config_t>(), 272);
        assert_eq!(size_of::<ctl_pixtx_pipe_set_config_t>(), 32);
        assert_eq!(size_of::<ctl_display_feature_reset_t>(), 16);
    }

    // ── DLL loading degradation ─────────────────────────────────────────

    #[test]
    fn load_lib_nonexistent_returns_none() {
        assert!(load_lib("chromadeck_nonexistent_intel_xyz.dll").is_none());
    }

    #[test]
    fn failed_session_load_retries_after_backoff() {
        // A failed load is cached only until the backoff elapses, never for the
        // process lifetime.
        let now = Instant::now();
        assert!(!retry_due(now, now + SESSION_RETRY_BACKOFF));
        assert!(retry_due(now + SESSION_RETRY_BACKOFF, now));
        assert!(retry_due(now + SESSION_RETRY_BACKOFF + Duration::from_secs(1), now));
    }

    // ── Correlation: never guess ─────────────────────────────────────────

    #[test]
    fn correlate_matches_unique_output() {
        let outs = vec![Some("EDID:AAAA-0001-00000000".to_string()), Some("EDID:BBBB-0002-00000000".to_string())];
        assert_eq!(correlate(&outs, "EDID:BBBB-0002-00000000").unwrap(), 1);
    }

    #[test]
    fn correlate_no_match_is_err() {
        let outs = vec![Some("EDID:AAAA-0001-00000000".to_string())];
        assert!(correlate(&outs, "EDID:FFFF-FFFF-00000000").is_err());
    }

    #[test]
    fn correlate_ambiguous_is_err_never_guesses() {
        // Two identical-twin panels sharing one EDID: must refuse, not pick one.
        let outs = vec![
            Some("EDID:AAAA-0001-00000000".to_string()),
            Some("EDID:AAAA-0001-00000000".to_string()),
        ];
        let err = correlate(&outs, "EDID:AAAA-0001-00000000").unwrap_err();
        assert!(err.contains("ambiguous"), "got: {err}");
    }

    #[test]
    fn correlate_unreadable_edid_is_err() {
        // An active output with an unreadable EDID must never match by guess.
        let outs = vec![None];
        assert!(correlate(&outs, "EDID:AAAA-0001-00000000").is_err());
    }

    #[test]
    fn correlate_empty_is_err() {
        assert!(correlate(&[], "EDID:AAAA-0001-00000000").is_err());
    }
}

#[cfg(test)]
mod backend_tests {
    use super::*;
    use crate::color::TestRecorder;
    use crate::nvapi::MockNvapi;
    use crate::store::Preset;

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    fn test_preset() -> Preset {
        Preset {
            id: "p".into(),
            name: "p".into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness: 50.0,
            contrast: 50.0,
            rgb_gains: [1.0, 1.0, 1.0],
            gamma: 1.0,
            vibrance: 50.0,
            saturation: 50.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
        }
    }

    /// A healthy internal-panel capability: 3ch 1D LUT + both matrix blocks.
    fn healthy_probe() -> IgclProbe {
        IgclProbe {
            hdr: false,
            blocks: vec![
                IgclBlockInfo {
                    block_id: 327427,
                    block_type: CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS,
                    samples_per_channel: 0,
                    channels: 0,
                },
                IgclBlockInfo {
                    block_id: 327428,
                    block_type: CTL_PIXTX_BLOCK_TYPE_1D_LUT,
                    samples_per_channel: 2049,
                    channels: 3,
                },
                IgclBlockInfo {
                    block_id: 327429,
                    block_type: CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX,
                    samples_per_channel: 0,
                    channels: 0,
                },
            ],
        }
    }

    /// The AUO panel's native 3ch gamma-encode LUT (spec §13.4): a linear
    /// limited-range encode `v[i] = min(0.025 + 1.025·i/2048, 1.0)`.
    fn native_curve() -> Vec<f64> {
        (0..2049)
            .map(|i| (0.025 + 1.025 * i as f64 / 2048.0).min(1.0))
            .collect()
    }

    // ── Mock FFI (never loads the real DLL) ──────────────────────────────

    pub struct MockIgcl {
        pub resolve_err: Option<String>,
        pub probe: IgclProbe,
        pub fail_set: bool,
        pub fail_restore: bool,
        pub fail_read_lut: bool,
        /// When true, `set_pipeline` returns Ok but does NOT update `read_lut`
        /// (simulates an accepted-but-ineffective write — the exact failure
        /// mode of RESTORE_DEFAULT on the real driver). Atomic so tests can
        /// toggle it mid-flight through the shared `&'static` handle.
        pub set_lut_noop: std::sync::atomic::AtomicBool,
        /// The LUT that `read_1dlut` returns; `set_pipeline` updates it to the
        /// OneDLut block's samples (a faithful simulation of the live panel).
        pub read_lut: std::sync::Mutex<Vec<f64>>,
        pub calls: std::sync::Mutex<Vec<String>>,
        pub set_blocks: std::sync::Mutex<Vec<Vec<PixtxBlock>>>,
    }

    impl MockIgcl {
        pub fn new() -> Self {
            MockIgcl {
                resolve_err: None,
                probe: IgclProbe { hdr: false, blocks: vec![] },
                fail_set: false,
                fail_restore: false,
                fail_read_lut: false,
                set_lut_noop: std::sync::atomic::AtomicBool::new(false),
                read_lut: std::sync::Mutex::new(Vec::new()),
                calls: std::sync::Mutex::new(Vec::new()),
                set_blocks: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl IgclApi for MockIgcl {
        fn resolve(&self, edid_id: &str) -> Result<IgclOutput, String> {
            self.calls.lock().unwrap().push(format!("resolve:{edid_id}"));
            match &self.resolve_err {
                Some(e) => Err(e.clone()),
                None => Ok(IgclOutput(std::ptr::null_mut::<c_void>())),
            }
        }
        fn probe(&self, _output: IgclOutput) -> Result<IgclProbe, String> {
            self.calls.lock().unwrap().push("probe".to_string());
            Ok(self.probe.clone())
        }
        fn set_pipeline(&self, _output: IgclOutput, blocks: &[PixtxBlock]) -> Result<(), String> {
            self.calls.lock().unwrap().push("set_pipeline".to_string());
            self.set_blocks.lock().unwrap().push(blocks.to_vec());
            if !self.set_lut_noop.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(PixtxBlock::OneDLut { samples, .. }) = blocks
                    .iter()
                    .find(|b| matches!(b, PixtxBlock::OneDLut { .. }))
                {
                    *self.read_lut.lock().unwrap() = samples.clone();
                }
            }
            if self.fail_set {
                Err("mock set failure".into())
            } else {
                Ok(())
            }
        }
        fn restore_default(&self, _output: IgclOutput) -> Result<(), String> {
            self.calls.lock().unwrap().push("restore_default".to_string());
            if self.fail_restore {
                Err("mock restore failure".into())
            } else {
                Ok(())
            }
        }
        fn read_1dlut(
            &self,
            _output: IgclOutput,
            _block_id: u32,
            _samples_per_channel: u32,
            _channels: u32,
        ) -> Result<Vec<f64>, String> {
            self.calls.lock().unwrap().push("read_1dlut".to_string());
            if self.fail_read_lut {
                Err("mock read failure".into())
            } else {
                Ok(self.read_lut.lock().unwrap().clone())
            }
        }
    }

    fn backend(mock: MockIgcl) -> IntelBackend {
        let leaked: &'static MockIgcl = Box::leak(Box::new(mock));
        IntelBackend::with_api(leaked)
    }

    // ── UI-unit → normalized mapping ──────────────────────────────────────

    #[test]
    fn ui_to_gain_neutral_and_ends() {
        assert_close(ui_to_gain(50.0), 1.0);
        assert_close(ui_to_gain(0.0), 0.8);
        assert_close(ui_to_gain(100.0), 1.2);
    }

    #[test]
    fn saturation_factor_neutral_and_ends() {
        assert_eq!(saturation_factor(50.0), 1.0);
        assert_eq!(saturation_factor(0.0), 0.0);
        assert_eq!(saturation_factor(100.0), 2.0);
    }

    // ── 1D LUT golden values ──────────────────────────────────────────────

    #[test]
    fn gamma_lut_neutral_is_identity() {
        let lut = build_gamma_lut(2049, 1.0, 1.0, 1.0);
        assert_eq!(lut.len(), 2049);
        assert_eq!(lut[0], 0.0);
        assert_eq!(lut[2048], 1.0);
        assert_eq!(lut[1024], 0.5);
    }

    #[test]
    fn gamma_lut_gamma_golden_midpoint() {
        let lut = build_gamma_lut(2049, 1.0, 1.0, 2.2);
        assert_close(lut[1024], 0.5f64.powf(2.2));
        assert_eq!(lut[0], 0.0);
        assert_eq!(lut[2048], 1.0);
    }

    #[test]
    fn gamma_lut_brightness_golden() {
        let lut = build_gamma_lut(2049, 0.5, 1.0, 1.0);
        assert_eq!(lut[2048], 0.5);
        assert_eq!(lut[1024], 0.25);
    }

    #[test]
    fn gamma_lut_contrast_golden() {
        let lut = build_gamma_lut(2049, 1.0, 0.5, 1.0);
        assert_eq!(lut[0], 0.25);
        assert_eq!(lut[2048], 0.75);
        assert_eq!(lut[1024], 0.5);
    }

    // ── Matrix builders ───────────────────────────────────────────────────

    #[test]
    fn gains_matrix_is_diagonal() {
        let m = gains_matrix([0.5, 1.0, 2.0]);
        assert_eq!(m, [[0.5, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]]);
    }

    #[test]
    fn saturation_matrix_identity_at_neutral() {
        let m = saturation_matrix(1.0);
        assert_eq!(m, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn saturation_matrix_grayscale_at_zero() {
        let m = saturation_matrix(0.0);
        assert_eq!(
            m,
            [
                [LUMA_R, LUMA_G, LUMA_B],
                [LUMA_R, LUMA_G, LUMA_B],
                [LUMA_R, LUMA_G, LUMA_B],
            ]
        );
    }

    #[test]
    fn hue_rotation_identity_at_zero() {
        let m = hue_rotation_matrix(0.0);
        for i in 0..3 {
            for j in 0..3 {
                assert_close(m[i][j], if i == j { 1.0 } else { 0.0 });
            }
        }
    }

    #[test]
    fn hue_rotation_is_luma_preserving() {
        for deg in [0.0, 30.0, 90.0, 180.0, 359.0] {
            let m = hue_rotation_matrix(deg);
            for row in 0..3 {
                let sum: f64 = m[row].iter().sum();
                assert_close(sum, 1.0);
            }
        }
    }

    // ── Composition ───────────────────────────────────────────────────────

    #[test]
    fn compose_color_matrix_identity_at_neutral() {
        let m = compose_color_matrix([1.0, 1.0, 1.0], 50.0, 0.0);
        assert_eq!(m, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn compose_color_matrix_gains_only_is_diagonal() {
        let m = compose_color_matrix([0.5, 1.0, 1.0], 50.0, 0.0);
        assert_eq!(m, [[0.5, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn compose_pipeline_order_and_content() {
        // Non-neutral saturation keeps the matrix non-identity so both blocks
        // are emitted (an identity matrix is intentionally omitted).
        let mut p = test_preset();
        p.saturation = 65.0;
        let blocks = compose_pipeline(&p, &healthy_probe()).unwrap();
        assert_eq!(blocks.len(), 2, "one matrix + one 1D LUT");

        // Matrix first (linear position), targeting the AND_OFFSETS block.
        match &blocks[0] {
            PixtxBlock::Matrix { block_id, block_type, matrix } => {
                assert_eq!(*block_id, 327427);
                assert_eq!(*block_type, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS);
                assert_eq!(*matrix, compose_color_matrix(p.rgb_gains, p.saturation, p.hue_deg));
            }
            other => panic!("expected Matrix, got {other:?}"),
        }

        // 1D LUT second, 3 channels of 2049 samples (R,G,B identical).
        match &blocks[1] {
            PixtxBlock::OneDLut { block_id, samples_per_channel, channels, samples } => {
                assert_eq!(*block_id, 327428);
                assert_eq!(*samples_per_channel, 2049);
                assert_eq!(*channels, 3);
                assert_eq!(samples.len(), 2049 * 3);
                // Channel-uniform: R == G == B for every sample index.
                for i in 0..2049 {
                    assert_eq!(samples[i], samples[2049 + i]);
                    assert_eq!(samples[i], samples[2 * 2049 + i]);
                }
            }
            other => panic!("expected OneDLut, got {other:?}"),
        }
    }

    #[test]
    fn compose_pipeline_prefers_3ch_lut_over_1ch() {
        let mut info = healthy_probe();
        info.blocks.insert(
            0,
            IgclBlockInfo {
                block_id: 327426,
                block_type: CTL_PIXTX_BLOCK_TYPE_1D_LUT,
                samples_per_channel: 2049,
                channels: 1,
            },
        );
        let mut p = test_preset();
        p.saturation = 65.0; // keep the (non-identity) matrix block in the result
        let blocks = compose_pipeline(&p, &info).unwrap();
        // The 3ch block (id 327428) must win over the 1ch block (327426).
        match &blocks[1] {
            PixtxBlock::OneDLut { block_id, channels, .. } => {
                assert_eq!(*block_id, 327428);
                assert_eq!(*channels, 3);
            }
            other => panic!("expected OneDLut, got {other:?}"),
        }
    }

    fn matrix_only_probe() -> IgclProbe {
        IgclProbe {
            hdr: false,
            blocks: vec![IgclBlockInfo {
                block_id: 1,
                block_type: CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX,
                samples_per_channel: 0,
                channels: 0,
            }],
        }
    }

    fn lut_only_probe(samples_per_channel: u32) -> IgclProbe {
        IgclProbe {
            hdr: false,
            blocks: vec![IgclBlockInfo {
                block_id: 1,
                block_type: CTL_PIXTX_BLOCK_TYPE_1D_LUT,
                samples_per_channel,
                channels: 3,
            }],
        }
    }

    #[test]
    fn compose_pipeline_lut_only_when_matrix_is_absent_and_neutral() {
        // The display exposes no matrix block. A preset with no matrix-family
        // deviation (identity composed matrix) is delivered by the LUT alone.
        let blocks = compose_pipeline(&test_preset(), &lut_only_probe(2049)).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], PixtxBlock::OneDLut { .. }));
    }

    #[test]
    fn compose_pipeline_writes_identity_matrix_even_when_present() {
        // M-1: a neutral matrix must still be positively written so a prior
        // saturation/hue/gains apply (non-identity matrix) is neutralized.
        // SET_CUSTOM preserves unspecified blocks, so omitting the identity
        // would leave the old matrix active. The result is [identity Matrix,
        // 1D LUT].
        let blocks = compose_pipeline(&test_preset(), &healthy_probe()).unwrap();
        assert_eq!(blocks.len(), 2, "identity matrix + 1D LUT");
        match &blocks[0] {
            PixtxBlock::Matrix { block_id, block_type, matrix } => {
                assert_eq!(*block_id, 327427);
                assert_eq!(*block_type, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS);
                assert_eq!(*matrix, IDENTITY_3X3, "neutral preset writes the identity matrix");
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        assert!(matches!(blocks[1], PixtxBlock::OneDLut { .. }));
    }

    #[test]
    fn compose_pipeline_matrix_only_when_lut_is_absent() {
        // The display exposes no 1D LUT. A matrix-only preset (saturation)
        // is delivered by the matrix alone.
        let mut p = test_preset();
        p.saturation = 70.0;
        let blocks = compose_pipeline(&p, &matrix_only_probe()).unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            PixtxBlock::Matrix { matrix, .. } => {
                assert_eq!(*matrix, compose_color_matrix(p.rgb_gains, p.saturation, p.hue_deg));
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
    }

    #[test]
    fn compose_pipeline_rejects_matrix_adjustments_without_matrix_block() {
        let mut p = test_preset();
        p.saturation = 70.0;
        let err = compose_pipeline(&p, &lut_only_probe(2049)).unwrap_err();
        assert!(err.contains("no 3x3 matrix block"), "got: {err}");
    }

    #[test]
    fn compose_pipeline_rejects_gamma_adjustments_without_lut_block() {
        let mut p = test_preset();
        p.gamma = 2.2;
        let err = compose_pipeline(&p, &matrix_only_probe()).unwrap_err();
        assert!(err.contains("no 1D LUT block"), "got: {err}");
    }

    #[test]
    fn compose_pipeline_rejects_singleton_lut() {
        // samples == 1 would divide by zero in build_gamma_lut.
        let err = compose_pipeline(&test_preset(), &lut_only_probe(1)).unwrap_err();
        assert!(err.contains("at least 2 required"), "got: {err}");
    }

    #[test]
    fn compose_pipeline_no_writable_block_is_err() {
        let info = IgclProbe { hdr: false, blocks: vec![] };
        assert!(compose_pipeline(&test_preset(), &info).is_err());
    }

    // ── probe verdicts (via mock) ─────────────────────────────────────────

    #[test]
    fn probe_healthy_reports_expected_verdicts() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        let b = backend(mock);
        let caps = b.probe("EDID:X");
        let find = |n: &str| caps.iter().find(|c| c.adjustment == n).unwrap().cap;
        assert_eq!(find(ADJUSTMENT_GAMMA), AdjustmentCap::Supported);
        assert_eq!(find(ADJUSTMENT_BRIGHTNESS), AdjustmentCap::Supported);
        assert_eq!(find(ADJUSTMENT_CONTRAST), AdjustmentCap::Supported);
        assert_eq!(find(ADJUSTMENT_RGB_GAINS), AdjustmentCap::Supported);
        assert_eq!(find(ADJUSTMENT_SATURATION), AdjustmentCap::Supported);
        assert_eq!(find(ADJUSTMENT_HUE), AdjustmentCap::Supported);
        assert!(matches!(find(ADJUSTMENT_VIBRANCE), AdjustmentCap::Unsupported(_)));
    }

    #[test]
    fn probe_ambiguous_resolve_reports_all_unsupported() {
        let mut mock = MockIgcl::new();
        mock.resolve_err = Some("ambiguous: multiple Intel outputs share EDID X".into());
        let b = backend(mock);
        let caps = b.probe("EDID:X");
        for c in &caps {
            assert!(matches!(c.cap, AdjustmentCap::Unsupported(_)), "{}", c.adjustment);
        }
    }

    #[test]
    fn probe_hdr_reports_all_unsupported() {
        let mut mock = MockIgcl::new();
        mock.probe = IgclProbe { hdr: true, blocks: healthy_probe().blocks };
        let b = backend(mock);
        let caps = b.probe("EDID:X");
        for c in &caps {
            assert!(matches!(c.cap, AdjustmentCap::Unsupported(_)), "{}", c.adjustment);
        }
    }

    #[test]
    fn hdr_classification_uses_enabled_not_supported_flag() {
        // The bug: HDR was classified from FeatureSupportedFlags ("capable"),
        // so every HDR-capable panel running SDR was treated as HDR and the
        // whole backend was disabled. Fix: classify from FeatureEnabledFlags
        // ("currently enabled"). Supported-but-not-enabled → adjustments stay
        // available; enabled → unsupported.
        assert!(
            !hdr_enabled(0x0),
            "SDR (HDR enabled bit clear, even if the supported bit is set) → not HDR"
        );
        assert!(
            !hdr_enabled(0x1 | 0x2 | 0x4),
            "other enabled features with the HDR bit clear → not HDR"
        );
        assert!(
            hdr_enabled(CTL_STD_DISPLAY_FEATURE_FLAG_HDR),
            "HDR enabled bit set → HDR"
        );
        assert!(
            hdr_enabled(CTL_STD_DISPLAY_FEATURE_FLAG_HDR | 0x1),
            "HDR enabled alongside other features → HDR"
        );
    }

    #[test]
    fn hdr_from_props_reads_enabled_not_supported_flag() {
        // L-1: pin the production call site (`probe` → `hdr_from_props`). If a
        // revert reintroduces `FeatureSupportedFlags`, this test fails even
        // though the pure `hdr_enabled` helper still passes.
        let mut props = zeroed::<ctl_display_properties_t>();
        props.FeatureSupportedFlags = CTL_STD_DISPLAY_FEATURE_FLAG_HDR;
        props.FeatureEnabledFlags = 0;
        assert!(
            !hdr_from_props(&props),
            "HDR supported-but-not-enabled → not HDR (reads ENABLED, not SUPPORTED)"
        );

        props.FeatureEnabledFlags = CTL_STD_DISPLAY_FEATURE_FLAG_HDR;
        props.FeatureSupportedFlags = 0;
        assert!(hdr_from_props(&props), "HDR enabled (regardless of support) → HDR");
    }

    // ── apply / restore behavior ──────────────────────────────────────────

    #[test]
    fn apply_composes_and_sets_pipeline() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let mut preset = test_preset();
        preset.saturation = 65.0; // non-identity matrix → both blocks emitted

        let result = b.apply(&api, &nv, &preset, "EDID:apply-set");

        assert!(result.gamma_applied, "apply must succeed: {:?}", result.error);
        assert!(!result.vibrance_applied);
        assert!(result.error.is_none());
        // GDI purge was attempted (TestRecorder records a "gamma" write).
        assert_eq!(*api.calls.lock().unwrap(), vec!["gamma"]);
        // Exactly one set_pipeline with two blocks.
        let set_blocks = mock.set_blocks.lock().unwrap();
        assert_eq!(set_blocks.len(), 1);
        assert_eq!(set_blocks[0].len(), 2);
    }

    #[test]
    fn apply_matrix_only_pipeline_succeeds_without_lut_capture() {
        // Display exposes only a matrix block; the preset is gamma-neutral and
        // saturation-only, so a matrix-only pipeline must apply without trying
        // to capture a non-existent 1D LUT.
        let mut mock = MockIgcl::new();
        mock.probe = matrix_only_probe();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let mut preset = test_preset();
        preset.saturation = 70.0;

        let result = b.apply(&api, &nv, &preset, "EDID:matrix-only");

        assert!(result.gamma_applied, "matrix-only apply must succeed: {:?}", result.error);
        assert!(result.error.is_none());
        assert!(
            !mock.calls.lock().unwrap().iter().any(|c| c == "read_1dlut"),
            "no LUT capture for a matrix-only pipeline"
        );
        let set_blocks = mock.set_blocks.lock().unwrap();
        assert_eq!(set_blocks.len(), 1);
        assert_eq!(set_blocks[0].len(), 1);
        assert!(matches!(set_blocks[0][0], PixtxBlock::Matrix { .. }));
        drop(set_blocks);

        // Restore needs no LUT capture: the matrix-only apply never touched the
        // 1D LUT. Restore resets via RESTORE_DEFAULT AND positively re-writes
        // the identity matrix (M-1 — never rely on RESTORE_DEFAULT alone).
        let restore = b.restore(&api, &nv, "EDID:matrix-only");
        assert!(restore.gamma_applied, "matrix-only restore must succeed: {:?}", restore.error);
        assert!(restore.error.is_none());
        assert!(mock.calls.lock().unwrap().iter().any(|c| c == "restore_default"));
        // The restore re-SET must carry an explicit identity matrix.
        let set_blocks = mock.set_blocks.lock().unwrap();
        assert_eq!(set_blocks.len(), 2, "one apply SET + one restore re-SET");
        match &set_blocks[1][0] {
            PixtxBlock::Matrix { block_id, block_type, matrix } => {
                assert_eq!(*block_id, 1);
                assert_eq!(*block_type, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX);
                assert_eq!(*matrix, IDENTITY_3X3);
            }
            other => panic!("expected identity Matrix, got {other:?}"),
        }
    }

    #[test]
    fn apply_resolve_failure_reports_error_not_panic() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        mock.resolve_err = Some("no active Intel output correlates".into());
        let b = backend(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);
        let result = b.apply(&api, &nv, &test_preset(), "EDID:X");
        assert!(!result.gamma_applied);
        assert!(result.error.as_ref().unwrap().contains("no active Intel output"));
    }

    #[test]
    fn apply_capture_failure_skips_without_applying() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        mock.fail_read_lut = true;
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let result = b.apply(&api, &nv, &test_preset(), "EDID:capture-fail");

        assert!(!result.gamma_applied, "capture failure must not claim success");
        let err = result.error.as_ref().unwrap();
        assert!(err.contains("capture pre-apply 1D LUT failed"), "got: {err}");
        assert!(err.contains("refusing to apply unrestorable state"), "got: {err}");
        // No SET may have been attempted when we cannot restore.
        assert!(mock.set_blocks.lock().unwrap().is_empty(), "no apply SET on capture failure");
    }

    #[test]
    fn apply_captures_then_restore_resets_captured_lut() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        *mock.read_lut.lock().unwrap() = native_curve();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let mut preset = test_preset();
        preset.saturation = 65.0;

        let apply = b.apply(&api, &nv, &preset, "EDID:roundtrip");
        assert!(apply.gamma_applied, "apply must succeed: {:?}", apply.error);

        // After apply the mock panel reflects the applied (non-native) LUT.
        let applied = mock.read_lut.lock().unwrap().clone();
        assert_ne!(applied[0], native_curve()[0], "apply must change the 3ch LUT");

        let restore = b.restore(&api, &nv, "EDID:roundtrip");
        assert!(restore.gamma_applied, "restore must succeed: {:?}", restore.error);
        assert!(restore.error.is_none());

        // The mock panel's LUT must be back to the captured native curve.
        let restored = mock.read_lut.lock().unwrap().clone();
        assert_eq!(restored, native_curve(), "restore must re-SET the captured pre-apply LUT");

        // The restore path must have issued a re-SET (not only RESTORE_DEFAULT):
        // an explicit identity matrix + the captured 1D LUT (M-1).
        let set_blocks = mock.set_blocks.lock().unwrap();
        assert_eq!(set_blocks.len(), 2, "one apply SET + one restore re-SET");
        assert_eq!(
            set_blocks[1].len(),
            2,
            "restore re-SETs the identity matrix + captured 1D LUT"
        );
        match &set_blocks[1][0] {
            PixtxBlock::Matrix { block_id, block_type, matrix } => {
                assert_eq!(*block_id, 327427);
                assert_eq!(*block_type, CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS);
                assert_eq!(*matrix, IDENTITY_3X3, "restore neutralizes the matrix by construction");
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        match &set_blocks[1][1] {
            PixtxBlock::OneDLut { samples, .. } => {
                assert_eq!(*samples, native_curve(), "restore re-SETs the captured native curve");
            }
            other => panic!("expected OneDLut, got {other:?}"),
        }
    }

    // ── C1: capture must not be overwritten by a second apply ───────────

    #[test]
    fn second_apply_does_not_overwrite_capture_and_restore_returns_to_first_preapply() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        *mock.read_lut.lock().unwrap() = native_curve();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        // Apply A (gamma 2.2) captures the native pre-apply state.
        let mut a = test_preset();
        a.gamma = 2.2;
        assert!(b.apply(&api, &nv, &a, "EDID:ab").gamma_applied, "apply A must succeed");
        let lut_a = mock.read_lut.lock().unwrap().clone();

        // Apply B (gamma 3.0) must NOT overwrite the native capture.
        let mut b2 = test_preset();
        b2.gamma = 3.0;
        assert!(b.apply(&api, &nv, &b2, "EDID:ab").gamma_applied, "apply B must succeed");
        let lut_b = mock.read_lut.lock().unwrap().clone();
        assert_ne!(lut_a, lut_b, "A and B must produce different LUTs");
        assert_eq!(
            get_capture("EDID:ab").unwrap().samples,
            native_curve(),
            "a second apply must not overwrite the first pre-apply capture"
        );

        // Restore must return to A's pre-apply state (native), not B's curve.
        let restore = b.restore(&api, &nv, "EDID:ab");
        assert!(restore.gamma_applied, "restore must succeed: {:?}", restore.error);
        assert_eq!(
            mock.read_lut.lock().unwrap().clone(),
            native_curve(),
            "restore must return to the FIRST pre-apply state, not B's"
        );
    }

    #[test]
    fn verified_restore_clears_capture_so_a_later_apply_recaptures() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        *mock.read_lut.lock().unwrap() = native_curve();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let mut preset = test_preset();
        preset.gamma = 2.2;
        assert!(b.apply(&api, &nv, &preset, "EDID:clr").gamma_applied);
        assert!(get_capture("EDID:clr").is_some(), "apply must capture");

        // Restore verifies by read-back and, on success, clears the capture.
        assert!(b.restore(&api, &nv, "EDID:clr").gamma_applied, "restore must succeed");
        assert!(get_capture("EDID:clr").is_none(), "a verified restore must clear the capture");

        // A subsequent apply re-captures the current (restored) state — native.
        let mut preset2 = test_preset();
        preset2.gamma = 1.5;
        assert!(b.apply(&api, &nv, &preset2, "EDID:clr").gamma_applied);
        assert!(get_capture("EDID:clr").is_some(), "apply after restore must re-capture");
        assert_eq!(
            get_capture("EDID:clr").unwrap().samples,
            native_curve(),
            "re-capture is the restored (native) state"
        );
    }

    #[test]
    fn restore_without_capture_reports_honest_failure() {
        let mock = MockIgcl::new();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let result = b.restore(&api, &nv, "EDID:nocapture");

        assert!(!result.gamma_applied, "must NOT claim success without a capture");
        let err = result.error.as_ref().unwrap();
        assert!(err.contains("no pre-apply 1D LUT capture"), "got: {err}");
    }

    #[test]
    fn restore_read_back_mismatch_reports_failure() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        *mock.read_lut.lock().unwrap() = native_curve();
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        // Apply captures `native` and writes the applied (identity) LUT.
        let apply = b.apply(&api, &nv, &test_preset(), "EDID:mismatch");
        assert!(apply.gamma_applied);

        // Simulate an accepted-but-ineffective restore write: the driver
        // returns Ok but the panel keeps the applied (non-native) LUT.
        mock.set_lut_noop.store(true, std::sync::atomic::Ordering::Relaxed);

        let restore = b.restore(&api, &nv, "EDID:mismatch");

        assert!(!restore.gamma_applied, "must NOT claim success on a mismatched read-back");
        let err = restore.error.as_ref().unwrap();
        assert!(err.contains("read-back"), "got: {err}");
    }

    #[test]
    fn restore_reports_error_when_reset_write_fails() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        *mock.read_lut.lock().unwrap() = native_curve();
        mock.fail_restore = true;
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        let apply = b.apply(&api, &nv, &test_preset(), "EDID:resetfail");
        assert!(apply.gamma_applied);

        let restore = b.restore(&api, &nv, "EDID:resetfail");
        assert!(!restore.gamma_applied);
        assert!(restore.error.as_ref().unwrap().contains("mock restore failure"));
    }

    // ── M-4: untrusted pre-apply capture (restart-without-reboot) ─────────

    fn lut_3ch(curve: &[f64]) -> Vec<f64> {
        let mut v = Vec::with_capacity(curve.len() * 3);
        for _ in 0..3 {
            v.extend_from_slice(curve);
        }
        v
    }

    #[test]
    fn apply_refuses_when_untrusted_capture_is_non_identity() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        // The panel carries a NON-identity leftover (native limited-range curve).
        *mock.read_lut.lock().unwrap() = lut_3ch(&native_curve());
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        clear_capture_untrusted("EDID:m4-refuse");
        mark_capture_untrusted("EDID:m4-refuse");

        let mut preset = test_preset();
        preset.gamma = 2.2;

        let result = b.apply(&api, &nv, &preset, "EDID:m4-refuse");

        assert!(!result.gamma_applied, "untrusted non-identity capture must refuse");
        let err = result.error.as_ref().unwrap();
        assert!(err.contains("reboot"), "refusal must direct the user to reboot: {err}");
        assert!(
            mock.set_blocks.lock().unwrap().is_empty(),
            "no SET when refusing an untrusted baseline"
        );
        assert!(get_capture("EDID:m4-refuse").is_none(), "no poisoned baseline is stored");
    }

    #[test]
    fn apply_allows_when_untrusted_capture_is_identity() {
        let mut mock = MockIgcl::new();
        mock.probe = healthy_probe();
        // The panel is verifiably factory (identity), e.g. after a reboot.
        *mock.read_lut.lock().unwrap() = lut_3ch(&build_gamma_lut(2049, 1.0, 1.0, 1.0));
        let mock: &'static MockIgcl = Box::leak(Box::new(mock));
        let b = IntelBackend::with_api(mock);
        let api = TestRecorder::new(true);
        let nv = MockNvapi::new(false);

        clear_capture_untrusted("EDID:m4-allow");
        mark_capture_untrusted("EDID:m4-allow");

        let mut preset = test_preset();
        preset.gamma = 2.2;

        let result = b.apply(&api, &nv, &preset, "EDID:m4-allow");

        assert!(result.gamma_applied, "identity (factory) capture must be allowed: {:?}", result.error);
        assert!(
            !is_capture_untrusted("EDID:m4-allow"),
            "a verified-factory capture clears the untrusted mark for this session"
        );
        // The stored baseline is the factory identity, not a leftover curve.
        let capture = get_capture("EDID:m4-allow").expect("capture stored");
        assert!(lut_is_identity(&capture.samples, capture.samples_per_channel, capture.channels));
    }

    // ── L-4: negative correlation cache TTL ──────────────────────────────

    #[test]
    fn negative_hit_expires_after_ttl() {
        let mut entries = HashMap::new();
        let now = Instant::now();
        entries.insert("E".to_string(), now + NEGATIVE_CORRELATION_TTL);
        assert!(negative_hit(&entries, "E", now), "fresh negative verdict is a hit");
        assert!(
            negative_hit(&entries, "E", now + NEGATIVE_CORRELATION_TTL - Duration::from_secs(1)),
            "still within TTL"
        );
        assert!(!negative_hit(&entries, "E", now + NEGATIVE_CORRELATION_TTL), "TTL elapsed");
        assert!(!negative_hit(&entries, "OTHER", now), "absent EDID is not a hit");
    }
}

// ── Hardware verification (real Intel panel only — never NVIDIA HDMI) ───────
//
// `#[ignore]`-gated: run manually on a machine with the Intel eDP panel, e.g.
//   cargo test --manifest-path src-tauri\Cargo.toml -- \
//     --ignored hw_verify_round_trip_restore_returns_to_captured \
//     --nocapture --test-threads=1
//
// IGCL enumerates only the Intel adapter's outputs, so `resolve` can never
// touch the NVIDIA HDMI display. The test resolves the internal panel by EDID,
// captures its pre-apply LUT, applies a mild saturation, verifies the LUT
// changed, restores, and verifies the LUT is byte-close to the capture.
#[cfg(test)]
mod hw_verify {
    use super::*;
    use crate::color::RealColorApi;
    use crate::nvapi::RealNvapi;
    use crate::store::Preset;

    /// Internal panel EDID id (AUO F7AD) — the only active Intel output on
    /// this machine (spec §2/§13.2).
    const INTERNAL_EDID: &str = "EDID:06AF-F7AD-00000000";

    fn sat65_preset() -> Preset {
        Preset {
            id: "hw".into(),
            name: "hw".into(),
            icc_hash: String::new(),
            icc_filename: String::new(),
            brightness: 50.0,
            contrast: 50.0,
            rgb_gains: [1.0, 1.0, 1.0],
            // Non-neutral gamma perturbs the 1D LUT (the capture/restore target)
            // even when the panel's pre-apply LUT is already identity.
            gamma: 2.2,
            vibrance: 50.0,
            saturation: 65.0,
            hue_deg: 0.0,
            color_model: "nvcp-v1".into(),
            color_tag: None,
            tag_shape: None,
        }
    }

    /// Unwind guard: if the test panics mid-flight, re-apply the captured
    /// pre-apply LUT so the panel is never left modified.
    struct HwRestoreGuard {
        output: IgclOutput,
        lut: PixtxBlock,
        dirty: bool,
    }

    impl Drop for HwRestoreGuard {
        fn drop(&mut self) {
            if !self.dirty {
                return;
            }
            let _ = REAL_IGCL.restore_default(self.output);
            let _ = REAL_IGCL.set_pipeline(self.output, std::slice::from_ref(&self.lut));
            eprintln!("[hw-verify] unwind guard: forced restore to captured pre-apply LUT");
        }
    }

    #[test]
    #[ignore]
    fn hw_verify_round_trip_restore_returns_to_captured() {
        // Fresh session state: no capture yet.
        captured_luts().lock().unwrap().clear();

        let backend = IntelBackend::new();
        let api = RealColorApi;
        let nv = RealNvapi;

        // ── No-capture restore must report an honest failure, not success.
        let no_cap = backend.restore(&api, &nv, INTERNAL_EDID);
        assert!(!no_cap.gamma_applied, "no-capture restore must NOT claim success");
        assert!(no_cap.error.as_ref().unwrap().contains("no pre-apply 1D LUT capture"));
        eprintln!(
            "[hw-verify] no-capture restore -> gamma_applied={} error={:?}",
            no_cap.gamma_applied, no_cap.error
        );

        // ── Resolve the internal panel and read its pre-apply LUT.
        let output = REAL_IGCL.resolve(INTERNAL_EDID).expect("resolve internal Intel panel");
        let info = REAL_IGCL.probe(output).expect("probe internal panel");
        let lut = info
            .blocks
            .iter()
            .filter(|b| b.block_type == CTL_PIXTX_BLOCK_TYPE_1D_LUT)
            .max_by_key(|b| b.channels)
            .copied()
            .expect("3ch 1D LUT block");

        let before = REAL_IGCL
            .read_1dlut(output, lut.block_id, lut.samples_per_channel, lut.channels)
            .expect("read pre-apply LUT");
        let per = lut.samples_per_channel as usize;
        eprintln!("[hw-verify] BEFORE       R[0..4] = {:?}", &before[0..4.min(per)]);
        eprintln!("[hw-verify] BEFORE       R[last-2..] = {:?}", &before[per - 2..per]);

        // ── Apply mild saturation (65), then confirm the LUT changed.
        let preset = sat65_preset();
        let apply = backend.apply(&api, &nv, &preset, INTERNAL_EDID);
        assert!(apply.gamma_applied, "apply must succeed: {:?}", apply.error);

        let mut guard = HwRestoreGuard {
            output,
            lut: PixtxBlock::OneDLut {
                block_id: lut.block_id,
                samples_per_channel: lut.samples_per_channel,
                channels: lut.channels,
                samples: before.clone(),
            },
            dirty: true,
        };

        let after = REAL_IGCL
            .read_1dlut(output, lut.block_id, lut.samples_per_channel, lut.channels)
            .expect("read post-apply LUT");
        eprintln!("[hw-verify] AFTER-APPLY  R[0..4] = {:?}", &after[0..4.min(per)]);
        assert!(
            max_delta(&after, &before) > LUT_VERIFY_EPSILON,
            "apply must change the 3ch LUT"
        );

        // ── Restore and verify the panel is byte-close to the capture.
        let restore = backend.restore(&api, &nv, INTERNAL_EDID);
        assert!(restore.gamma_applied, "restore must succeed: {:?}", restore.error);

        let after_restore = REAL_IGCL
            .read_1dlut(output, lut.block_id, lut.samples_per_channel, lut.channels)
            .expect("read post-restore LUT");
        eprintln!("[hw-verify] AFTER-RESTORE R[0..4] = {:?}", &after_restore[0..4.min(per)]);
        let delta = max_delta(&after_restore, &before);
        eprintln!("[hw-verify] max_delta(after_restore, before) = {delta:.3e}");
        assert!(
            luts_match(&after_restore, &before),
            "restored LUT must match the capture (max delta {delta:.3e})"
        );

        // Verified restore: disarm the unwind guard.
        guard.dirty = false;
    }
}
