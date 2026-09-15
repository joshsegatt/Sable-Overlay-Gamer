// sable-gpu: GPU telemetry
//
// Adapter pick: DXGI Factory6 HIGH_PERFORMANCE (not adapter 0 — that is
// the iGPU on most laptops).
// VRAM total: DedicatedVideoMemory. Used: QueryVideoMemoryInfo CurrentUsage.
// Load: NVAPI GetUsages on NVIDIA, else PDH GPU Engine counter.

use anyhow::{bail, Context, Result};
use sable_core::{GpuInfo, GpuMetrics, GpuVendor};
use tracing::debug;

/// Static identity of the gaming GPU.
pub fn get_gpu_info() -> Result<GpuInfo> {
    dxgi::get_primary_adapter_info()
}

/// Live counters. Never panics. Fields stay None when the source is missing.
pub fn get_gpu_metrics() -> GpuMetrics {
    let info = dxgi::get_primary_adapter_info().ok();
    let vendor = info.as_ref().map(|i| i.vendor.clone());

    let mut m = match vendor {
        Some(GpuVendor::Nvidia) => nvapi::get_metrics().unwrap_or_default(),
        _ => GpuMetrics::default(),
    };

    if let Ok((used, total)) = dxgi::get_vram_usage() {
        if m.vram_used_mb.is_none() {
            m.vram_used_mb = Some(used);
        }
        if m.vram_total_mb.is_none() {
            m.vram_total_mb = Some(total);
        }
    }

    if m.gpu_usage_pct.is_none() {
        if let Some(pct) = pdh::gpu_engine_usage_pct() {
            m.gpu_usage_pct = Some(pct);
        }
    }

    m
}

mod dxgi {
    use super::*;
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::*;

    const VENDOR_NVIDIA: u32 = 0x10DE;
    const VENDOR_AMD: u32 = 0x1002;
    const VENDOR_AMD_ATI: u32 = 0x1022;
    const VENDOR_INTEL: u32 = 0x8086;

    fn vendor_from_id(id: u32) -> GpuVendor {
        match id {
            VENDOR_NVIDIA => GpuVendor::Nvidia,
            VENDOR_AMD | VENDOR_AMD_ATI => GpuVendor::Amd,
            VENDOR_INTEL => GpuVendor::Intel,
            _ => GpuVendor::Unknown,
        }
    }

    fn is_software(desc: &DXGI_ADAPTER_DESC1) -> bool {
        (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0
            || desc.VendorId == 0x1414
    }

    fn utf16_name(raw: &[u16]) -> String {
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        String::from_utf16_lossy(&raw[..end])
            .trim()
            .trim_end_matches('\0')
            .to_string()
    }

    fn open_adapter() -> Result<IDXGIAdapter1> {
        unsafe {
            let factory: IDXGIFactory1 =
                CreateDXGIFactory1().context("CreateDXGIFactory1 failed")?;

            if let Ok(factory6) = factory.cast::<IDXGIFactory6>() {
                for i in 0..8u32 {
                    let adapter = factory6.EnumAdapterByGpuPreference(
                        i,
                        DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                    );
                    let adapter: IDXGIAdapter1 = match adapter {
                        Ok(a) => a.cast().context("adapter cast")?,
                        Err(_) => break,
                    };
                    let desc = adapter.GetDesc1().context("GetDesc1")?;
                    if is_software(&desc) {
                        continue;
                    }
                    return Ok(adapter);
                }
            }

            for i in 0..8u32 {
                let adapter = match factory.EnumAdapters1(i) {
                    Ok(a) => a,
                    Err(_) => break,
                };
                let desc = adapter.GetDesc1().context("GetDesc1")?;
                if is_software(&desc) {
                    continue;
                }
                return Ok(adapter);
            }

            bail!("no hardware DXGI adapter")
        }
    }

    pub fn get_primary_adapter_info() -> Result<GpuInfo> {
        unsafe {
            let adapter = open_adapter()?;
            let desc = adapter.GetDesc1().context("GetDesc1 failed")?;
            Ok(GpuInfo {
                vendor: vendor_from_id(desc.VendorId),
                name: utf16_name(&desc.Description),
                driver_version: get_driver_version_from_registry(),
                vram_total_mb: (desc.DedicatedVideoMemory / (1024 * 1024)) as u64,
            })
        }
    }

    pub fn get_vram_usage() -> Result<(u64, u64)> {
        unsafe {
            let adapter = open_adapter()?;
            let desc = adapter.GetDesc1().context("GetDesc1")?;
            let total = (desc.DedicatedVideoMemory / (1024 * 1024)) as u64;

            let used = match adapter.cast::<IDXGIAdapter3>() {
                Ok(adapter3) => {
                    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                    adapter3
                        .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                        .context("QueryVideoMemoryInfo")?;
                    info.CurrentUsage / (1024 * 1024)
                }
                Err(_) => 0,
            };

            Ok((used, total))
        }
    }

    fn get_driver_version_from_registry() -> String {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::ERROR_SUCCESS;
        use windows::Win32::System::Registry::*;

        let class_keys = [
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000",
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0001",
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0002",
            r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0003",
        ];

        for key_path in class_keys {
            let wide_key: Vec<u16> = key_path.encode_utf16().chain(Some(0)).collect();
            let val_name: Vec<u16> = "DriverVersion\0".encode_utf16().collect();
            let mut hkey = HKEY::default();

            let opened = unsafe {
                RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR(wide_key.as_ptr()),
                    Some(0),
                    KEY_READ,
                    &mut hkey,
                ) == ERROR_SUCCESS
            };
            if !opened {
                continue;
            }

            let mut buf = vec![0u16; 128];
            let mut size = (buf.len() * 2) as u32;
            let q = unsafe {
                RegQueryValueExW(
                    hkey,
                    PCWSTR(val_name.as_ptr()),
                    None,
                    None,
                    Some(buf.as_mut_ptr() as *mut u8),
                    Some(&mut size),
                )
            };
            let _ = unsafe { RegCloseKey(hkey) };

            if q == ERROR_SUCCESS {
                let chars = (size as usize / 2).saturating_sub(1);
                let version = OsString::from_wide(&buf[..chars.min(buf.len())])
                    .to_string_lossy()
                    .trim()
                    .to_string();
                if !version.is_empty() {
                    return version;
                }
            }
        }

        "Unknown".to_string()
    }
}

