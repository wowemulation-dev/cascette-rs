//! GET /hardware -- System hardware information.
//!
//! The real agent reports CPU, memory, GPU, and OS info via Win32 APIs.
//! We use the sysinfo crate for cross-platform CPU/memory/OS detection
//! and platform-specific GPU probes:
//!
//! - Linux: `/sys/class/drm` sysfs (fast, no subprocess) with `lspci -mm`
//!   fallback for human-readable vendor/device names.
//! - macOS: `system_profiler SPDisplaysDataType -json`
//! - Windows: `wmic path win32_VideoController get Name /format:list`
//!
//! All GPU probes are best-effort; an empty string is returned when
//! detection fails or the platform has no known probe path.

use axum::Json;

/// GET /hardware
///
/// Returns hardware information in Agent.exe wire format.
///
/// Serializes a `Hardware` protobuf message to JSON with flat fields:
/// `cpu_arch`, `cpu_num_cores`, `cpu_speed`, `memory`, `num_gpus`,
/// `gpu_1`/`gpu_2`/`gpu_3` (each a `Gpu` sub-message), `cpu_vendor`, `cpu_brand`.
pub async fn get_hardware() -> Json<serde_json::Value> {
    let sys = sysinfo::System::new_all();

    let cpu_num_cores = sys.cpus().len() as u32;
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_default();
    // cpu_vendor: not directly exposed by sysinfo; use empty string as Agent.exe
    // only populates this via CPUID on Windows.
    let cpu_vendor = String::new();
    // cpu_speed: frequency in MHz from sysinfo, or 0 if unavailable.
    let cpu_speed: u64 = sys.cpus().first().map_or(0, sysinfo::Cpu::frequency);
    // cpu_arch: Agent.exe uses an enum (0=x86, 1=x64, etc.).
    // We emit 1 for x86_64 (the only arch cascette targets) to match the
    // typical value a Battle.net launcher would see on a 64-bit install.
    let cpu_arch: u32 = u32::from(std::env::consts::ARCH == "x86_64");

    let memory: u64 = sys.total_memory();

    // GPU detection: best-effort, may return empty string.
    let gpu_name = detect_gpu_name();
    let num_gpus: u32 = u32::from(!gpu_name.is_empty());

    // Agent.exe supports up to 3 GPUs (gpu_1, gpu_2, gpu_3).
    // We populate gpu_1 if detection succeeded; gpu_2 and gpu_3 are empty objects.
    let gpu_1 = serde_json::json!({
        "vendor_id": 0u32,
        "device_id": 0u32,
        "shared_memory": 0i64,
        "video_memory": 0i64,
        "system_memory": 0i64,
        "integrated": false,
        "name": gpu_name,
    });

    Json(serde_json::json!({
        "cpu_arch": cpu_arch,
        "cpu_num_cores": cpu_num_cores,
        "cpu_speed": cpu_speed,
        "memory": memory,
        "num_gpus": num_gpus,
        "gpu_1": gpu_1,
        "gpu_2": {},
        "gpu_3": {},
        "cpu_vendor": cpu_vendor,
        "cpu_brand": cpu_brand,
    }))
}

/// Best-effort GPU name detection, cross-platform.
fn detect_gpu_name() -> String {
    #[cfg(target_os = "linux")]
    return linux_gpu_name().unwrap_or_default();

    #[cfg(target_os = "macos")]
    return macos_gpu_name().unwrap_or_default();

    #[cfg(target_os = "windows")]
    return windows_gpu_name().unwrap_or_default();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return String::new();
}

// ─── Linux ───────────────────────────────────────────────────────────────────

