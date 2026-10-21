# Intel IGCL display-handle → Windows display correlation (Phase E1 spike)

- **Date:** 2026-10-19
- **Status:** COMPLETE — correlation risk resolved with measured evidence. No blocking unknowns remain.
- **Spike crate:** `spikes/intel-igcl/` (standalone cargo bin, NOT a workspace member, NOT referenced by `src-tauri`). Full raw output saved to `spikes/intel-igcl/RUN_OUTPUT.txt`.
- **Reproduce:** `cargo run --manifest-path spikes\intel-igcl\Cargo.toml`

## 1. Verdict

**Deterministic correlation WORKS. The fallback rule from the plan is not needed for the common case, but is retained as a safety net.**

Two independent, converging correlation chains were proven on hardware:

1. **Encoder-ID chain (always available):** `ctl_display_properties_t.Os_display_encoder_handle.WindowsDisplayEncoderID` **is exactly** `QueryDisplayConfig`'s `DISPLAYCONFIG_PATH_TARGET_INFO.id` (the vidpn target id). That maps an Intel output handle to a Windows display path, and from there to `\\.\DISPLAYn` (via `DisplayConfigGetDeviceInfo(GET_SOURCE_NAME)`) and to the adapter LUID.
2. **EDID chain (preferred, matches ChromaDeck's key):** `ctlPanelDescriptorAccess(output, OpType=READ, BlockNumber=0)` returns the raw 128-byte EDID, which parses to ChromaDeck's exact `edid_id` via the same logic in `monitor.rs::identify_monitor`.

There is **no guesswork** and **no ordering assumption** (see §5).

## 2. Which adapter drives what on this machine (MUX/hybrid)

| Display | Adapter | QDC source LUID (LowPart) | GDI name | Technology | EDID mfr/prod |
|---|---|---|---|---|---|
| Internal panel (AUO, 1920×1080 @ 144 Hz) | **Intel** UHD (8086:A7A8) | `0x1331F` | `\\.\DISPLAY1` | INTERNAL | AUO `0x06AF` / `0xF7AD` |
| External HDMI (AOC 24B36XE) | **NVIDIA** RTX 3050 (10DE:25AC) | `0x13725` | `\\.\DISPLAY5` | HDMI | `0xE305` / `0x2403` |

- The internal panel is the **only active Intel-attached output** (IGCL reports exactly one output with `CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE`, and it carries `CTL_ENCODER_CONFIG_FLAG_INTERNAL_DISPLAY`).
- The HDMI port is **wired to the NVIDIA dGPU** — it does **not** appear anywhere in IGCL's 10 enumerated outputs. ChromaDeck's existing NVAPI path already handles it.
- Consequence: on this machine the new Intel backend applies **only to the internal panel**. This is a standard Optimus layout (eDP → iGPU, HDMI → dGPU), not a MUX-switchable panel.

## 3. Measured evidence (excerpt — full dump in RUN_OUTPUT.txt)

```
Windows EnumDisplayDevicesW (adapters)
  [0] "\\.\DISPLAY1" Intel(R) UHD Graphics  PCI\VEN_8086&DEV_A7A8... state=0x1
  [4] "\\.\DISPLAY5" NVIDIA GeForce RTX 3050 ... PCI\VEN_10DE&DEV_25AC... state=0x5

Windows QueryDisplayConfig (active paths)
  target_id=0x00800050 srcId=0 srcLuid=0x1331F srcGdi="\\.\DISPLAY1" tech=INTERNAL  monitor=AUOF7AD mfr=0xAF06 prod=0xF7AD
  target_id=0x00001100 srcId=0 srcLuid=0x13725 srcGdi="\\.\DISPLAY5" tech=HDMI      monitor=AOC2403 mfr=0xE305 prod=0x2403

Intel IGCL
  loaded "...\DriverStore\...\iigd_dch.inf_amd64_d2c6999664e89333\ControlLib.dll"
  API surface: pixtx_set_config=true display_feature_reset=false set_brightness=true panel_descriptor_access=true
  ctlInit -> SUCCESS, SupportedVersion = 0x00010001
  ctlEnumerateDevices -> 1 device: "Intel(R) UHD Graphics" pci=8086:A7A8 integrated=true device_id_size=0
  ctlEnumerateDisplayOutputs -> 10 outputs

  [output 0:0] WindowsDisplayEncoderID = 0x00800050
    ConfigFlags=0x3 (active+attached)  EncoderConfigFlags=0x1 (INTERNAL_DISPLAY)
    timing 1920x1080 @ 144.00Hz pixelclk=356130000Hz
    FeatureSupported=0xC (PSR|VRR; NO HDR bit)
    CORRELATE: QDC target 0x00800050 -> srcGdi "\\.\DISPLAY1" LUID 0x1331F tech=INTERNAL
    ctlEdidManagement READ -> CTL_RESULT_ERROR_KMD_CALL (0x40000017)      <-- fails on eDP
    ctlPanelDescriptorAccess READ -> 128 bytes, edid_id="EDID:06AF-F7AD-00000000"  <-- works

    pixtx CAPABILITY (NumBlocks=5):
      block[0] id=327425 3D_LUT NumSamplesPerChannel=17
      block[1] id=327426 1D_LUT NumSamplesPerChannel=2049 NumChannels=1
      block[2] id=327427 3X3_MATRIX_AND_OFFSETS
      block[3] id=327428 1D_LUT NumSamplesPerChannel=2049 NumChannels=3
      block[4] id=327429 3X3_MATRIX
      InputPixelFormat  ColorModel=0(RGB_FR) ColorSpace=0(REC709) Encoding=0(SRGB) BPC=0
      OutputPixelFormat ColorModel=1(RGB_LR) ColorSpace=0 Encoding=0(SRGB) BPC=8
    CURRENT 1D_LUT block[1] (1ch): 0.00000 0.00004 0.00008 ...   (sRGB decode/degamma curve)
    CURRENT 1D_LUT block[3] (3ch): 0.02500 0.02550 0.02600 ...   (output gamma encode)
    CURRENT matrix -> CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_TYPE (0x4800000B)
```

## 4. Chosen correlation algorithm (pseudocode)

ChromaDeck keys everything on `edid_id` (`monitor.rs`), and already resolves `edid_id → device_name` (`\\.\DISPLAYn`). The Intel backend adds a `device_name / edid_id → ctl_display_output_handle_t` resolver:

```
// Preferred (EDID chain) — used for the primary per-display handle resolution.
fn intel_output_for_edid(edid_id) -> Option<handle>:
    for each Intel adapter (ctlEnumerateDevices):
        for each output (ctlEnumerateDisplayOutputs):
            props = ctlGetDisplayProperties(output)
            if not (props.DisplayConfigFlags & CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE): continue
            edid = ctlPanelDescriptorAccess(output, OpType=READ, BlockNumber=0)
            if edid_id_from_edid(edid) == edid_id:   # monitor.rs::identify_monitor logic
                return output

// Fallback (encoder-ID chain) — resolves device_name (\\.\DISPLAYn) when the
// EDID read is unavailable (e.g. a future panel where PanelDescriptorAccess fails).
fn intel_output_for_device_name(device_name) -> Option<handle>:
    qdc = QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)
    target_id = find path in qdc where
        DisplayConfigGetDeviceInfo(GET_SOURCE_NAME, path.sourceInfo).viewGdiDeviceName == device_name
    for each Intel output:
        props = ctlGetDisplayProperties(output)
        if props.Os_display_encoder_handle.WindowsDisplayEncoderID == target_id:
            return output
```

Safety invariants (never guess):
- Filter by `DISPLAY_ACTIVE` (attached-but-inactive outputs have `WindowsDisplayEncoderID` values that do **not** match any active QDC path — proven: the 9 inactive outputs' IDs match nothing).
- Verify the resolved QDC path's `sourceInfo.adapterId` LUID belongs to the Intel adapter before applying. (IGCL does **not** return the adapter LUID — `pDeviceID` is NULL, `device_id_size=0` — so match Intel-ness via PCI id from `ctlGetDeviceProperties`/`EnumDisplayDevices` adapter `DeviceID`, and via the fact that the active Intel output's encoder ID resolved to a QDC path whose source LUID is Intel's.)
- If the resolved target is not the **sole** active Intel output, or the encoder-ID chain is ambiguous, fall back to the plan's rule: apply only when unambiguous, else report `Unsupported(reason)`.

## 5. Ordering / identity findings

- `\\.\DISPLAYn` ordering does **not** correspond to IGCL output-array ordering. IGCL enumerates 10 outputs (9 inactive) in an arbitrary physical-port order; the one active output (`0x00800050`) maps to `\\.\DISPLAY1` by value, not by index. **Correlate by `WindowsDisplayEncoderID == QDC targetInfo.id`, never by array index.**
- `DISPLAYCONFIG_TARGET_DEVICE_NAME.edidManufactureId` is **byte-swapped** vs raw EDID bytes 8–9: QDC reports `0xAF06` for EDID bytes `0x06AF` (AUO). If comparing against EDID-derived identity, use raw EDID bytes (from `ctlPanelDescriptorAccess` or the registry), or byte-swap the QDC field.
- `ctlEdidManagement(READ_EDID)` is **exported but non-functional** on this driver for the internal eDP panel (returns `CTL_RESULT_ERROR_KMD_CALL`). Use `ctlPanelDescriptorAccess` for EDID.

## 6. pixtx units / scaling / LUT format (from `ctlPixelTransformationGetConfig` + header)

- **1D LUT:** `double` samples, normalized `[0,1]`, `NumSamplesPerChannel = 2049` (= 2¹¹+1), `NumChannels = 1` (shared degamma) or `3` (per-RGB gamma). Memory layout = `R[0..n)`, then `G[0..n)`, then `B[0..n)` (`pSampleValues`, `n*channels` doubles). Uniform sampling (`SamplingType=0`).
- **3D LUT:** `double` samples, `NumSamplesPerChannel = 17` → 17³ = 4913 samples, R outer loop.
- **Matrix:** `double Matrix[3][3]` + `PreOffsets[3]` + `PostOffsets[3]`, **normalized** — identity is `diag(1,1,1)`, coefficients are linear 0..~1.x, **not 0-100**. Confirmed by Intel's `Color_Sample_App.cpp`: `ApplyHueSaturation(..., hue 0..359, saturation 0.75..1.25)`. Phase E3 must convert ChromaDeck's UI 0–100 → normalized coefficients (e.g. `rgb_gains g → diag(g/100)`, `saturation s → luma-lerp matrix`).
- **Pipeline (internal panel):** `[3D LUT] → [1D LUT 1ch degamma] → [3×3 matrix+offsets] → [1D LUT 3ch gamma] → [3×3 matrix]`.
- **Read-back:** 1D LUT blocks read back successfully (values are the *active* curves — block[1] is an sRGB decode curve, block[3] a non-identity encode). Matrix blocks **cannot** be read back with `QUERY_TYPE_CURRENT` on this driver (`CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_TYPE`); treat the matrix current state as identity-default and rely on set-then-restore semantics.

## 7. HDR

- Internal panel `FeatureSupportedFlags = 0xC` = `PSR | ADAPTIVESYNC_VRR`. The HDR bit (`CTL_STD_DISPLAY_FEATURE_FLAG_HDR = 1<<5`) is **clear** — the panel is not HDR-capable.
- Header confirms `CTL_RESULT_ERROR_3DLUT_NOT_SUPPORTED_IN_HDR` (0x48000010). E2/E3 must probe HDR (`FeatureEnabledFlags & HDR`) and report the color backend `Unsupported` in HDR mode rather than attempt a 3D-LUT apply.

## 8. Blockers / impossibilities found (valuable results)

1. **`ctlDisplayFeatureReset` is NOT exported** by this driver's `ControlLib.dll` / `IntelControlLib.dll` (header `ctl_api.h` declares it; the shipped DLL omits it — verified via `dumpbin /exports` and `GetProcAddress` = error 127). The plan's E3 restore `ctlDisplayFeatureReset(CTL_DISPLAY_FEATURE_RESET_FLAG_COLOR)` **cannot** be used on this driver. Restore must be `ctlPixelTransformationSetConfig(CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT, NumBlocks=0)` **only** (this symbol *is* exported).
2. **IGCL does not expose the adapter LUID** (`pDeviceID` NULL, `device_id_size=0`). Adapter↔GPU identity must come from PCI vendor/device id and the QDC source LUID, not from IGCL.
3. **`ctlEdidManagement(READ_EDID)` fails** (`KMD_CALL`) on the internal panel; use `ctlPanelDescriptorAccess`.
4. **DLL load requires full DriverStore path.** `LoadLibraryW("ControlLib.dll")` by bare name fails (the runtime's sibling dependencies `IntelControlLib.dll` / `intel_gfx_api-x64.dll` / `ze_loader.dll` live only in the DriverStore). E2 must enumerate `C:\Windows\System32\DriverStore\FileRepository\iigd*\` and load `ControlLib.dll` (then `IntelControlLib.dll`) by full path. `intel_gfx_api-x64.dll` loads but does **not** export `ctlInit` — do not use it as the entrypoint.

## 9. Enumerated API surface for Phase E2/E3 (this driver, `ControlLib.dll`)

Mandatory (resolved + verified on hardware): `ctlInit`, `ctlClose`, `ctlEnumerateDevices`, `ctlEnumerateDisplayOutputs`, `ctlGetDeviceProperties`, `ctlGetDisplayProperties`, `ctlGetAdaperDisplayEncoderProperties` (note upstream "Adaper" typo, preserved), `ctlPixelTransformationGetConfig`, `ctlPixelTransformationSetConfig`.

Optional / conditional:
- `ctlEdidManagement` — exported, READ_EDID fails on eDP (keep for external DP/HDMI on Intel, fall back to `ctlPanelDescriptorAccess`).
- `ctlPanelDescriptorAccess` — exported, **works** for EDID (primary EDID path).
- `ctlSetBrightnessSetting` — exported but "companion display" only per plan; not used for desktop color.
- `ctlDisplayFeatureReset` — **absent** (see §8.1).

## 10. Verified `#[repr(C)]` transcriptions

Transcribed from the public Intel SDK header `include/igcl_api.h` (Copyright (C) 2025 Intel Corporation, Intel Software License Agreement 10.07.21 — header redistribution permitted with notice, "solely for use on Intel platforms"). Only the minimal subset used is reproduced; names preserved verbatim (including `ctl_pixtx_config_opertaion_type_t` / `CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT` and `ctlGetAdaperDisplayEncoderProperties` typos). No Intel binaries were reverse-engineered (only exported symbols + the public header). Runtime `size_of` values printed below each struct were confirmed on hardware and match the C layout.

```rust
type ctl_result_t = u32;
type ctl_api_handle_t = *mut c_void;
type ctl_device_adapter_handle_t = *mut c_void;
type ctl_display_output_handle_t = *mut c_void;

#[repr(C)] struct ctl_application_id_t { Data1: u32, Data2: u16, Data3: u16, Data4: [u8; 8] }

#[repr(C)] struct ctl_init_args_t {              // size 36
    Size: u32, Version: u8, AppVersion: u32, flags: u32,
    SupportedVersion: u32, ApplicationUID: ctl_application_id_t,
}
// Usage: Size=36, Version=0, AppVersion=(1<<16)|1, flags=0, UID=all-zeros.

#[repr(C)] struct ctl_firmware_version_t { major_version: u64, minor_version: u64, build_number: u64 }
#[repr(C)] struct ctl_adapter_bdf_t { bus: u8, device: u8, function: u8 }

#[repr(C)] struct ctl_device_adapter_properties_t {  // size 320
    Size: u32, Version: u8, pDeviceID: *mut c_void, device_id_size: u32,
    device_type: u32, supported_subfunction_flags: u32, driver_version: u64,
    firmware_version: ctl_firmware_version_t, pci_vendor_id: u32, pci_device_id: u32,
    rev_id: u32, num_eus_per_sub_slice: u32, num_sub_slices_per_slice: u32, num_slices: u32,
    name: [i8; 100], graphics_adapter_properties: u32, Frequency: u32,
    pci_subsys_id: u16, pci_subsys_vendor_id: u16, adapter_bdf: ctl_adapter_bdf_t,
    num_xe_cores: u32, reserved: [i8; 108],
}

#[repr(C)] struct ctl_generic_void_datatype_t { pData: *mut c_void, size: u32 }   // size 16
#[repr(C)] union ctl_os_display_encoder_identifier_t {   // size 16 (NOT 4 — the union is
    WindowsDisplayEncoderID: u32,                        // sized by the non-Windows arm)
    DisplayEncoderID: ctl_generic_void_datatype_t,
}

#[repr(C)] struct ctl_revision_datatype_t { major_version: u8, minor_version: u8, revision_version: u8 }

#[repr(C)] struct ctl_display_timing_t {   // size 64
    Size: u32, Version: u8, PixelClock: u64, HActive: u32, VActive: u32,
    HTotal: u32, VTotal: u32, HBlank: u32, VBlank: u32, HSync: u32, VSync: u32,
    RefreshRate: f32, SignalStandard: u32, VicId: u8,
}

#[repr(C)] struct ctl_display_properties_t {   // size 200
    Size: u32, Version: u8,
    Os_display_encoder_handle: ctl_os_display_encoder_identifier_t,
    Type: u32, AttachedDisplayMuxType: u32, ProtocolConverterOutput: u32,
    SupportedSpec: ctl_revision_datatype_t, SupportedOutputBPCFlags: u32,
    ProtocolConverterType: u32, DisplayConfigFlags: u32,
    FeatureEnabledFlags: u32, FeatureSupportedFlags: u32,
    AdvancedFeatureEnabledFlags: u32, AdvancedFeatureSupportedFlags: u32,
    Display_Timing_Info: ctl_display_timing_t, ReservedFields: [u32; 16],
}

#[repr(C)] struct ctl_adapter_display_encoder_properties_t {   // size 112
    Size: u32, Version: u8,
    Os_display_encoder_handle: ctl_os_display_encoder_identifier_t,
    Type: u32, IsOnBoardProtocolConverterOutputPresent: bool,
    SupportedSpec: ctl_revision_datatype_t, SupportedOutputBPCFlags: u32,
    EncoderConfigFlags: u32, FeatureSupportedFlags: u32, AdvancedFeatureSupportedFlags: u32,
    ReservedFields: [u32; 16],
}

#[repr(C)] struct ctl_edid_management_args_t {   // size 40
    Size: u32, Version: u8, OpType: u32, EdidType: u32, EdidSize: u32,
    pEdidBuf: *mut u8, OutFlags: u32,
}

#[repr(C)] struct ctl_panel_descriptor_access_args_t {   // size 32
    Size: u32, Version: u8, OpType: u32, BlockNumber: u32,
    DescriptorDataSize: u32, pDescriptorData: *mut u8,
}

#[repr(C)] struct ctl_display_feature_reset_t {   // size 16 (declared in header; symbol absent here)
    Size: u32, Version: u8, ResetFeature: u32, ResetFeatureStatus: u32,
}

// --- pixtx ---
#[repr(C)] struct ctl_pixtx_color_primaries_t { Size: u32, Version: u8, xR: f64, yR: f64, xG: f64, yG: f64, xB: f64, yB: f64, xW: f64, yW: f64 }

#[repr(C)] struct ctl_pixtx_pixel_format_t {   // size 120
    Size: u32, Version: u8, BitsPerColor: u32, IsFloat: bool, EncodingType: u32,
    ColorSpace: u32, ColorModel: u32, ColorPrimaries: ctl_pixtx_color_primaries_t,
    MaxBrightness: f64, MinBrightness: f64,
}

#[repr(C)] struct ctl_pixtx_1dlut_config_t {   // size 40
    Size: u32, Version: u8, SamplingType: u32, NumSamplesPerChannel: u32,
    NumChannels: u32, pSampleValues: *mut f64, pSamplePositions: *mut f64,
}

#[repr(C)] struct ctl_pixtx_matrix_config_t {   // size 128
    Size: u32, Version: u8, PreOffsets: [f64; 3], PostOffsets: [f64; 3], Matrix: [[f64; 3]; 3],
}

#[repr(C)] struct ctl_pixtx_3dlut_sample_t { Red: f64, Green: f64, Blue: f64 }
#[repr(C)] struct ctl_pixtx_3dlut_config_t { Size: u32, Version: u8, NumSamplesPerChannel: u32, pSampleValues: *mut ctl_pixtx_3dlut_sample_t }

#[repr(C)] union ctl_pixtx_config_t {   // size 128
    OneDLutConfig: ctl_pixtx_1dlut_config_t,
    ThreeDLutConfig: ctl_pixtx_3dlut_config_t,
    MatrixConfig: ctl_pixtx_matrix_config_t,
}

#[repr(C)] struct ctl_pixtx_block_config_t {   // size 144
    Size: u32, Version: u8, BlockId: u32, BlockType: u32, Config: ctl_pixtx_config_t,
}

#[repr(C)] struct ctl_pixtx_pipe_get_config_t {   // size 272
    Size: u32, Version: u8, QueryType: u32,
    InputPixelFormat: ctl_pixtx_pixel_format_t, OutputPixelFormat: ctl_pixtx_pixel_format_t,
    NumBlocks: u32, pBlockConfigs: *mut ctl_pixtx_block_config_t,
}

#[repr(C)] struct ctl_pixtx_pipe_set_config_t {   // size 32
    Size: u32, Version: u8, OpertaionType: u32, Flags: u32, NumBlocks: u32,
    pBlockConfigs: *mut ctl_pixtx_block_config_t,
}
```

Key constants (verbatim upstream names):

```rust
const CTL_RESULT_SUCCESS: u32 = 0x0000_0000;
const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ACTIVE: u32 = 1 << 0;
const CTL_DISPLAY_CONFIG_FLAG_DISPLAY_ATTACHED: u32 = 1 << 1;
const CTL_ENCODER_CONFIG_FLAG_INTERNAL_DISPLAY: u32 = 1 << 0;
const CTL_PIXTX_CONFIG_QUERY_TYPE_CAPABILITY: u32 = 0;
const CTL_PIXTX_CONFIG_QUERY_TYPE_CURRENT: u32 = 1;
const CTL_PIXTX_BLOCK_TYPE_1D_LUT: u32 = 1;
const CTL_PIXTX_BLOCK_TYPE_3D_LUT: u32 = 2;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX: u32 = 3;
const CTL_PIXTX_BLOCK_TYPE_3X3_MATRIX_AND_OFFSETS: u32 = 4;
const CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT: u32 = 1;  // note "OPERTAION" typo, upstream
const CTL_PIXTX_CONFIG_OPERTAION_TYPE_SET_CUSTOM: u32 = 2;
```

### pixtx query protocol (verified)

- **CAPABILITY:** call `ctlPixelTransformationGetConfig` with `QueryType=CAPABILITY`, `pBlockConfigs=NULL` → `NumBlocks` filled. Allocate `NumBlocks` block configs, set each `.Size`, call again → per-block `BlockId`/`BlockType`/`NumSamplesPerChannel`/`NumChannels` filled.
- **CURRENT:** read **one block at a time** (`NumBlocks=1`, `pBlockConfigs=&one block`, `QueryType=CURRENT`). Passing all blocks at once is rejected with `CTL_RESULT_ERROR_INVALID_PIXTX_BLOCK_TYPE`. For 1D-LUT blocks, allocate `NumSamplesPerChannel * NumChannels` doubles into `pSampleValues` first.

## 11. What this means for Phase E2/E3

- **E2 FFI:** transcribe the §10 structs verbatim; dynamic-load `ControlLib.dll` (then `IntelControlLib.dll`) by full DriverStore path (§8.4); resolve the §9 symbols; `ctlInit` with the §10 init-args pattern.
- **E3 `IntelBackend`:** resolve the target via §4 (prefer `ctlPanelDescriptorAccess` EDID → `edid_id`; fall back to encoder-ID chain). Apply gamma/brightness/contrast via the 3-channel 1D LUT (2049 samples, normalized `double`), rgb_gains/saturation/hue via the 3×3 matrix (normalized coefficients), composition order documented in code.
- **E3 restore:** `ctlPixelTransformationSetConfig(RESTORE_DEFAULT, NumBlocks=0)` **only** — do **not** call `ctlDisplayFeatureReset` (absent on this driver; guard with the optional-symbol check).
- **HDR:** probe `FeatureEnabledFlags & HDR`; report `Unsupported` in HDR (3D-LUT unsupported in HDR per `CTL_RESULT_ERROR_3DLUT_NOT_SUPPORTED_IN_HDR`).
- **License:** add a `NOTICE` file for the transcribed header structs (Intel Software License Agreement 10.07.21, header redistribution with notice).

## 12. Regression verification

`cargo test --manifest-path src-tauri\Cargo.toml` → **149 passed, 0 failed** (the concurrent backend-refactor agent has added tests since the plan's 128 baseline and this task's "136" figure; the spike itself adds nothing to that crate). The spike crate lives entirely under `spikes/intel-igcl/`, is git-ignored for `target/`, and is not a workspace member, so it cannot affect `src-tauri` build/tests.

## 13. Phase E5 — hardware verification of the real Intel backend (round-trip)

- **Date:** 2026-10-19
- **Status:** COMPLETE — real `IntelBackend::apply` + 1D-LUT read-back proven on hardware; **restore is broken (critical)**; panel left in its native state via explicit reconstruction.
- **Harness:** a temporary `#[ignore]`-gated unit test module in `backend/intel.rs` (`mod e5_hw_verify`), **removed after the run**. Run: `cargo test --ignored e5_hw_verify_round_trip -- --nocapture --test-threads=1`. A `RestoreGuard` (Drop) forced `RESTORE_DEFAULT` on any unwind so the panel could never be left mid-flight.

### 13.1 Verdict

Real (non-mock) `IntelBackend::apply` resolves the internal panel via EDID correlation and lands the pixtx SET; read-back proves the 1D LUT changed. **`ctlPixelTransformationSetConfig(CTL_PIXTX_CONFIG_OPERTAION_TYPE_RESTORE_DEFAULT, NumBlocks=0)` is effectively a no-op for the 3-channel gamma-encode LUT on this driver** — it returns `CTL_RESULT_SUCCESS` but leaves the LUT exactly where `SET_CUSTOM` put it. Because the plan's §8.1 marked `RESTORE_DEFAULT` as "the ONLY supported reset" (and this was never exercised — the E1 spike was read-only), the Intel `restore`/`reset_monitor` path **reports success while leaving the panel modified**. The panel was restored by reconstructing the native LUT and re-applying it (verified by read-back); a reboot fully returns it to the driver's factory default.

### 13.2 Measured evidence (verbatim harness output)

```
[E5] monitors:
  \\.\DISPLAY1  edid=EDID:06AF-F7AD-00000000  model=AUO F7AD     (internal, Intel)
  \\.\DISPLAY5  edid=EDID:05E3-2403-000027F5  model=AOC 2403     (HDMI, NVIDIA)

A. resolve(EDID:06AF-F7AD-00000000) -> OK
   probe: hdr=false  blocks = [327425 3D_LUT(17³), 327426 1D_LUT(1ch,2049),
                               327427 3X3_MATRIX_AND_OFFSETS, 327428 1D_LUT(3ch,2049), 327429 3X3_MATRIX]
   3ch LUT block id=327428 samples/ch=2049 channels=3

B. BEFORE         R[0..4] = 0.025000 0.025500 0.026001 0.026501   (native limited-range encode)
   apply (saturation 65, else neutral) -> gamma_applied=true
   AFTER          R[0..4] = 0.000000 0.000488 0.000977 0.001465   (identity — LUT changed)

C. restore (RESTORE_DEFAULT) -> gamma_applied=true
   AFTER-RESTORE  R[0..4] = 0.000000 0.000488 0.000977 0.001465   (STILL identity — NOT restored)

   1ch LUT (327426, sRGB degamma) R[0..4] = 0.000000 0.000038 0.000076 0.000113  (untouched throughout)

D. select(EDID:06AF-F7AD-00000000)            -> intel
   select(EDID:05E3-2403-000027F5, DISPLAY5)  -> nvapi

E. apply(bogus EDID:FFFF-FFFF-FFFFFFFF) -> gamma_applied=false,
   error="gdi-purge: no display device found …; intel: no active Intel output correlates to display …"
```

### 13.3 Per-step pass/fail

| Proof | Result | Evidence |
|---|---|---|
| A. real apply (EDID-correlated) | **PASS** | `resolve` OK; `set_pipeline` returned success |
| B. 1D LUT changed on apply | **PASS** | R[0] `0.025000` → `0.000000` (identity) |
| C. restore returns to default | **FAIL** | `RESTORE_DEFAULT` left the LUT at identity, not the pre-apply `0.025` |
| D. registry selection | **PASS** | internal panel → `intel`; HDMI → `nvapi` |
| E. no false success for unlanded step | **PASS** | bogus EDID → `gamma_applied=false` + error |

### 13.4 Critical finding — `RESTORE_DEFAULT` does not restore the 1D LUT

- The pre-apply ("native") 3ch gamma-encode LUT is an **exactly linear limited-range map**, `v[i] = min(0.025 + 1.025·i/2048, 1.0)` (black floor `0.025 = 1/40`, slope `1.025 = 41/40`); it reproduces every captured sample to 6 decimals (`0.025000 0.025500 0.026001 0.026501`). It is *not* a gamma curve — the panel's actual gamma lives elsewhere (the 1ch block is the sRGB degamma; the panel applies its own response).
- `ctlPixelTransformationSetConfig(RESTORE_DEFAULT, NumBlocks=0)` returns `CTL_RESULT_SUCCESS` but does **not** revert the 3ch LUT to that native curve — it leaves it at the last `SET_CUSTOM` value (identity). Repeated calls (three, including the unwind guard) do not help.
- Consequence: the app's Intel `restore`/`reset_monitor` (E3) reports `gamma_applied=true` while the panel stays modified. This is a real correctness/safety bug, not a harness artifact. Fixing it requires a working reset for this driver (see §13.6), not `ctlDisplayFeatureReset` (absent — §8.1).

### 13.5 Restoration (what was done to leave the panel normal)

The panel was restored by reconstructing the native 3ch LUT and re-applying it explicitly (identity matrix + `v[i]=min(0.025+1.025·i/2048,1.0)` via `set_pipeline`). Read-back afterwards: `0.025000 0.025500 0.026001 0.026501` — **byte-identical to the pre-apply capture**, and the 1ch degamma block was never touched. Residual caveats: (a) the reconstruction is exact in the low range (verified) and a faithful linear extension + clip near white (unverified high range — no pre-apply high samples were captured); (b) the pixtx SET state is runtime-only, so a **reboot** returns the panel to the driver's true factory default and removes any residual uncertainty. The NVIDIA HDMI display was never written to (proof D is selection-only).

### 13.6 Implications / next steps

- **E3 restore must be fixed.** `RESTORE_DEFAULT` alone is insufficient. Options: capture the pre-apply CURRENT 1D-LUT and re-`SET_CUSTOM` it on restore (the Intel `Color_Sample_App` pattern — it saves CURRENT before applying, then restores by re-setting the saved config, *not* via `RESTORE_DEFAULT`); or probe whether the driver's "default" is identity and accept identity-encode as the reset target (documenting the deviation from the native 0.025 floor).
- The HDMI EDID id on this machine is `EDID:05E3-2403-000027F5` (raw-EDID bytes 8–9 = `0x05E3`). §2/§3's `0xE305` is the byte-swapped `DisplayConfigGetDeviceInfo` field, consistent with the §5 byte-swap note.
- The nominal `probe` capability (`IntelBackend::nominal_probe`) matches the live probe here: 3ch 1D LUT present, matrix blocks present, `hdr=false`.

## 14. Phase E6 — restore correctness fix + hardware verification

- **Date:** 2026-10-19
- **Status:** COMPLETE — `restore`/`reset_monitor` no longer reports false success; the panel is returned to its captured pre-apply state and the result is verified by read-back.

### 14.1 The fix (implemented in `src-tauri/src/backend/intel.rs`)

The E5 bug was that `IntelBackend::restore` called `RESTORE_DEFAULT` (NumBlocks=0) and reported `gamma_applied=true` unconditionally, even though that operation leaves the 3ch gamma-encode 1D LUT at the last `SET_CUSTOM` value. The fix follows §13.6's `Color_Sample_App` pattern:

1. **Apply captures before it writes.** On every `apply`, after composing the pipeline, the backend finds the 1D-LUT block it is about to overwrite and reads its CURRENT samples via `ctlPixelTransformationGetConfig` (CURRENT query, one block at a time — the same read-back path the E1 spike proved works). The capture is keyed by `edid_id` and held **in memory only** (session-scoped; pixtx is runtime-only driver state, so nothing is persisted to disk).
2. **Capture/store failure → refuse to apply.** If the read-back or the in-memory store fails, `apply` does **not** issue the `SET_CUSTOM` write; it returns `gamma_applied=false` with `"capture pre-apply 1D LUT failed; refusing to apply unrestorable state: …"`. This implements the product principle "no false success, reliable restoration" — we never apply state we could not later restore.
3. **Restore = RESTORE_DEFAULT (matrix) + re-SET the captured LUT + read-back verify.** `restore` first calls `RESTORE_DEFAULT` (which resets the 3×3 matrix, the one block it genuinely restores), then re-`SET_CUSTOM`s the captured 1D LUT, then reads the LUT back and compares it against the capture. It reports `gamma_applied=true` **only** when the read-back matches within `LUT_VERIFY_EPSILON` (`1e-9`).
4. **No capture / mismatch → honest failure.** If there is no capture (app restarted with no prior apply this session) or the read-back mismatches, `restore` reports `gamma_applied=false` with an explicit reason. It still issues a best-effort `RESTORE_DEFAULT` (matrix) but never claims the 1D LUT was restored.

The `ColorBackend` trait signature is unchanged; `ExitPolicy::KeepApplied` / `shutdown_sweep` semantics are untouched (clean quit still performs no hardware write — only the explicit reset/restore paths changed).

### 14.2 Hardware round-trip (internal panel only, NVIDIA HDMI never touched)

Run (single-threaded, `#[ignore]`-gated `backend::intel::hw_verify::hw_verify_round_trip_restore_returns_to_captured`):

```
cargo test --manifest-path src-tauri\Cargo.toml --lib -- \
  --ignored hw_verify_round_trip_restore_returns_to_captured --nocapture --test-threads=1
```

Verbatim output (the panel's pre-apply state this session was **identity**, see §14.3):

```
[hw-verify] no-capture restore -> gamma_applied=false error=Some("intel reset: no pre-apply 1D LUT capture ...")
[hw-verify] BEFORE       R[0..4] = [0.0, 0.00048828125, 0.0009765625, 0.00146484375]
[hw-verify] BEFORE       R[last-2..] = [0.99951171875, 1.0]
[hw-verify] AFTER-APPLY  R[0..4] = [0.0, 5.960464477539063e-8, 2.384185791015625e-7, 5.960464477539063e-7]
[hw-verify] AFTER-RESTORE R[0..4] = [0.0, 0.00048828125, 0.0009765625, 0.00146484375]
[hw-verify] max_delta(after_restore, before) = 0.000e0
```

Per-step result:

| Step | Result | Evidence |
|---|---|---|
| no-capture restore | **PASS** | `gamma_applied=false`, reason names the missing capture |
| capture (BEFORE) | **PASS** | identity curve `i/2048` read back |
| apply (saturation 65 + gamma 2.2) | **PASS** | R[1] `0.00048828` → `5.96e-8` (the `(i/2048)^2.2` curve) |
| restore → read-back | **PASS** | R[0..4] returns exactly to the capture |
| read-back verify | **PASS** | `max_delta(after_restore, before) = 0.0` (byte-exact) |

The byte-exact round-trip (`max_delta = 0.0`) confirms `LUT_VERIFY_EPSILON = 1e-9` is appropriately tight: a genuine non-restore (RESTORE_DEFAULT leaving identity) shows a ~`2.5e-2` delta at the low end, six orders of magnitude above the epsilon.

### 14.3 Critical finding — the panel's current state is identity, not the E5 `0.025` curve

The E5 run observed a pre-apply 3ch LUT of `v[i] = min(0.025 + 1.025·i/2048, 1.0)` (black floor `0.025 = 1/40`). **This run's pre-apply 3ch LUT is the identity curve `v[i] = i/2048` (black floor `0.0`), read back byte-exactly.** The two runs disagree on the panel's "native" state, which means:

- The `0.025` limited-range encode E5 captured was **runtime state**, not the driver's reboot default. Something (the Intel driver's limited-range handling for this eDP panel, a prior colour application, or a mode change) set it before E5; it was not the post-reboot factory state.
- After the machine reboots (or the driver re-initializes the panel), the 3ch LUT returns to **identity**, not to the `0.025` curve. The `0.025` floor is therefore **not** a value ChromaDeck should reconstruct as "default" — reconstructing it would actually be *writing a non-default curve* on a freshly-booted machine.

This is precisely the "previous agent reconstructed the high end as a linear extension" caveat from §13.5, generalized: the reconstruction was exact *as a linear extension of the captured low samples*, but the underlying assumption that the `0.025` curve is the factory default is unverified and now contradicted. The capture-based fix sidesteps the entire question — it restores whatever the panel actually had before the last apply, with no assumption about what "default" is.

### 14.4 Unit tests added (`src-tauri/src/backend/intel.rs`)

- `apply_capture_failure_skips_without_applying` — capture failure ⇒ no SET, honest error.
- `apply_captures_then_restore_resets_captured_lut` — capture→apply→restore re-SETs the captured LUT.
- `restore_without_capture_reports_honest_failure` — no capture ⇒ `gamma_applied=false`.
- `restore_read_back_mismatch_reports_failure` — accepted-but-ineffective write ⇒ failure.
- `restore_reports_error_when_reset_write_fails` — `RESTORE_DEFAULT` failure reported.
- `hw_verify_round_trip_restore_returns_to_captured` (`#[ignore]`) — hardware round-trip above.

The mock FFI (`MockIgcl`) now simulates the live panel faithfully: `set_pipeline` updates its read-back LUT, `restore_default` does **not** (reproducing the E5 bug), and a `set_lut_noop` flag models an accepted-but-ineffective write. Each test uses a distinct `edid_id` because the capture table is a process-global `Mutex<HashMap<String, CapturedLut>>` shared across backend instances.

### 14.5 Regression counts

- `cargo test --manifest-path src-tauri\Cargo.toml` → **225 passed, 0 failed, 1 ignored** (current gate; this section originally recorded 208 when the phase landed).
- `npm test` → **134 passed** (current gate; originally 127).
- `cargo build --manifest-path src-tauri\Cargo.toml` → warning-clean.