mod nvapi {
    use super::*;
    use std::sync::OnceLock;
    use windows::core::PCSTR;
    use windows::core::PCWSTR;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    const NVAPI_OK: i32 = 0;
    const ID_INITIALIZE: u32 = 0x0150E828;
    const ID_ENUM_PHYSICAL: u32 = 0xE5AC921F;
    const ID_GET_USAGES: u32 = 0x189A1FDF;
    const MAX_GPUS: usize = 64;
    const USAGES_SLOTS: usize = 34;

    type QueryIface = unsafe extern "C" fn(u32) -> *mut std::ffi::c_void;
    type NvInit = unsafe extern "C" fn() -> i32;
    type NvEnum = unsafe extern "C" fn(*mut usize, *mut u32) -> i32;
    type NvUsages = unsafe extern "C" fn(usize, *mut NvUsagesBlock) -> i32;

    #[repr(C)]
    struct NvUsagesBlock {
        version: u32,
        usage: [u32; USAGES_SLOTS],
    }

    struct NvApi {
        enum_gpus: NvEnum,
        get_usages: NvUsages,
    }

    fn load() -> Option<&'static NvApi> {
        static API: OnceLock<Option<NvApi>> = OnceLock::new();
        API.get_or_init(|| match load_inner() {
            Ok(api) => Some(api),
            Err(e) => {
                debug!("NVAPI unavailable: {e}");
                None
            }
        })
        .as_ref()
    }

    fn load_inner() -> Result<NvApi> {
        let lib_name: Vec<u16> = "nvapi64.dll\0".encode_utf16().collect();
        let hlib = unsafe { LoadLibraryW(PCWSTR(lib_name.as_ptr())).context("nvapi64.dll missing")? };
        let query = unsafe { GetProcAddress(hlib, PCSTR(b"nvapi_QueryInterface\0".as_ptr())) }
            .context("nvapi_QueryInterface missing")?;
        let query_fn: QueryIface = unsafe { std::mem::transmute(query) };

        let init_ptr = unsafe { query_fn(ID_INITIALIZE) };
        if init_ptr.is_null() {
            bail!("NvAPI_Initialize missing");
        }
        let init_fn: NvInit = unsafe { std::mem::transmute(init_ptr) };
        let status = unsafe { init_fn() };
        if status != NVAPI_OK {
            bail!("NvAPI_Initialize status {status}");
        }

        let enum_ptr = unsafe { query_fn(ID_ENUM_PHYSICAL) };
        let usages_ptr = unsafe { query_fn(ID_GET_USAGES) };
        if enum_ptr.is_null() || usages_ptr.is_null() {
            bail!("EnumPhysicalGPUs / GetUsages missing");
        }

        Ok(NvApi {
            enum_gpus: unsafe { std::mem::transmute(enum_ptr) },
            get_usages: unsafe { std::mem::transmute(usages_ptr) },
        })
    }

    pub fn get_metrics() -> Result<GpuMetrics> {
        let api = load().context("NVAPI not loaded")?;
        let mut handles = [0usize; MAX_GPUS];
        let mut count = 0u32;
        let st = unsafe { (api.enum_gpus)(handles.as_mut_ptr(), &mut count) };
        if st != NVAPI_OK || count == 0 {
            bail!("EnumPhysicalGPUs status {st} count {count}");
        }

        let mut block = NvUsagesBlock {
            version: (std::mem::size_of::<NvUsagesBlock>() as u32) | (1u32 << 16),
            usage: [0; USAGES_SLOTS],
        };
        let st = unsafe { (api.get_usages)(handles[0], &mut block) };
        if st != NVAPI_OK {
            bail!("GetUsages status {st}");
        }

        let pct = block.usage[3].min(100) as f32;
        Ok(GpuMetrics {
            gpu_usage_pct: Some(pct),
            ..GpuMetrics::default()
        })
    }
}