/// GPU name on Linux.
///
/// First attempts sysfs (no subprocess, instant). Falls back to `lspci -mm`
/// for a human-readable vendor + device string when sysfs only yields hex IDs.
#[cfg(target_os = "linux")]
fn linux_gpu_name() -> Option<String> {
    // Attempt 1: sysfs — may return a human-readable label or a hex PCI ID.
    if let Some(name) = linux_sysfs_gpu_name() {
        // If it looks like a real name (not a raw hex ID like "10de:2204"),
        // return it immediately.
        if !name.contains(':') || name.len() > 9 {
            return Some(name);
        }
        // It is a bare hex ID — try lspci for a better name, keep hex as fallback.
        return Some(linux_lspci_gpu_name().unwrap_or(name));
    }

    // Attempt 2: lspci -mm (available on most desktop Linux installations).
    linux_lspci_gpu_name()
}

/// Read GPU info from the DRM subsystem on Linux.
///
/// Iterates `/sys/class/drm/card*` looking for `label`, then `PCI_ID` in
/// `uevent`, then raw `vendor`+`device` sysfs files.
#[cfg(target_os = "linux")]
fn linux_sysfs_gpu_name() -> Option<String> {
    use std::fs;
    use std::path::Path;

    let drm = Path::new("/sys/class/drm");
    if !drm.exists() {
        return None;
    }

    let mut entries: Vec<_> = fs::read_dir(drm)
        .ok()?
        .filter_map(std::result::Result::ok)
        .collect();
    // Sort for deterministic ordering (card0 before card1, etc.).
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only top-level card devices (card0, card1, …); skip render nodes and
        // connector entries like card0-HDMI-A-1.
        if !name_str.starts_with("card") || name_str.contains('-') {
            continue;
        }

        let device_dir = entry.path().join("device");

        // Human-readable label (available on some drivers, e.g. virtio-gpu).
        let label_path = device_dir.join("label");
        if let Ok(label) = fs::read_to_string(&label_path) {
            let label = label.trim().to_string();
            if !label.is_empty() {
                return Some(label);
            }
        }

        // PCI_ID from uevent (e.g. "PCI_ID=10DE:2204").
        let uevent_path = device_dir.join("uevent");
        if let Ok(uevent) = fs::read_to_string(&uevent_path) {
            for line in uevent.lines() {
                if let Some(pci_id) = line.strip_prefix("PCI_ID=") {
                    return Some(pci_id.to_string());
                }
            }
        }

        // Last resort: raw vendor + device hex files.
        let vendor = fs::read_to_string(device_dir.join("vendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let device = fs::read_to_string(device_dir.join("device"))
            .ok()
            .map(|s| s.trim().to_string());

        if let (Some(v), Some(d)) = (vendor, device) {
            return Some(format!("{v}:{d}"));
        }
    }

    None
}

/// Parse human-readable GPU name from `lspci -mm` output on Linux.
///
/// `lspci -mm` emits one device per line in machine-readable format:
/// `<slot> "<class>" "<vendor>" "<device>" ...`
/// We filter for VGA/display/3D controllers and combine vendor + device.
#[cfg(target_os = "linux")]
fn linux_lspci_gpu_name() -> Option<String> {
    use std::process::Command;

    let output = Command::new("lspci").arg("-mm").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("vga") || lower.contains("display") || lower.contains("3d controller") {
            // Fields are quoted: slot "class" "vendor" "device" ...
            let fields: Vec<&str> = line.splitn(6, '"').collect();
            // indices: 0=slot, 1=class, 2=sep, 3=vendor, 4=sep, 5=device...
            if fields.len() >= 6 {
                let vendor = fields[3].trim();
                // device name is between the 5th and 6th quote.
                let device = fields[5]
                    .trim_matches('"')
                    .split('"')
                    .next()
                    .unwrap_or("")
                    .trim();
                if !vendor.is_empty() && !device.is_empty() {
                    return Some(format!("{vendor} {device}"));
                }
            }
        }
    }

    None
}

// ─── macOS ───────────────────────────────────────────────────────────────────

