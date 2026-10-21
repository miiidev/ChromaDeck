// ============================================================================
// intel-igcl-spike — throwaway research probe for ChromaDeck Phase E1.
//
// Purpose: resolve the single biggest project risk — how Intel IGCL display
// handles (ctl_display_output_handle_t) correlate to Windows displays
// (\\.\DISPLAYn / EDID), which ChromaDeck keys on via `m.edid_id`.
//
// This crate is intentionally NOT a member of any Cargo workspace and is NOT
// referenced by src-tauri. It only ever READS driver state (ctlEnumerate*,
// ctlGetDisplayProperties, ctlEdidManagement READ, ctlPixelTransformationGet
// Config). It never calls a mutating API.
//
// Intel IGCL structures below are transcribed from the public Intel SDK header
// `igcl_api.h` (https://github.com/intel/drivers.gpu.control-library, file
// include/igcl_api.h, "Copyright (C) 2025 Intel Corporation", Intel Software
// License Agreement 10.07.21 — header redistribution permitted with notice).
// Only the subset actually used is transcribed; names are preserved verbatim
// (including the upstream `ctl_pixtx_config_opertaion_type_t` /
// `CTL_PIXTX_CONFIG_OPERTAION_TYPE_*` spelling). No Intel binaries are reverse
// engineered — only exported symbols + the public header are used.
// ============================================================================

#![allow(non_camel_case_types, dead_code, non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;

use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

// ============================================================================
// IGCL FFI layer (transcribed minimal subset of igcl_api.h)
// ============================================================================

type ctl_result_t = u32;
type ctl_version_info_t = u32;
type ctl_init_flags_t = u32;

type ctl_api_handle_t = *mut c_void;
type ctl_device_adapter_handle_t = *mut c_void;
type ctl_display_output_handle_t = *mut c_void;

const CTL_RESULT_SUCCESS: ctl_result_t = 0x0000_0000;
const CTL_RESULT_ERROR_INSUFFICIENT_PIXTX_BLOCK_CONFIG_MEMORY: ctl_result_t = 0x4800_000d;

// ctl_edid_management_optype_t
const CTL_EDID_MANAGEMENT_OPTYPE_READ_EDID: u32 = 1;
// ctl_edid_type_t
const CTL_EDID_TYPE_CURRENT: u32 = 1;
// ctl_edid_management_out_flag_t
const CTL_EDID_MANAGEMENT_OUT_FLAG_SUPPLIED_EDID: u32 = 1 << 1;
const CTL_EDID_MANAGEMENT_OUT_FLAG_MONITOR_EDID: u32 = 1 << 2;
const CTL_EDID_MANAGEMENT_OUT_FLAG_DISPLAY_CONNECTED: u32 = 1 << 3;

// ctl_display_config_flag_t
const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE: u32 = 1 << 0;
const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED: u32 = 1 << 1;

// ctl_encoder_config_flag_t
const CTL_ENCODER_CONFIG_FLAG_INTERNAL_DISPLAY: u32 = 1 << 0;
const CTL_ENCODER_CONFIG_FLAG_VIRTUAL_DISPLAY: u32 = 1 << 5;
const CTL_ENCODER_CONFIG_FLAG_HIDDEN_DISPLAY: u32 = 1 << 6;

// ctl_pixtx_config_query_type_t
const CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY: u32 = 0;
const CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT: u32 = 1;

// ctl_pixtx_block_type_t
const CTL_PIXTX_BLOCK_TYPE_1D_LUT: u32 = 1;
const CTL_PIXTX_BLOCK_TYPE_3D_LUT: u32 = 2;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX: u32 = 3;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS: u32 = 4;

const fn ctl_make_version(major: u32, minor: u32) -> u32 {
    (major << 16) | (minor & 0x0000_ffff)
}

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
    AppVersion: ctl_version_info_t,
    flags: ctl_init_flags_t,
    SupportedVersion: ctl_version_info_t,
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

// --- pixtx ---

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

#[repr(C)]
struct ctl_display_feature_reset_t {
    Size: u32,
    Version: u8,
    ResetFeature: u32,
    ResetFeatureStatus: u32,
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

// Function pointer types (calling convention: __cdecl / extern "C", identical to
// "system" on x64). Signatures transcribed from igcl_api.h.
type FnCtlInit = unsafe extern "C" fn(*mut ctl_init_args_t, *mut ctl_api_handle_t) -> ctl_result_t;
type FnCtlClose = unsafe extern "C" fn(ctl_api_handle_t) -> ctl_result_t;
type FnCtlEnumerateDevices =
    unsafe extern "C" fn(ctl_api_handle_t, *mut u32, *mut ctl_device_adapter_handle_t) -> ctl_result_t;
type FnCtlEnumerateDisplayOutputs =
    unsafe extern "C" fn(ctl_device_adapter_handle_t, *mut u32, *mut ctl_display_output_handle_t) -> ctl_result_t;
type FnCtlGetDeviceProperties =
    unsafe extern "C" fn(ctl_device_adapter_handle_t, *mut ctl_device_adapter_properties_t) -> ctl_result_t;
type FnCtlGetDisplayProperties =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_display_properties_t) -> ctl_result_t;
type FnCtlGetAdaperDisplayEncoderProperties =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_adapter_display_encoder_properties_t) -> ctl_result_t;
type FnCtlEdidManagement =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_edid_management_args_t) -> ctl_result_t;
type FnCtlPixelTransformationGetConfig =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_pixtx_pipe_get_config_t) -> ctl_result_t;
type FnCtlPixelTransformationSetConfig =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_pixtx_pipe_set_config_t) -> ctl_result_t;
type FnCtlDisplayFeatureReset =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_display_feature_reset_t) -> ctl_result_t;
type FnCtlPanelDescriptorAccess =
    unsafe extern "C" fn(ctl_display_output_handle_t, *mut ctl_panel_descriptor_access_args_t) -> ctl_result_t;

