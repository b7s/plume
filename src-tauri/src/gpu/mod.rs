use serde::Serialize;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::PCSTR;

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub id: String,
    pub label: String,
    pub version: Option<String>,
}

/// Detect available GPU compute backends on this system.
/// Always returns at least the CPU fallback.
pub fn detect() -> Vec<GpuInfo> {
    let mut backends = Vec::new();
    backends.push(GpuInfo {
        id: "cpu".into(),
        label: "CPU (default)".into(),
        version: None,
    });

    if has_vulkan() {
        backends.push(GpuInfo {
            id: "vulkan".into(),
            label: "Vulkan (any GPU)".into(),
            version: None,
        });
    }

    if let Some(ver) = cuda_version() {
        backends.push(GpuInfo {
            id: "cuda".into(),
            label: format!("CUDA {ver} (NVIDIA GPU)"),
            version: Some(ver),
        });
    }

    if has_amd_hip() {
        backends.push(GpuInfo {
            id: "hip".into(),
            label: "AMD HIP (Radeon GPU)".into(),
            version: None,
        });
    }

    backends
}

fn has_vulkan() -> bool {
    unsafe {
        let name = windows::core::w!("vulkan-1.dll");
        LoadLibraryW(name).is_ok()
    }
}

fn cuda_version() -> Option<String> {
    unsafe {
        let name = windows::core::w!("nvcuda.dll");
        let lib = match LoadLibraryW(name) {
            Ok(h) => h,
            Err(_) => return None,
        };

        type CuDriverGetVersion = unsafe extern "C" fn(*mut i32) -> i32;
        const CUDA_SUCCESS: i32 = 0;

        let fn_name = b"cuDriverGetVersion\0";
        if let Some(farproc) = GetProcAddress(lib, PCSTR(fn_name.as_ptr())) {
            let func: CuDriverGetVersion = std::mem::transmute(farproc);
            let mut version = 0i32;
            let result = func(&mut version);
            if result == CUDA_SUCCESS && version > 0 {
                let major = version / 1000;
                let minor = (version % 1000) / 10;
                return Some(format!("{major}.{minor}"));
            }
        }
    }
    None
}

fn has_amd_hip() -> bool {
    unsafe {
        let name = windows::core::w!("amdhip64.dll");
        LoadLibraryW(name).is_ok()
    }
}

/// Parse a CUDA version string like "12.4" or "13.3" into (major, minor).
pub fn parse_cuda_version(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        Some((major, minor))
    } else {
        None
    }
}

/// Given the GitHub releases JSON data and a detected CUDA driver version,
/// find the best matching CUDA asset URL (highest ≤ driver version).
pub fn find_best_cuda_asset(data: &serde_json::Value, driver_ver: Option<&str>) -> Result<String, String> {
    let assets = data["assets"]
        .as_array()
        .ok_or("no assets array")?;

    let driver_parsed = driver_ver.and_then(parse_cuda_version);

    let mut cuda_assets: Vec<(u32, u32, String)> = Vec::new();
    for asset in assets {
        let name = asset["name"].as_str().unwrap_or("");
        let url = asset["browser_download_url"].as_str().unwrap_or("").to_string();
        if !name.contains("win-cuda-") || !name.ends_with("-x64.zip") || name.contains("cudart") {
            continue;
        }
        let ver_str = name
            .strip_suffix("-x64.zip")
            .and_then(|s| s.split("win-cuda-").nth(1))
            .and_then(|s| s.split('-').next());
        if let Some(ver_str) = ver_str {
            if let Some((maj, min)) = parse_cuda_version(ver_str) {
                cuda_assets.push((maj, min, url));
            }
        }
    }

    if cuda_assets.is_empty() {
        return Err("no CUDA assets found in release".into());
    }

    cuda_assets.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    if let Some((dmaj, dmin)) = driver_parsed {
        if let Some(best) = cuda_assets
            .iter()
            .find(|(maj, min, _)| *maj < dmaj || (*maj == dmaj && *min <= dmin))
        {
            return Ok(best.2.clone());
        }
    }

    Ok(cuda_assets.last().map(|a| a.2.clone()).unwrap_or_default())
}