/// GPU name on macOS via `system_profiler SPDisplaysDataType -json`.
#[cfg(target_os = "macos")]
fn macos_gpu_name() -> Option<String> {
    use std::process::Command;

    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Path: SPDisplaysDataType[0].spdisplays_ndrvs[0]._name
    // or directly SPDisplaysDataType[0].sppci_model
    let displays = json.get("SPDisplaysDataType")?.as_array()?;
    for display in displays {
        if let Some(model) = display.get("sppci_model").and_then(|v| v.as_str()) {
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }
        // Fallback: nested ndrvs array
        if let Some(ndrvs) = display.get("spdisplays_ndrvs").and_then(|v| v.as_array()) {
            for drv in ndrvs {
                if let Some(name) = drv.get("_name").and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    None
}

// ─── Windows ─────────────────────────────────────────────────────────────────

/// GPU name on Windows via `wmic path win32_VideoController get Name /format:list`.
///
/// Returns the first non-empty `Name=` value from the output.
#[cfg(target_os = "windows")]
fn windows_gpu_name() -> Option<String> {
    use std::process::Command;

    let output = Command::new("wmic")
        .args([
            "path",
            "win32_VideoController",
            "get",
            "Name",
            "/format:list",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(name) = line.strip_prefix("Name=") {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Verify that detect_gpu_name() returns a String (may be empty on CI).
    #[test]
    fn detect_gpu_name_returns_string() {
        let name = detect_gpu_name();
        // Must be valid UTF-8 and not panic.
        let _ = name.len();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_sysfs_gpu_name_does_not_panic() {
        // May return None in CI environments without a GPU.
        let _ = linux_sysfs_gpu_name();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_lspci_does_not_panic() {
        // lspci may not be installed in all CI environments.
        let _ = linux_lspci_gpu_name();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_gpu_name_does_not_panic() {
        let _ = macos_gpu_name();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_gpu_name_does_not_panic() {
        let _ = windows_gpu_name();
    }

    #[tokio::test]
    async fn get_hardware_returns_all_fields() {
        let Json(value) = get_hardware().await;

        // Flat Agent.exe proto fields must all be present.
        assert!(value.get("cpu_arch").is_some(), "cpu_arch missing");
        assert!(
            value.get("cpu_num_cores").is_some(),
            "cpu_num_cores missing"
        );
        assert!(value.get("cpu_speed").is_some(), "cpu_speed missing");
        assert!(value.get("memory").is_some(), "memory missing");
        assert!(value.get("num_gpus").is_some(), "num_gpus missing");
        assert!(value.get("cpu_vendor").is_some(), "cpu_vendor missing");
        assert!(value.get("cpu_brand").is_some(), "cpu_brand missing");

        // GPU sub-messages must be present.
        assert!(value.get("gpu_1").is_some(), "gpu_1 missing");
        assert!(value.get("gpu_2").is_some(), "gpu_2 missing");
        assert!(value.get("gpu_3").is_some(), "gpu_3 missing");

        // gpu_1 must have the Gpu sub-message fields.
        let gpu_1 = value.get("gpu_1").unwrap();
        assert!(gpu_1.get("vendor_id").is_some(), "gpu_1.vendor_id missing");
        assert!(gpu_1.get("device_id").is_some(), "gpu_1.device_id missing");
        assert!(
            gpu_1.get("video_memory").is_some(),
            "gpu_1.video_memory missing"
        );
        assert!(
            gpu_1.get("integrated").is_some(),
            "gpu_1.integrated missing"
        );
        assert!(gpu_1.get("name").is_some(), "gpu_1.name missing");
    }

    #[tokio::test]
    async fn get_hardware_cpu_arch_is_valid() {
        let Json(value) = get_hardware().await;
        let cpu_arch = value["cpu_arch"]
            .as_u64()
            .expect("cpu_arch should be a number");
        // Only 0 (x86) and 1 (x86_64) are expected values from cascette.
        assert!(cpu_arch <= 1, "cpu_arch out of expected range");
    }
}