struct Igcl {
    path: String,
    _lib: HMODULE,
    init: FnCtlInit,
    close: FnCtlClose,
    enumerate_devices: FnCtlEnumerateDevices,
    enumerate_display_outputs: FnCtlEnumerateDisplayOutputs,
    get_device_properties: FnCtlGetDeviceProperties,
    get_display_properties: FnCtlGetDisplayProperties,
    get_adapter_display_encoder_properties: FnCtlGetAdaperDisplayEncoderProperties,
    edid_management: FnCtlEdidManagement,
    pixtx_get_config: FnCtlPixelTransformationGetConfig,
    // Optional (present in header but not necessarily exported by this driver).
    pixtx_set_config: Option<FnCtlPixelTransformationSetConfig>,
    display_feature_reset: Option<FnCtlDisplayFeatureReset>,
    set_brightness: Option<unsafe extern "C" fn()>,
    panel_descriptor_access: Option<FnCtlPanelDescriptorAccess>,
}

/// Candidate DLL paths, in probe order. The real IGCL runtime DLLs live in the
/// DriverStore alongside their dependencies (IntelControlLib.dll /
/// intel_gfx_api-x64.dll / ze_loader.dll ...), so they must be loaded by full
/// path for the OS loader to resolve siblings. Bare-name fallbacks last.
fn candidate_dll_paths() -> Vec<String> {
    let mut paths = Vec::new();
    let names = ["ControlLib.dll", "IntelControlLib.dll", "intel_gfx_api-x64.dll"];
    let repo = std::path::Path::new(r"C:\Windows\System32\DriverStore\FileRepository");
    if let Ok(entries) = std::fs::read_dir(repo) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let dn = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
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

impl Igcl {
    unsafe fn load() -> Option<Self> {
        for path in candidate_dll_paths() {
            let lib = match load_lib(&path) {
                Some(h) => h,
                None => {
                    let err = windows::Win32::Foundation::GetLastError();
                    println!("[IGCL] load_lib FAILED (err {}): {}", err.0, path);
                    continue;
                }
            };
            match Self::build(path.clone(), lib) {
                Some(igcl) => return Some(igcl),
                None => {
                    println!("[IGCL] loaded but symbol resolve FAILED: {}", path);
                    let _ = FreeLibrary(lib);
                }
            }
        }
        None
    }

    unsafe fn build(path: String, lib: HMODULE) -> Option<Self> {
        // Resolve the exported symbols. Any failure frees the library (done by
        // the caller via the `need!` early-return path).
        macro_rules! need {
            ($e:expr) => {
                match $e {
                    Some(v) => v,
                    None => return None,
                }
            };
        }

        Some(Igcl {
            path,
            _lib: lib,
            init: need!(resolve(lib, "ctlInit")),
            close: need!(resolve(lib, "ctlClose")),
            enumerate_devices: need!(resolve(lib, "ctlEnumerateDevices")),
            enumerate_display_outputs: need!(resolve(lib, "ctlEnumerateDisplayOutputs")),
            get_device_properties: need!(resolve(lib, "ctlGetDeviceProperties")),
            get_display_properties: need!(resolve(lib, "ctlGetDisplayProperties")),
            get_adapter_display_encoder_properties: need!(resolve(lib, "ctlGetAdaperDisplayEncoderProperties")),
            edid_management: need!(resolve(lib, "ctlEdidManagement")),
            pixtx_get_config: need!(resolve(lib, "ctlPixelTransformationGetConfig")),
            pixtx_set_config: resolve(lib, "ctlPixelTransformationSetConfig"),
            display_feature_reset: resolve(lib, "ctlDisplayFeatureReset"),
            set_brightness: resolve(lib, "ctlSetBrightnessSetting"),
            panel_descriptor_access: resolve(lib, "ctlPanelDescriptorAccess"),
        })
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn load_lib(name: &str) -> Option<HMODULE> {
    let w = wide(name);
    let h = match LoadLibraryW(PCWSTR::from_raw(w.as_ptr())) {
        Ok(h) => h,
        Err(_) => return None,
    };
    if h.is_invalid() {
        None
    } else {
        Some(h)
    }
}

unsafe fn resolve<F>(lib: HMODULE, name: &str) -> Option<F> {
    let mut b = name.as_bytes().to_vec();
    b.push(0);
    let ptr = GetProcAddress(lib, PCSTR::from_raw(b.as_ptr()))?;
    Some(std::mem::transmute_copy(&ptr))
}

fn ctl_result_name(r: ctl_result_t) -> &'static str {
    match r {
        0x0000_0000 => "CTL_RESULT_SUCCESS",
        0x4000_0001 => "CTL_RESULT_ERROR_NOT_INITIALIZED",
        0x4000_0002 => "CTL_RESULT_ERROR_ALREADY_INITIALIZED",
        0x4000_0003 => "CTL_RESULT_ERROR_DEVICE_LOST",
        0x4000_0004 => "CTL_RESULT_ERROR_OUT_OF_HOST_MEMORY",
        0x4000_0006 => "CTL_RESULT_ERROR_INSUFFICIENT_PERMISSIONS",
        0x4000_0007 => "CTL_RESULT_ERROR_NOT_AVAILABLE",
        0x4000_0008 => "CTL_RESULT_ERROR_UNINITIALIZED",
        0x4000_0009 => "CTL_RESULT_ERROR_UNSUPPORTED_VERSION",
        0x4000_000a => "CTL_RESULT_ERROR_UNSUPPORTED_FEATURE",
        0x4000_000b => "CTL_RESULT_ERROR_INVALID_ARGUMENT",
        0x4000_000c => "CTL_RESULT_ERROR_INVALID_API_HANDLE",
        0x4000_000d => "CTL_RESULT_ERROR_INVALID_NULL_HANDLE",
        0x4000_000e => "CTL_RESULT_ERROR_INVALID_NULL_POINTER",
        0x4000_000f => "CTL_RESULT_ERROR_INVALID_SIZE",
        0x4000_0010 => "CTL_RESULT_ERROR_UNSUPPORTED_SIZE",
        0x4000_0014 => "CTL_RESULT_ERROR_DATA_NOT_FOUND",
        0x4000_0015 => "CTL_RESULT_ERROR_NOT_IMPLEMENTED",
        0x4000_0016 => "CTL_RESULT_ERROR_OS_CALL",
        0x4000_0017 => "CTL_RESULT_ERROR_KMD_CALL",
        0x4000_001a => "CTL_RESULT_ERROR_INVALID_OPERATION_TYPE",
        0x4800_0003 => "CTL_RESULT_ERROR_DISPLAY_NOT_ATTACHED",
        0x4800_0004 => "CTL_RESULT_ERROR_DISPLAY_NOT_ACTIVE",
        0x4800_0007 => "CTL_RESULT_ERROR_INVALID_PIXTX_GET_CONFIG_QUERY_TYPE",
        0x4800_0008 => "CTL_RESULT_ERROR_INVALID_PIXTX_SET_CONFIG_OPERATION_TYPE",
        0x4800_000a => "CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_ID",
        0x4800_000b => "CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_TYPE",
        0x4800_000d => "CTL_RESULT_ERROR_INSUFFICIENT_PIXTX_BLOCK_CONFIG_MEMORY",
        0x4800_0010 => "CTL_RESULT_ERROR_3DLUT_NOT_SUPPORTED_IN_HDR",
        0x4800_0011 => "CTL_RESULT_ERROR_3DLUT_INVALID_OPERATION",
        0x4800_0012 => "CTL_RESULT_ERROR_3DLUT_UNSUCCESSFUL",
        _ => "CTL_RESULT_<unknown>",
    }
}

fn result_str(r: ctl_result_t) -> String {
    format!("0x{r:08X} ({})", ctl_result_name(r))
}

fn cstr(buf: &[i8]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).to_string()
}

fn wstr(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

impl Drop for Igcl {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self._lib);
        }
    }
}