/// PDH `GPU Engine(*)\Utilization Percentage`.
/// First collect primes the query (PDH rule). Later polls average 3D engines.
mod pdh {
    use std::sync::Mutex;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::System::Performance::*;

    const PDH_MORE_DATA: u32 = 0x8000_07D2;

    struct Query {
        handle: PDH_HQUERY,
        counter: PDH_HCOUNTER,
        primed: bool,
    }

    unsafe impl Send for Query {}

    fn open() -> Option<Query> {
        unsafe {
            let mut handle = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) != 0 {
                return None;
            }
            let path: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage\0"
                .encode_utf16()
                .collect();
            let mut counter = PDH_HCOUNTER::default();
            if PdhAddEnglishCounterW(handle, PCWSTR(path.as_ptr()), 0, &mut counter) != 0 {
                let _ = PdhCloseQuery(handle);
                return None;
            }
            Some(Query {
                handle,
                counter,
                primed: false,
            })
        }
    }

    fn name_has_3d(sz: PWSTR) -> bool {
        if sz.is_null() {
            return false;
        }
        let s = unsafe { sz.to_string().unwrap_or_default() }.to_ascii_lowercase();
        s.contains("engtype_3d") || s.contains("3d")
    }

    pub fn gpu_engine_usage_pct() -> Option<f32> {
        static Q: Mutex<Option<Query>> = Mutex::new(None);
        let mut guard = Q.lock().ok()?;
        if guard.is_none() {
            *guard = open();
        }
        let q = guard.as_mut()?;

        unsafe {
            if PdhCollectQueryData(q.handle) != 0 {
                return None;
            }
            if !q.primed {
                q.primed = true;
                return None;
            }

            let mut buf_size = 0u32;
            let mut item_count = 0u32;
            let first = PdhGetFormattedCounterArrayW(
                q.counter,
                PDH_FMT_DOUBLE.0,
                &mut buf_size,
                &mut item_count,
                None,
            );
            if first != 0 && first != PDH_MORE_DATA {
                return None;
            }
            if buf_size == 0 || item_count == 0 {
                return None;
            }

            let mut raw = vec![0u8; buf_size as usize];
            let items = raw.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(
                q.counter,
                PDH_FMT_DOUBLE.0,
                &mut buf_size,
                &mut item_count,
                Some(items),
            ) != 0
            {
                return None;
            }

            let slice = std::slice::from_raw_parts(items, item_count as usize);
            let mut sum = 0.0f64;
            let mut n = 0u32;
            let mut sum_all = 0.0f64;
            let mut n_all = 0u32;
            for item in slice {
                let v = item.FmtValue.Anonymous.doubleValue;
                if !v.is_finite() || v < 0.0 {
                    continue;
                }
                sum_all += v;
                n_all += 1;
                if name_has_3d(item.szName) {
                    sum += v;
                    n += 1;
                }
            }

            let (s, c) = if n > 0 { (sum, n) } else { (sum_all, n_all) };
            if c == 0 {
                return None;
            }
            Some((s / f64::from(c)).clamp(0.0, 100.0) as f32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_info_does_not_panic() {
        let _ = get_gpu_info();
    }

    #[test]
    fn gpu_metrics_does_not_panic() {
        let _ = get_gpu_metrics();
    }
}
