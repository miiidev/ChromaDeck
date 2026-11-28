# AMD Native Display Backend (`ADLX`) Design

## 1. Goals & Constraints
- Provide first-class AMD Radeon dGPU and Ryzen iGPU display color control via AMD's official MIT-licensed **ADLX** (`amdadlx64.dll`) runtime library.
- Zero build-time SDK binary dependency: `amdadlx64.dll` is loaded dynamically at runtime via `LoadLibraryW` and resolved via `GetProcAddress`. Non-AMD machines skip in `< 1 ms` and cache the negative availability result.
- Preserve ChromaDeck's strict backend selection order per physical display:
  **`nvapi` → `amd` → `intel` → `gdi`** (supports hybrid laptops such as Ryzen iGPU internal panel + NVIDIA RTX dGPU external monitor).
- All `unsafe` FFI stays strictly encapsulated in `src-tauri/src/backend/amd.rs`. All decision logic (`probe`, `apply`, `restore`, range mapping, EDID correlation, drift detection) sits behind a mockable `AmdApi` trait tested without hardware.

## 2. Transcribed C ABI Surface
Transcribed verbatim from `GPUOpen-LibrariesAndSDKs/ADLX` `SDK/Include/`:
- Entry exports: `ADLXQueryFullVersion`, `ADLXInitialize`, `ADLXInitializeWithIncompatibleDriver` (fallback), `ADLXTerminate`.
- Root singleton: `IADLXSystem` (note: does **not** inherit `IADLXInterface`; slot 0 is `GetHybridGraphicsType`, slot 3 is `GetDisplaysServices`).
- Display enumeration: `IADLXDisplayServices::GetDisplays` → `IADLXDisplayList` (`Size`, `At_DisplayList`) → `IADLXDisplay` (`EDID`, `Name`, `UniqueId`, `ConnectorType`).
- Color controls: `IADLXDisplayCustomColor` (`Is*Supported`, `Get*Range`, `Get*`, `Set*` for `Hue`, `Saturation`, `Brightness`, `Contrast`).
- Gamma/RGB LUT: `IADLXDisplayGamma` (`IsCurrentReGammaPQ`, `IsCurrentReGammaPQ2084Interim`, `IsCurrentReGammaRamp`, `GetGammaRamp`, `SetReGammaRamp_Memory`, `ResetGammaRamp`).

## 3. Display Correlation (`edid_id`)
- `IADLXDisplay::EDID` returns a NUL-terminated ASCII hex string of the panel's EDID.
- ChromaDeck strips whitespace, decodes the base 128-byte block, and feeds it into `crate::monitor::edid_id_from_bytes(&[u8; 128])` — producing byte-identical keys to `monitor::list_monitors()`.
- Matched via `crate::monitor::key_match`. Ambiguous multi-display matches (two identical EDIDs) refuse to guess and return an error / fall back cleanly.

## 4. Slider Mapping & Stage Purity
- **Stage purity:** Before writing ADLX controls, `apply` purges the Windows GDI gamma ramp to linear identity (`gdi_identity_purge`) so OS ramps cannot double-apply on top of ADLX.
- **Custom Color (`Brightness`, `Contrast`, `Saturation`, `Hue`):**
  - Each control queries its live `ADLX_IntRange { minValue, maxValue, step }` at runtime rather than hardcoding ranges.
  - Neutral midpoint is `(minValue + maxValue) / 2`.
  - Brightness/Contrast/Saturation (`0..=100`, neutral `50`) map linearly onto `[minValue, maxValue]` snapped to `step`.
  - Hue (`0..=359°`, neutral `0°`) wraps to signed `[-180°, +180°]` and maps linearly onto `[minValue, maxValue]` snapped to `step`.
  - Vibrance is honestly reported `Unsupported("AMD ADLX uses Saturation instead of NVIDIA Digital Vibrance")`.
- **Gamma & RGB Gains (`IADLXDisplayGamma`):**
  - `ADLX_GammaRamp.gamma[256 * 3]` uses planar layout (`gamma[i + channel * 256]`, verified against `Samples/C/Display/DisplayGamma/mainDisplayGamma.c`).
  - Because ADLX's re-gamma LUT sits at the linear→sRGB OETF stage, ChromaDeck computes the sRGB OETF curve first (`12.92*x` below `0.0031308`, else `1.055*x^(1/2.4) - 0.055`), applies ChromaDeck's relative gamma exponent `powf(1.0 / preset.gamma)` and per-channel `rgb_gains[ch]`, and scales to `0..=65535`.
  - When `preset.gamma == 1.0` and `preset.rgb_gains == [1.0, 1.0, 1.0]`, `apply` calls `ResetGammaRamp()` so the panel's factory transfer curve stays untouched.
- **HDR detection:** If `IsCurrentReGammaPQ` or `IsCurrentReGammaPQ2084Interim` is true, `probe` reports all adjustments `Unsupported("HDR is active on this display — switch to SDR to apply presets")` and `apply` refuses cleanly.
- **Restore & Read-back Verification:** `restore` resets Custom Color controls to each control's neutral midpoint (verified by `Get*`), calls `ResetGammaRamp()`, and verifies GDI identity.

## 5. Hardware Verification Status
- Dev machine has Intel UHD + NVIDIA RTX (no AMD GPU installed).
- Standalone probe binary `spikes/amd-adlx` (`amd-adlx-spike.exe`) + portable ChromaDeck bundle (`npm run bundle:portable`) are provided for live verification on AMD Radeon / Ryzen integrated-graphics laptops.
- All correlation, range mapping, sRGB re-gamma curve, HDR gating, apply/restore read-back verification, and drift detection paths are unit-tested via `MockAmd` in `src-tauri/src/backend/amd.rs`.