// ============================================================================
// Windows-side enumeration
// ============================================================================

#[derive(Clone)]
struct WinAdapter {
    index: u32,
    name: String,        // \\.\DISPLAYn
    device_string: String,
    device_id: String,   // PCI\VEN_... or MONITOR\...
    state_flags: u32,
}

#[derive(Clone)]
struct WinPath {
    source_luid_low: u32,
    source_luid_high: i32,
    source_id: u32,
    target_id: u32,
    output_technology: i32,
    monitor_path: String,     // \\?\DISPLAY#... PNP device path
    source_gdi_name: String,  // \\.\DISPLAYn (GDI source device name)
    friendly: String,
    edid_mfr: u16,
    edid_prod: u16,
}

fn output_tech_name(t: i32) -> &'static str {
    match t {
        -2147483648 => "INTERNAL",
        0 => "VGA",
        1 => "SVIDEO",
        4 => "DVI",
        5 => "HDMI",
        6 => "LVDS",
        10 => "DISPLAYPORT_EXTERNAL",
        11 => "DISPLAYPORT_EMBEDDED",
        16 => "INDIRECT_WIRED",
        17 => "INDIRECT_VIRTUAL",
        _ => "OTHER",
    }
}

fn enum_display_devices() -> Vec<WinAdapter> {
    use windows::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};

    let mut out = Vec::new();
    for i in 0..64u32 {
        let mut adapter = DISPLAY_DEVICEW::default();
        adapter.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        let ok = unsafe { EnumDisplayDevicesW(None, i, &mut adapter, 0) };
        if !ok.as_bool() {
            break;
        }
        let name = wstr(&adapter.DeviceName);
        if name.is_empty() {
            continue;
        }
        out.push(WinAdapter {
            index: i,
            name,
            device_string: wstr(&adapter.DeviceString),
            device_id: wstr(&adapter.DeviceID),
            state_flags: adapter.StateFlags.0,
        });
    }
    out
}

fn query_display_config() -> Vec<WinPath> {
    use windows::Win32::Devices::Display::{
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    };

    let mut out = Vec::new();
    let flags = QDC_ONLY_ACTIVE_PATHS;

    let mut num_paths: u32 = 0;
    let mut num_modes: u32 = 0;
    let sz = unsafe { GetDisplayConfigBufferSizes(flags, &mut num_paths, &mut num_modes) };
    if sz.0 != 0 || num_paths == 0 {
        println!("  [windows] GetDisplayConfigBufferSizes failed/empty (err {}, paths {})", sz.0, num_paths);
        return out;
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); num_paths as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); num_modes as usize];
    let r = unsafe {
        QueryDisplayConfig(
            flags,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            None,
        )
    };
    if r.0 != 0 {
        println!("  [windows] QueryDisplayConfig failed (err {})", r.0);
        return out;
    }

    for p in paths.iter().take(num_paths as usize) {
        let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
        name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        name.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        name.header.adapterId = p.targetInfo.adapterId;
        name.header.id = p.targetInfo.id;
        let gr = unsafe { DisplayConfigGetDeviceInfo(&mut name.header) };
        let monitor_path = if gr == 0 {
            wstr(&name.monitorDevicePath)
        } else {
            format!("<GetDeviceInfo err {gr}>")
        };
        let friendly = if gr == 0 {
            wstr(&name.monitorFriendlyDeviceName)
        } else {
            String::new()
        };
        // Source GDI device name (\\.\DISPLAYn) via GET_SOURCE_NAME.
        let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
        src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        src.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        src.header.adapterId = p.sourceInfo.adapterId;
        src.header.id = p.sourceInfo.id;
        let sr = unsafe { DisplayConfigGetDeviceInfo(&mut src.header) };
        let source_gdi_name = if sr == 0 {
            wstr(&src.viewGdiDeviceName)
        } else {
            format!("<GetSourceName err {sr}>")
        };
        out.push(WinPath {
            source_luid_low: p.sourceInfo.adapterId.LowPart,
            source_luid_high: p.sourceInfo.adapterId.HighPart,
            source_id: p.sourceInfo.id,
            target_id: p.targetInfo.id,
            output_technology: p.targetInfo.outputTechnology.0,
            monitor_path,
            source_gdi_name,
            friendly,
            edid_mfr: name.edidManufactureId,
            edid_prod: name.edidProductCodeId,
        });
    }
    out
}

// ============================================================================
// EDID identity derivation (mirrors ChromaDeck src-tauri/src/monitor.rs
// `identify_monitor` so the printed id matches ChromaDeck's `edid_id`).
// ============================================================================

fn parse_edid_serial(edid: &[u8]) -> Option<String> {
    for off in [0x36usize, 0x48, 0x5A, 0x6C] {
        if edid.get(off) != Some(&0x00) || edid.get(off + 1) != Some(&0x00) {
            continue;
        }
        if edid.get(off + 3) == Some(&0xFF) {
            let raw = edid.get(off + 4..off + 18)?;
            let s: String = raw
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect();
            let t = s.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn parse_edid_model(edid: &[u8]) -> Option<String> {
    for off in [0x36usize, 0x48, 0x5A, 0x6C] {
        if edid.get(off) != Some(&0x00) || edid.get(off + 1) != Some(&0x00) {
            continue;
        }
        if edid.get(off + 3) == Some(&0xFC) {
            let raw = edid.get(off + 4..off + 18)?;
            let s: String = raw
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect();
            let t = s.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn edid_identity(edid: &[u8]) -> (String, String) {
    if edid.len() < 16 {
        return ("<no-edid>".into(), "<no-edid>".into());
    }
    let serial = parse_edid_serial(edid).unwrap_or_default();
    let model = parse_edid_model(edid).unwrap_or_default();
    if !serial.is_empty() {
        return (serial, model);
    }
    let mfr = u16::from_be_bytes([edid[8], edid[9]]);
    let prod = u16::from_le_bytes([edid[10], edid[11]]);
    let ser = u32::from_le_bytes([edid[12], edid[13], edid[14], edid[15]]);
    let id = format!("EDID:{mfr:04X}-{prod:04X}-{ser:08X}");
    (id, model)
}

// ============================================================================
// IGCL enumeration + dump
// ============================================================================

unsafe fn igcl_run(igcl: &Igcl, adapters: &[WinAdapter], paths: &[WinPath]) {
    // Print transcribed struct sizes for the spec (self-check vs header).
    println!("[sizes] ctl_init_args_t={} ctl_device_adapter_properties_t={} ctl_display_properties_t={}",
        std::mem::size_of::<ctl_init_args_t>(),
        std::mem::size_of::<ctl_device_adapter_properties_t>(),
        std::mem::size_of::<ctl_display_properties_t>());
    println!("[sizes] ctl_adapter_display_encoder_properties_t={} ctl_edid_management_args_t={}",
        std::mem::size_of::<ctl_adapter_display_encoder_properties_t>(),
        std::mem::size_of::<ctl_edid_management_args_t>());
    println!("[sizes] ctl_pixtx_pipe_get_config_t={} ctl_pixtx_block_config_t={} ctl_pixtx_matrix_config_t={} ctl_pixtx_1dlut_config_t={}",
        std::mem::size_of::<ctl_pixtx_pipe_get_config_t>(),
        std::mem::size_of::<ctl_pixtx_block_config_t>(),
        std::mem::size_of::<ctl_pixtx_matrix_config_t>(),
        std::mem::size_of::<ctl_pixtx_1dlut_config_t>());

    let mut init_args = std::mem::zeroed::<ctl_init_args_t>();
    init_args.Size = std::mem::size_of::<ctl_init_args_t>() as u32;
    init_args.Version = 0;
    init_args.AppVersion = ctl_make_version(1, 1);
    init_args.flags = 0;
    init_args.ApplicationUID = ctl_application_id_t {
        Data1: 0,
        Data2: 0,
        Data3: 0,
        Data4: [0; 8],
    };

    let mut h_api: ctl_api_handle_t = std::ptr::null_mut();
    let init_res = (igcl.init)(&mut init_args, &mut h_api);
    println!("\n[ctlInit] -> {}", result_str(init_res));
    if init_res != CTL_RESULT_SUCCESS || h_api.is_null() {
        println!("[ctlInit] BLOCKED — IGCL init failed; cannot enumerate. Stopping IGCL path.");
        return;
    }
    println!("[ctlInit] SupportedVersion = 0x{:08X}", init_args.SupportedVersion);

    // Enumerate device adapters.
    let mut dev_count: u32 = 0;
    let r = (igcl.enumerate_devices)(h_api, &mut dev_count, std::ptr::null_mut());
    println!("[ctlEnumerateDevices count] -> {} (count={})", result_str(r), dev_count);
    if r != CTL_RESULT_SUCCESS || dev_count == 0 {
        (igcl.close)(h_api);
        return;
    }
    let mut devices = vec![std::ptr::null_mut::<c_void>(); dev_count as usize];
    let r = (igcl.enumerate_devices)(h_api, &mut dev_count, devices.as_mut_ptr());
    println!("[ctlEnumerateDevices] -> {} ({} devices)", result_str(r), dev_count);
    if r != CTL_RESULT_SUCCESS {
        (igcl.close)(h_api);
        return;
    }

    for (di, dev) in devices.iter().enumerate() {
        let mut props = std::mem::zeroed::<ctl_device_adapter_properties_t>();
        props.Size = std::mem::size_of::<ctl_device_adapter_properties_t>() as u32;
        let r = (igcl.get_device_properties)(*dev, &mut props);
        if r != CTL_RESULT_SUCCESS {
            println!("\n[device {di}] ctlGetDeviceProperties -> {}", result_str(r));
            continue;
        }
        let (luid_low, luid_high) = if props.device_id_size >= 8 && !props.pDeviceID.is_null() {
            let b = std::slice::from_raw_parts(props.pDeviceID as *const u8, 8);
            let low = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let high = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            (low, high)
        } else {
            (0u32, 0i32)
        };
        let integrated = (props.graphics_adapter_properties & 1) != 0;
        println!("\n[device {di}] name=\"{}\" pci={:04X}:{:04X} rev={:02X} integrated={} device_id_size={}",
            cstr(&props.name), props.pci_vendor_id, props.pci_device_id, props.rev_id,
            integrated, props.device_id_size);
        println!("[device {di}] adapter LUID(pDeviceID) = {:08X}:{:08X}", luid_high as u32, luid_low);
        // Correlate to EnumDisplayDevices adapter.
        for a in adapters {
            let is_intel = a.device_id.contains("8086");
            let is_nvidia = a.device_id.contains("10DE");
            if (is_intel && props.pci_vendor_id == 0x8086) || (is_nvidia && props.pci_vendor_id == 0x10DE) {
                println!("[device {di}]   EnumDisplayDevices adapter \"{}\" id=\"{}\" (vendor match)", a.name, a.device_id);
            }
        }

        // Enumerate display outputs for this adapter.
        let mut out_count: u32 = 0;
        let r = (igcl.enumerate_display_outputs)(*dev, &mut out_count, std::ptr::null_mut());
        println!("[device {di}] ctlEnumerateDisplayOutputs count -> {} (count={})", result_str(r), out_count);
        if r != CTL_RESULT_SUCCESS || out_count == 0 {
            continue;
        }
        let mut outputs = vec![std::ptr::null_mut::<c_void>(); out_count as usize];
        let r = (igcl.enumerate_display_outputs)(*dev, &mut out_count, outputs.as_mut_ptr());
        if r != CTL_RESULT_SUCCESS {
            println!("[device {di}] ctlEnumerateDisplayOutputs -> {}", result_str(r));
            continue;
        }

        for (oi, out) in outputs.iter().enumerate() {
            dump_output(igcl, di, oi, *out, paths);
        }
    }

    (igcl.close)(h_api);
    println!("\n[ctlClose] done");
}

unsafe fn dump_output(igcl: &Igcl, di: usize, oi: usize, out: ctl_display_output_handle_t, paths: &[WinPath]) {
    let mut props = std::mem::zeroed::<ctl_display_properties_t>();
    props.Size = std::mem::size_of::<ctl_display_properties_t>() as u32;
    let r = (igcl.get_display_properties)(out, &mut props);
    if r != CTL_RESULT_SUCCESS {
        println!("\n[output {di}:{oi}] ctlGetDisplayProperties -> {}", result_str(r));
        return;
    }

    let enc = props.Os_display_encoder_handle.WindowsDisplayEncoderID;
    let active = (props.DisplayConfigFlags & CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE) != 0;
    let attached = (props.DisplayConfigFlags & CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED) != 0;
    let t = &props.Display_Timing_Info;

    println!("\n[output {di}:{oi}] WindowsDisplayEncoderID = 0x{enc:08X} ({enc})");
    println!("[output {di}:{oi}]   Type={} MuxType={} PConOut={} ConfigFlags=0x{:X} (active={} attached={})",
        props.Type, props.AttachedDisplayMuxType, props.ProtocolConverterOutput,
        props.DisplayConfigFlags, active, attached);
    println!("[output {di}:{oi}]   timing {}x{} @ {:.2}Hz pixelclk={}Hz",
        t.HActive, t.VActive, t.RefreshRate, t.PixelClock);
    println!("[output {di}:{oi}]   FeatureSupported=0x{:X} AdvancedSupported=0x{:X}",
        props.FeatureSupportedFlags, props.AdvancedFeatureSupportedFlags);

    // Encoder properties (INTERNAL_DISPLAY flag lives here).
    let mut encprops = std::mem::zeroed::<ctl_adapter_display_encoder_properties_t>();
    encprops.Size = std::mem::size_of::<ctl_adapter_display_encoder_properties_t>() as u32;
    let er = (igcl.get_adapter_display_encoder_properties)(out, &mut encprops);
    if er == CTL_RESULT_SUCCESS {
        let internal = (encprops.EncoderConfigFlags & CTL_ENCODER_CONFIG_FLAG_INTERNAL_DISPLAY) != 0;
        let virtual_ = (encprops.EncoderConfigFlags & CTL_ENCODER_CONFIG_FLAG_VIRTUAL_DISPLAY) != 0;
        let hidden = (encprops.EncoderConfigFlags & CTL_ENCODER_CONFIG_FLAG_HIDDEN_DISPLAY) != 0;
        println!("[output {di}:{oi}]   EncoderConfigFlags=0x{:X} (internal={} virtual={} hidden={})",
            encprops.EncoderConfigFlags, internal, virtual_, hidden);
    } else {
        println!("[output {di}:{oi}]   ctlGetAdaperDisplayEncoderProperties -> {}", result_str(er));
    }

    // Windows correlation: WindowsDisplayEncoderID == QueryDisplayConfig target id?
    let matched = paths.iter().find(|p| p.target_id == enc);
    if let Some(mp) = matched {
        println!("[output {di}:{oi}]   CORRELATE: QDC target id 0x{enc:08X} -> srcGdi=\"{}\" source LUID {:08X}:{:08X}, tech={}, monitor=\"{}\" friendly=\"{}\"",
            mp.source_gdi_name, mp.source_luid_high as u32, mp.source_luid_low,
            output_tech_name(mp.output_technology), mp.monitor_path, mp.friendly);
    } else {
        println!("[output {di}:{oi}]   CORRELATE: no QDC active path with target id 0x{enc:08X} (inactive/attached-only output)");
    }

    // EDID read via ctlEdidManagement (READ_EDID, two-pass).
    let mut args = std::mem::zeroed::<ctl_edid_management_args_t>();
    args.Size = std::mem::size_of::<ctl_edid_management_args_t>() as u32;
    args.OpType = CTL_EDID_MANAGEMENT_OPTYPE_READ_EDID;
    args.EdidType = CTL_EDID_TYPE_CURRENT;
    args.EdidSize = 0;
    args.pEdidBuf = std::ptr::null_mut();
    let r1 = (igcl.edid_management)(out, &mut args);
    if r1 == CTL_RESULT_SUCCESS && args.EdidSize > 0 {
        let mut buf = vec![0u8; args.EdidSize as usize];
        args.pEdidBuf = buf.as_mut_ptr();
        let r2 = (igcl.edid_management)(out, &mut args);
        if r2 == CTL_RESULT_SUCCESS {
            let (edid_id, model) = edid_identity(&buf);
            println!("[output {di}:{oi}]   EDID ({}) bytes -> edid_id=\"{}\" model=\"{}\" outflags=0x{:X} (monitor_edid={} supplied_edid={})",
                buf.len(), edid_id, model, args.OutFlags,
                (args.OutFlags & CTL_EDID_MANAGEMENT_OUT_FLAG_MONITOR_EDID) != 0,
                (args.OutFlags & CTL_EDID_MANAGEMENT_OUT_FLAG_SUPPLIED_EDID) != 0);
            // mfr/product cross-check vs DisplayConfigGetDeviceInfo fields.
            if let Some(mp) = matched {
                let mfr = u16::from_be_bytes([buf[8], buf[9]]);
                let prod = u16::from_le_bytes([buf[10], buf[11]]);
                println!("[output {di}:{oi}]   EDID bytes[8..12] mfr=0x{mfr:04X} prod=0x{prod:04X} | QDC target name mfr=0x{:04X} prod=0x{:04X} {}",
                    mp.edid_mfr, mp.edid_prod,
                    if mfr == mp.edid_mfr && prod == mp.edid_prod { "(MATCH)" } else { "(MISMATCH)" });
            }
        } else {
            println!("[output {di}:{oi}]   ctlEdidManagement READ (pass2) -> {}", result_str(r2));
        }
    } else {
        println!("[output {di}:{oi}]   ctlEdidManagement READ (pass1) -> {} (size={})", result_str(r1), args.EdidSize);
        // Alternate EDID/DisplayID path: ctlPanelDescriptorAccess.
        if let Some(pda) = igcl.panel_descriptor_access {
            let mut pargs = std::mem::zeroed::<ctl_panel_descriptor_access_args_t>();
            pargs.Size = std::mem::size_of::<ctl_panel_descriptor_access_args_t>() as u32;
            pargs.OpType = 1; // READ
            pargs.BlockNumber = 0;
            pargs.DescriptorDataSize = 0;
            pargs.pDescriptorData = std::ptr::null_mut();
            let pr1 = (pda)(out, &mut pargs);
            if pr1 == CTL_RESULT_SUCCESS && pargs.DescriptorDataSize > 0 {
                let mut pbuf = vec![0u8; pargs.DescriptorDataSize as usize];
                pargs.pDescriptorData = pbuf.as_mut_ptr();
                let pr2 = (pda)(out, &mut pargs);
                if pr2 == CTL_RESULT_SUCCESS && pbuf.len() >= 16 {
                    let (pid, pmodel) = edid_identity(&pbuf);
                    println!("[output {di}:{oi}]   ctlPanelDescriptorAccess READ -> {} bytes, edid_id=\"{}\" model=\"{}\"",
                        pbuf.len(), pid, pmodel);
                } else {
                    println!("[output {di}:{oi}]   ctlPanelDescriptorAccess READ (pass2) -> {}", result_str(pr2));
                }
            } else {
                println!("[output {di}:{oi}]   ctlPanelDescriptorAccess READ (pass1) -> {} (size={})", result_str(pr1), pargs.DescriptorDataSize);
            }
        } else {
            println!("[output {di}:{oi}]   ctlPanelDescriptorAccess symbol NOT exported by this driver.");
        }
    }

    // Pixel transformation capability + current read-back.
    dump_pixtx(igcl, di, oi, out);
}

unsafe fn dump_pixtx(igcl: &Igcl, di: usize, oi: usize, out: ctl_display_output_handle_t) {
    // Pass 1: get NumBlocks.
    let mut get = std::mem::zeroed::<ctl_pixtx_pipe_get_config_t>();
    get.Size = std::mem::size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
    get.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY;
    get.pBlockConfigs = std::ptr::null_mut();
    let r1 = (igcl.pixtx_get_config)(out, &mut get);
    let cap_res = r1; // may be SUCCESS or INSUFFICIENT_PIXTX_BLOCK_CONFIG_MEMORY
    let nblocks = get.NumBlocks as usize;
    println!("[output {di}:{oi}]   pixtx CAPABILITY query -> {} (NumBlocks={})", result_str(cap_res), nblocks);
    if nblocks == 0 {
        return;
    }
    println!("[output {di}:{oi}]   pixtx InputPixelFormat  ColorModel={} ColorSpace={} Encoding={} BPC={} IsFloat={}",
        get.InputPixelFormat.ColorModel, get.InputPixelFormat.ColorSpace,
        get.InputPixelFormat.EncodingType, get.InputPixelFormat.BitsPerColor, get.InputPixelFormat.IsFloat);
    println!("[output {di}:{oi}]   pixtx OutputPixelFormat ColorModel={} ColorSpace={} Encoding={} BPC={} IsFloat={}",
        get.OutputPixelFormat.ColorModel, get.OutputPixelFormat.ColorSpace,
        get.OutputPixelFormat.EncodingType, get.OutputPixelFormat.BitsPerColor, get.OutputPixelFormat.IsFloat);

    // Pass 2: allocate blocks, set Size, query capability details.
    let mut blocks: Vec<ctl_pixtx_block_config_t> = (0..nblocks)
        .map(|_| {
            let mut b = std::mem::zeroed::<ctl_pixtx_block_config_t>();
            b.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
            b
        })
        .collect();
    get.pBlockConfigs = blocks.as_mut_ptr();
    get.NumBlocks = nblocks as u32;
    let r2 = (igcl.pixtx_get_config)(out, &mut get);
    println!("[output {di}:{oi}]   pixtx CAPABILITY details -> {} (NumBlocks={})", result_str(r2), get.NumBlocks);

    for (bi, b) in blocks.iter().enumerate() {
        match b.BlockType {
            CTL_PIXTX_BLOCK_TYPE_1D_LUT => {
                let c = &b.Config.OneDLutConfig;
                println!("[output {di}:{oi}]     block[{bi}] id={} 1D_LUT NumSamplesPerChannel={} NumChannels={} SamplingType={}",
                    b.BlockId, c.NumSamplesPerChannel, c.NumChannels, c.SamplingType);
            }
            CTL_PIXTX_BLOCK_TYPE_3D_LUT => {
                let c = &b.Config.ThreeDLutConfig;
                println!("[output {di}:{oi}]     block[{bi}] id={} 3D_LUT NumSamplesPerChannel={}",
                    b.BlockId, c.NumSamplesPerChannel);
            }
            CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX | CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS => {
                let c = &b.Config.MatrixConfig;
                println!("[output {di}:{oi}]     block[{bi}] id={} 3X3_MATRIX{} matrix=[{:?}] pre={:?} post={:?}",
                    b.BlockId,
                    if b.BlockType == CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS { "_AND_OFFSETS" } else { "" },
                    c.Matrix, c.PreOffsets, c.PostOffsets);
            }
            other => println!("[output {di}:{oi}]     block[{bi}] id={} BlockType={}", b.BlockId, other),
        }
    }

    // CURRENT read-back: per-block, one block at a time (mirrors the Intel
    // Color_Sample_App.cpp pattern: QueryType=CURRENT + pBlockConfigs=&one block
    // + NumBlocks=1). Passing all blocks at once for CURRENT is rejected with
    // CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_TYPE by this driver.
    for (bi, cap) in blocks.iter().enumerate() {
        match cap.BlockType {
            CTL_PIXTX_BLOCK_TYPE_1D_LUT => {
                let mut b = std::mem::zeroed::<ctl_pixtx_block_config_t>();
                b.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
                b.BlockId = cap.BlockId;
                b.BlockType = cap.BlockType;
                let n = cap.Config.OneDLutConfig.NumSamplesPerChannel as usize
                    * cap.Config.OneDLutConfig.NumChannels.max(1) as usize;
                let mut samples = vec![0.0f64; n.max(1)];
                b.Config.OneDLutConfig.SamplingType = cap.Config.OneDLutConfig.SamplingType;
                b.Config.OneDLutConfig.NumSamplesPerChannel = cap.Config.OneDLutConfig.NumSamplesPerChannel;
                b.Config.OneDLutConfig.NumChannels = cap.Config.OneDLutConfig.NumChannels;
                b.Config.OneDLutConfig.pSampleValues = samples.as_mut_ptr();

                let mut c = std::mem::zeroed::<ctl_pixtx_pipe_get_config_t>();
                c.Size = std::mem::size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
                c.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT;
                c.NumBlocks = 1;
                c.pBlockConfigs = &mut b;
                let r = (igcl.pixtx_get_config)(out, &mut c);
                let ch = cap.Config.OneDLutConfig.NumChannels.max(1) as usize;
                let per = cap.Config.OneDLutConfig.NumSamplesPerChannel as usize;
                let mut preview = String::new();
                for k in 0..(per.min(3)) {
                    preview.push_str(&format!(" {:.5}", samples.get(k).copied().unwrap_or(0.0)));
                }
                println!("[output {di}:{oi}]     CURRENT 1D_LUT block[{bi}] -> {} samples/ch={} channels={} R[0..]{}...",
                    result_str(r), per, ch, preview);
            }
            CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX | CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS => {
                let mut b = std::mem::zeroed::<ctl_pixtx_block_config_t>();
                b.Size = std::mem::size_of::<ctl_pixtx_block_config_t>() as u32;
                b.BlockId = cap.BlockId;
                b.BlockType = cap.BlockType;

                let mut c = std::mem::zeroed::<ctl_pixtx_pipe_get_config_t>();
                c.Size = std::mem::size_of::<ctl_pixtx_pipe_get_config_t>() as u32;
                c.QueryType = CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT;
                c.NumBlocks = 1;
                c.pBlockConfigs = &mut b;
                let r = (igcl.pixtx_get_config)(out, &mut c);
                let m = b.Config.MatrixConfig;
                println!("[output {di}:{oi}]     CURRENT matrix block[{bi}] -> {} matrix={:?} pre={:?} post={:?}",
                    result_str(r), m.Matrix, m.PreOffsets, m.PostOffsets);
            }
            CTL_PIXTX_BLOCK_TYPE_3D_LUT => {
                println!("[output {di}:{oi}]     CURRENT 3D_LUT block[{bi}] skipped (17^3 samples; count known from capability)");
            }
            _ => {}
        }
    }
}

// ============================================================================
// main
// ============================================================================

fn main() {
    println!("=== intel-igcl-spike: IGCL display handle <-> Windows display correlation ===");

    println!("\n--- Windows EnumDisplayDevicesW (adapters) ---");
    let adapters = enum_display_devices();
    for a in &adapters {
        println!("  [{}] \"{}\" str=\"{}\" id=\"{}\" state=0x{:X}",
            a.index, a.name, a.device_string, a.device_id, a.state_flags);
    }

    println!("\n--- Windows QueryDisplayConfig (active paths) ---");
    let paths = query_display_config();
    for p in &paths {
        println!("  target_id=0x{:08X} srcId={} srcLuid={:08X}:{:08X} srcGdi=\"{}\" tech={} monitor=\"{}\" friendly=\"{}\" mfr=0x{:04X} prod=0x{:04X}",
            p.target_id, p.source_id, p.source_luid_high as u32, p.source_luid_low,
            p.source_gdi_name, output_tech_name(p.output_technology), p.monitor_path, p.friendly,
            p.edid_mfr, p.edid_prod);
    }

    println!("\n--- Intel IGCL ---");
    match unsafe { Igcl::load() } {
        None => {
            println!("[IGCL] FAILED to load ControlLib.dll / IntelControlLib.dll.");
        }
        Some(igcl) => {
            println!("[IGCL] loaded \"{}\" and resolved all symbols.", igcl.path);
            println!("[IGCL] API surface availability: pixtx_set_config={} display_feature_reset={} set_brightness={} panel_descriptor_access={}",
                igcl.pixtx_set_config.is_some(), igcl.display_feature_reset.is_some(),
                igcl.set_brightness.is_some(), igcl.panel_descriptor_access.is_some());
            unsafe { igcl_run(&igcl, &adapters, &paths) };
        }
    }

    println!("\n=== done ===");
}
