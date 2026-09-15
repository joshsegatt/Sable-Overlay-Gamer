// sable-presets: Optimization preset engine with rollback
//
// Every change is snapshotted before apply.
// Rollback restores the exact state captured in the snapshot.
// Risk levels gate changes: Low is auto-applicable, Medium/High require explicit confirm.

#![allow(unused_imports, unused_variables, dead_code, unused_mut)]

use anyhow::{Context, Result};
use chrono::Utc;
use sable_core::{Preset, PresetChange, PresetChange::*, RiskLevel, RollbackSnapshot};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;
#[cfg(target_os = "windows")]
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW,
    QueryServiceStatus, StartServiceW, SERVICE_AUTO_START, SERVICE_CHANGE_CONFIG,
    SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_NO_CHANGE, SERVICE_QUERY_STATUS,
    SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS, SERVICE_STOP, SC_MANAGER_CONNECT,
};

/// Built-in catalog. Medium = needs confirm. High is reserved.
pub fn bundled_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            name: "High Performance Power Plan".to_string(),
            description: "Switches Windows to the High Performance power plan. Prevents CPU frequency scaling during gameplay and reduces input latency caused by sleep states. Most impactful on laptops and desktop systems on Balanced plan.".to_string(),
            risk: RiskLevel::Low,
            changes: vec![SetPowerPlan {
                plan_guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c".to_string(),
                plan_name: "High performance".to_string(),
            }],
            is_bundled: true,
            is_applied: false,
        },
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            name: "Maximize Foreground CPU Priority".to_string(),
            description: "Sets Windows CPU time distribution to heavily favor the active foreground application. Background processes receive fewer CPU quanta. Reversible registry write to Win32PrioritySeparation.".to_string(),
            risk: RiskLevel::Medium,
            changes: vec![SetPrioritySeparation { value: 38 }],
            is_bundled: true,
            is_applied: false,
        },
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
            name: "Disable Xbox Game DVR".to_string(),
            description: "Disables background game recording (Xbox Game DVR/Game Bar capture). Eliminates GPU overhead from continuous video encoding. Rollback restores the previous registry values.".to_string(),
            risk: RiskLevel::Low,
            changes: vec![DisableGameDvr, DisableGameBar],
            is_bundled: true,
            is_applied: false,
        },
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
            name: "Enable Windows Game Mode".to_string(),
            description: "Activates Windows Game Mode which deprioritizes background processes and may allocate more resources to the active game process. Effect varies by system configuration.".to_string(),
            risk: RiskLevel::Low,
            changes: vec![EnableGameMode],
            is_bundled: true,
            is_applied: false,
        },
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000005").unwrap(),
            name: "Disable Search Indexer During Gaming".to_string(),
            description: "Stops the Windows Search Indexer (WSearch) and sets it to manual start. Rollback restores automatic start and restarts the service if it was running.".to_string(),
            risk: RiskLevel::Medium,
            changes: vec![DisableSearchIndexer],
            is_bundled: true,
            is_applied: false,
        },
        Preset {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000006").unwrap(),
            name: "HAGS Advisory".to_string(),
            description: "Hardware Accelerated GPU Scheduling (HAGS) can improve latency on some GPU/driver combinations but causes issues on others. Requires driver reload after toggle.".to_string(),
            risk: RiskLevel::Medium,
            changes: vec![SetHags { enabled: true }],
            is_bundled: true,
            is_applied: false,
        },
    ]
}

pub struct PresetEngine {
    snapshots_dir: PathBuf,
    applied_presets: HashMap<Uuid, RollbackSnapshot>,
}

impl PresetEngine {
    pub fn new() -> Self {
        let snapshots_dir = get_app_data_dir().join("snapshots");
        let _ = fs::create_dir_all(&snapshots_dir);
        Self {
            snapshots_dir,
            applied_presets: HashMap::new(),
        }
    }

    pub fn apply(&mut self, preset: &Preset) -> Result<()> {
        info!("Applying preset: {} ({})", preset.name, preset.id);

        let snapshot = self.capture_snapshot(preset)?;

        for change in &preset.changes {
            match apply_change(change) {
                Ok(()) => info!("  Applied: {change:?}"),
                Err(e) => {
                    warn!("  Failed to apply {change:?}: {e}");
                    if let Ok(snap) = serde_json::to_value(&snapshot) {
                        let _ = self.rollback_from_value(&snap);
                    }
                    return Err(e).context(format!("Preset apply failed at change: {change:?}"));
                }
            }
        }

        self.save_snapshot(&snapshot)?;
        self.applied_presets.insert(preset.id, snapshot);
        Ok(())
    }

    pub fn rollback(&mut self, preset_id: Uuid) -> Result<()> {
        let snapshot = self
            .applied_presets
            .remove(&preset_id)
            .or_else(|| self.load_snapshot(preset_id).ok().flatten())
            .context(format!("No rollback snapshot found for preset {preset_id}"))?;

        info!("Rolling back preset: {preset_id}");
        self.rollback_from_value_owned(&snapshot)?;
        self.delete_snapshot(preset_id)?;
        Ok(())
    }

    fn capture_snapshot(&self, preset: &Preset) -> Result<RollbackSnapshot> {
        let mut state = serde_json::json!({});

        for change in &preset.changes {
            match change {
                SetPowerPlan { .. } => {
                    let current_guid = get_current_power_plan_guid().unwrap_or_default();
                    let current_name = get_current_power_plan_name().unwrap_or_default();
                    state["PowerPlan"] = serde_json::json!({
                        "guid": current_guid,
                        "name": current_name,
                    });
                }
                SetHags { .. } => {
                    let current = get_hags_state().unwrap_or(false);
                    state["Hags"] = serde_json::json!({ "enabled": current });
                }
                DisableGameDvr | DisableGameBar => {
                    let dvr = get_game_dvr_state().unwrap_or(true);
                    let bar = get_game_bar_state().unwrap_or(true);
                    state["GameDvr"] = serde_json::json!({ "enabled": dvr });
                    state["GameBar"] = serde_json::json!({ "enabled": bar });
                }
                EnableGameMode => {
                    let mode = get_game_mode_state().unwrap_or(false);
                    state["GameMode"] = serde_json::json!({ "enabled": mode });
                }
                SetPrioritySeparation { .. } => {
                    let current = get_priority_separation().unwrap_or(2);
                    state["PrioritySeparation"] = serde_json::json!({ "value": current });
                }
                DisableSearchIndexer => {
                    let running = is_search_indexer_running();
                    state["SearchIndexer"] = serde_json::json!({ "was_running": running });
                }
                _ => {}
            }
        }

        Ok(RollbackSnapshot {
            id: Uuid::new_v4(),
            preset_id: preset.id,
            timestamp: Utc::now(),
            state,
        })
    }

    fn rollback_from_value(&self, state: &serde_json::Value) -> Result<()> {
        self.rollback_state(state)
    }

    fn rollback_from_value_owned(&self, snapshot: &RollbackSnapshot) -> Result<()> {
        self.rollback_state(&snapshot.state)
    }

    fn rollback_state(&self, state: &serde_json::Value) -> Result<()> {
        for action in rollback_plan(state) {
            apply_restore(&action)?;
        }
        Ok(())
    }

    fn save_snapshot(&self, snap: &RollbackSnapshot) -> Result<()> {
        let path = self.snapshots_dir.join(format!("{}.json", snap.preset_id));
        let json = serde_json::to_string_pretty(snap)?;
        fs::write(&path, json).context("Failed to write rollback snapshot")?;
        Ok(())
    }

    fn load_snapshot(&self, preset_id: Uuid) -> Result<Option<RollbackSnapshot>> {
        let path = self.snapshots_dir.join(format!("{preset_id}.json"));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let snap: RollbackSnapshot = serde_json::from_str(&content)?;
        Ok(Some(snap))
    }

    fn delete_snapshot(&self, preset_id: Uuid) -> Result<()> {
        let path = self.snapshots_dir.join(format!("{preset_id}.json"));
        if path.exists() {
            fs::remove_file(&path).context("Failed to delete snapshot")?;
        }
        Ok(())
    }
}

impl Default for PresetEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// What rollback will write. Pure function so tests can assert the old
/// "only restore when false" bug cannot return.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RestoreAction {
    PowerPlan { guid: String, name: String },
    Hags { enabled: bool },
    GameDvr { enabled: bool },
    GameBar { enabled: bool },
    GameMode { enabled: bool },
    PrioritySeparation { value: u32 },
    SearchIndexer { was_running: bool },
}

fn rollback_plan(state: &serde_json::Value) -> Vec<RestoreAction> {
    let mut out = Vec::new();
    if let Some(pp) = state.get("PowerPlan") {
        if let (Some(guid), Some(name)) = (
            pp.get("guid").and_then(|v| v.as_str()),
            pp.get("name").and_then(|v| v.as_str()),
        ) {
            if !guid.is_empty() {
                out.push(RestoreAction::PowerPlan {
                    guid: guid.to_string(),
                    name: name.to_string(),
                });
            }
        }
    }
    if let Some(enabled) = state
        .get("Hags")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
    {
        out.push(RestoreAction::Hags { enabled });
    }
    if let Some(enabled) = state
        .get("GameDvr")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
    {
        out.push(RestoreAction::GameDvr { enabled });
    }
    if let Some(enabled) = state
        .get("GameBar")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
    {
        out.push(RestoreAction::GameBar { enabled });
    }
    if let Some(enabled) = state
        .get("GameMode")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
    {
        out.push(RestoreAction::GameMode { enabled });
    }
    if let Some(value) = state
        .get("PrioritySeparation")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_u64())
    {
        out.push(RestoreAction::PrioritySeparation {
            value: value as u32,
        });
    }
    if state.get("SearchIndexer").is_some() {
        let was_running = state
            .get("SearchIndexer")
            .and_then(|v| v.get("was_running"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.push(RestoreAction::SearchIndexer { was_running });
    }
    out
}

fn apply_restore(action: &RestoreAction) -> Result<()> {
    match action {
        RestoreAction::PowerPlan { guid, name } => set_power_plan(guid, name),
        RestoreAction::Hags { enabled } => set_hags(*enabled),
        RestoreAction::GameDvr { enabled } => set_game_dvr(*enabled),
        RestoreAction::GameBar { enabled } => set_game_bar(*enabled),
        RestoreAction::GameMode { enabled } => set_game_mode(*enabled),
        RestoreAction::PrioritySeparation { value } => set_priority_separation(*value),
        RestoreAction::SearchIndexer { was_running } => restore_search_indexer(*was_running),
    }
}

fn apply_change(change: &PresetChange) -> Result<()> {
    match change {
        SetPowerPlan {
            plan_guid,
            plan_name,
        } => set_power_plan(plan_guid, plan_name),
        SetProcessPriority { .. } => {
            anyhow::bail!("SetProcessPriority is not implemented; refusing to mark preset applied")
        }
        SetCpuAffinity { .. } => {
            anyhow::bail!("SetCpuAffinity is not implemented; refusing to mark preset applied")
        }
        SetHags { enabled } => set_hags(*enabled),
        DisableGameDvr => set_game_dvr(false),
        DisableGameBar => set_game_bar(false),
        EnableGameMode => set_game_mode(true),
        ThrottleBackgroundProcesses { .. } => {
            anyhow::bail!("ThrottleBackgroundProcesses is not implemented; refusing to mark preset applied")
        }
        NvidiaDrsSetting { .. } => {
            anyhow::bail!("NvidiaDrsSetting is not implemented; refusing to mark preset applied")
        }
        SetPrioritySeparation { value } => set_priority_separation(*value),
        DisableSearchIndexer => stop_search_indexer(),
    }
}

fn set_power_plan(guid_str: &str, name: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Power::PowerSetActiveScheme;

        if let Ok(guid) = parse_guid(guid_str) {
            unsafe {
                let err = PowerSetActiveScheme(None, Some(&guid));
                if err != windows::Win32::Foundation::ERROR_SUCCESS {
                    return Err(anyhow::anyhow!("PowerSetActiveScheme failed: {:?}", err));
                }
            }
            info!("Power plan set to: {name}");
        } else {
            return Err(anyhow::anyhow!("invalid power plan GUID: {guid_str}"));
        }
    }
    let _ = (guid_str, name);
    Ok(())
}

pub fn get_current_power_plan_guid() -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Power::PowerGetActiveScheme;
        unsafe {
            let mut scheme_guid: *mut windows::core::GUID = std::ptr::null_mut();
            let err = PowerGetActiveScheme(None, &mut scheme_guid);
            if err != windows::Win32::Foundation::ERROR_SUCCESS {
                return Err(anyhow::anyhow!("PowerGetActiveScheme failed: {:?}", err));
            }
            if !scheme_guid.is_null() {
                let guid = *scheme_guid;
                return Ok(format_guid(&guid));
            }
        }
    }
    Ok(String::new())
}

pub fn get_current_power_plan_name() -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Power::{PowerGetActiveScheme, PowerReadFriendlyName};
        unsafe {
            let mut scheme_guid: *mut windows::core::GUID = std::ptr::null_mut();
            let err = PowerGetActiveScheme(None, &mut scheme_guid);
            if err != windows::Win32::Foundation::ERROR_SUCCESS || scheme_guid.is_null() {
                return Ok("Unknown".to_string());
            }
            let guid = *scheme_guid;

            let mut buf_size: u32 = 0;
            PowerReadFriendlyName(None, Some(&guid), None, None, None, &mut buf_size);
            if buf_size == 0 {
                return Ok("Unknown".to_string());
            }

            let mut buf = vec![0u8; buf_size as usize];
            let err = PowerReadFriendlyName(
                None,
                Some(&guid),
                None,
                None,
                Some(buf.as_mut_ptr()),
                &mut buf_size,
            );
            if err != windows::Win32::Foundation::ERROR_SUCCESS {
                return Ok("Unknown".to_string());
            }

            let wide: &[u16] =
                std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf_size as usize / 2);
            let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
            let name = String::from_utf16_lossy(&wide[..end]).trim().to_string();
            return Ok(if name.is_empty() {
                "Unknown".to_string()
            } else {
                name
            });
        }
    }
    Ok("Unknown".to_string())
}

fn parse_guid(s: &str) -> Result<windows::core::GUID> {
    let s = s.replace('-', "");
    if s.len() < 32 {
        return Err(anyhow::anyhow!("Invalid GUID: {s}"));
    }
    let data1 = u32::from_str_radix(&s[0..8], 16)?;
    let data2 = u16::from_str_radix(&s[8..12], 16)?;
    let data3 = u16::from_str_radix(&s[12..16], 16)?;
    let data4_str = &s[16..32];
    let mut data4 = [0u8; 8];
    for i in 0..8 {
        data4[i] = u8::from_str_radix(&data4_str[i * 2..i * 2 + 2], 16)?;
    }
    Ok(windows::core::GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

fn format_guid(g: &windows::core::GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    )
}

fn get_hags_state() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Registry::*;

        let key_wide: Vec<u16> = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut hkey = HKEY::default();
        unsafe {
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key_wide.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            ) == windows::Win32::Foundation::ERROR_SUCCESS
            {
                let val_name: Vec<u16> = "HwSchMode\0".encode_utf16().collect();
                let mut val: u32 = 0;
                let mut val_size = 4u32;
                if RegQueryValueExW(
                    hkey,
                    PCWSTR(val_name.as_ptr()),
                    None,
                    None,
                    Some(&mut val as *mut _ as *mut u8),
                    Some(&mut val_size),
                )
                .is_ok()
                {
                    let _ = RegCloseKey(hkey);
                    return Ok(val == 2);
                }
                let _ = RegCloseKey(hkey);
            }
        }
    }
    Ok(false)
}

pub fn get_hags_state_pub() -> anyhow::Result<bool> {
    get_hags_state()
}

fn set_hags(enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Registry::*;

        let key_wide: Vec<u16> = r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut hkey = HKEY::default();
        unsafe {
            let err = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key_wide.as_ptr()),
                Some(0),
                KEY_WRITE,
                &mut hkey,
            );
            if err != windows::Win32::Foundation::ERROR_SUCCESS {
                return Err(anyhow::anyhow!("Registry open failed: {:?}", err));
            }

            let val_name: Vec<u16> = "HwSchMode\0".encode_utf16().collect();
            let val: u32 = if enabled { 2 } else { 1 };
            let err2 = RegSetValueExW(
                hkey,
                PCWSTR(val_name.as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&val.to_le_bytes()),
            );
            if err2 != windows::Win32::Foundation::ERROR_SUCCESS {
                let _ = RegCloseKey(hkey);
                return Err(anyhow::anyhow!("RegSetValueEx failed: {:?}", err2));
            }

            let _ = RegCloseKey(hkey);
        }
        info!("HAGS set to: {enabled} — driver reload required");
    }
    let _ = enabled;
    Ok(())
}

fn get_game_dvr_state() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        return read_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
            "AppCaptureEnabled",
        )
        .map(|v| v != 0);
    }
    Ok(true)
}

fn set_game_dvr(enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        write_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\GameDVR",
            "AppCaptureEnabled",
            if enabled { 1 } else { 0 },
        )?;
        write_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"System\GameConfigStore",
            "GameDVR_Enabled",
            if enabled { 1 } else { 0 },
        )?;
    }
    let _ = enabled;
    Ok(())
}

fn get_game_bar_state() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        return read_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\GameBar",
            "ShowStartupPanel",
        )
        .map(|v| v != 0);
    }
    Ok(true)
}

fn set_game_bar(enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        write_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\GameBar",
            "ShowStartupPanel",
            if enabled { 1 } else { 0 },
        )?;
    }
    let _ = enabled;
    Ok(())
}

fn get_game_mode_state() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        return read_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\GameBar",
            "AutoGameModeEnabled",
        )
        .map(|v| v != 0);
    }
    Ok(false)
}

fn set_game_mode(enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        write_reg_dword(
            windows::Win32::System::Registry::HKEY_CURRENT_USER,
            r"Software\Microsoft\GameBar",
            "AutoGameModeEnabled",
            if enabled { 1 } else { 0 },
        )?;
    }
    let _ = enabled;
    Ok(())
}

#[cfg(target_os = "windows")]
fn read_reg_dword(
    root: windows::Win32::System::Registry::HKEY,
    key_path: &str,
    value_name: &str,
) -> Result<u32> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let key_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();

    unsafe {
        let open_err = RegOpenKeyExW(root, PCWSTR(key_wide.as_ptr()), Some(0), KEY_READ, &mut hkey);
        if open_err != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("Registry open failed: {:?}", open_err));
        }

        let mut val: u32 = 0;
        let mut val_size = 4u32;
        let result = RegQueryValueExW(
            hkey,
            PCWSTR(val_wide.as_ptr()),
            None,
            None,
            Some(&mut val as *mut _ as *mut u8),
            Some(&mut val_size),
        );

        let _ = RegCloseKey(hkey);
        if result != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("RegQueryValueEx failed: {:?}", result));
        }
        Ok(val)
    }
}

#[cfg(target_os = "windows")]
fn write_reg_dword(
    root: windows::Win32::System::Registry::HKEY,
    key_path: &str,
    value_name: &str,
    value: u32,
) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let key_wide: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();

    unsafe {
        let create_err = RegCreateKeyExW(
            root,
            PCWSTR(key_wide.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            Some(&mut disposition),
        );
        if create_err != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("RegCreateKeyEx failed: {:?}", create_err));
        }

        let result = RegSetValueExW(
            hkey,
            PCWSTR(val_wide.as_ptr()),
            Some(0),
            REG_DWORD,
            Some(&value.to_le_bytes()),
        );

        let _ = RegCloseKey(hkey);
        if result != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("RegSetValueEx failed: {:?}", result));
        }
    }
    Ok(())
}

fn get_priority_separation() -> Result<u32> {
    #[cfg(target_os = "windows")]
    {
        return read_reg_dword(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\PriorityControl",
            "Win32PrioritySeparation",
        );
    }
    Ok(2)
}

fn set_priority_separation(value: u32) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        write_reg_dword(
            windows::Win32::System::Registry::HKEY_LOCAL_MACHINE,
            r"SYSTEM\CurrentControlSet\Control\PriorityControl",
            "Win32PrioritySeparation",
            value,
        )?;
        info!("Win32PrioritySeparation set to {value}");
    }
    let _ = value;
    Ok(())
}

fn is_search_indexer_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Services::*;

        let svc_name: Vec<u16> = "WSearch\0".encode_utf16().collect();
        unsafe {
            let sc = match OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) {
                Ok(h) => h,
                Err(_) => return false,
            };
            let svc = match OpenServiceW(sc, PCWSTR(svc_name.as_ptr()), SERVICE_QUERY_STATUS) {
                Ok(h) => h,
                Err(_) => {
                    let _ = CloseServiceHandle(sc);
                    return false;
                }
            };
            let mut status = SERVICE_STATUS::default();
            let running =
                QueryServiceStatus(svc, &mut status).is_ok() && status.dwCurrentState == SERVICE_RUNNING;
            let _ = CloseServiceHandle(svc);
            let _ = CloseServiceHandle(sc);
            return running;
        }
    }
    false
}

fn stop_search_indexer() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Services::*;

        let svc_name: Vec<u16> = "WSearch\0".encode_utf16().collect();
        unsafe {
            let sc = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| anyhow::anyhow!("OpenSCManager failed: {e}"))?;
            let svc = OpenServiceW(
                sc,
                PCWSTR(svc_name.as_ptr()),
                SERVICE_STOP | SERVICE_CHANGE_CONFIG,
            )
            .map_err(|e| {
                let _ = CloseServiceHandle(sc);
                anyhow::anyhow!("OpenService (WSearch) failed: {e}")
            })?;

            let _ = ChangeServiceConfigW(
                svc,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                SERVICE_DEMAND_START,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            );

            let mut ss = SERVICE_STATUS::default();
            let _ = ControlService(svc, SERVICE_CONTROL_STOP, &mut ss);

            let _ = CloseServiceHandle(svc);
            let _ = CloseServiceHandle(sc);
        }
        info!("Windows Search Indexer (WSearch) stopped and set to manual start");
    }
    Ok(())
}

fn restore_search_indexer(was_running: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Services::*;

        let svc_name: Vec<u16> = "WSearch\0".encode_utf16().collect();
        unsafe {
            let sc = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT)
                .map_err(|e| anyhow::anyhow!("OpenSCManager failed: {e}"))?;
            let svc = OpenServiceW(
                sc,
                PCWSTR(svc_name.as_ptr()),
                SERVICE_START | SERVICE_CHANGE_CONFIG,
            )
            .map_err(|e| {
                let _ = CloseServiceHandle(sc);
                anyhow::anyhow!("OpenService (WSearch) failed: {e}")
            })?;

            let _ = ChangeServiceConfigW(
                svc,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                SERVICE_AUTO_START,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            );

            if was_running {
                let _ = StartServiceW(svc, None);
            }

            let _ = CloseServiceHandle(svc);
            let _ = CloseServiceHandle(sc);
        }
        info!(
            "WSearch start type restored to AUTO; running={} restored",
            was_running
        );
    }
    let _ = was_running;
    Ok(())
}

fn get_app_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("APPDATA") {
        PathBuf::from(p).join("Sable")
    } else {
        PathBuf::from(r"C:\Users\Default\AppData\Roaming\Sable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_presets_have_unique_ids() {
        let presets = bundled_presets();
        let ids: std::collections::HashSet<String> =
            presets.iter().map(|p| p.id.to_string()).collect();
        assert_eq!(ids.len(), presets.len(), "All preset IDs must be unique");
    }

    #[test]
    fn priority_and_indexer_are_medium() {
        let presets = bundled_presets();
        let pri = presets
            .iter()
            .find(|p| p.name.contains("Foreground CPU"))
            .unwrap();
        let idx = presets
            .iter()
            .find(|p| p.name.contains("Search Indexer"))
            .unwrap();
        assert_eq!(pri.risk, RiskLevel::Medium);
        assert_eq!(idx.risk, RiskLevel::Medium);
    }

    #[test]
    fn rollback_restores_dvr_when_it_was_enabled() {
        let state = serde_json::json!({
            "GameDvr": { "enabled": true },
            "GameBar": { "enabled": true },
            "GameMode": { "enabled": false }
        });
        let plan = rollback_plan(&state);
        assert!(
            plan.contains(&RestoreAction::GameDvr { enabled: true }),
            "old bug skipped restore when snapshot enabled=true"
        );
        assert!(plan.contains(&RestoreAction::GameBar { enabled: true }));
        assert!(plan.contains(&RestoreAction::GameMode { enabled: false }));
    }

    #[test]
    fn rollback_restores_dvr_when_it_was_disabled() {
        let state = serde_json::json!({ "GameDvr": { "enabled": false } });
        let plan = rollback_plan(&state);
        assert_eq!(plan, vec![RestoreAction::GameDvr { enabled: false }]);
    }

    #[test]
    fn unimplemented_changes_do_not_succeed() {
        let err = apply_change(&SetProcessPriority { priority: 2 }).unwrap_err();
        assert!(err.to_string().contains("not implemented"));
        let err = apply_change(&SetCpuAffinity { mask: 1 }).unwrap_err();
        assert!(err.to_string().contains("not implemented"));
        let err = apply_change(&NvidiaDrsSetting {
            setting_id: 1,
            value: 1,
            description: "x".into(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("not implemented"));
    }

    #[test]
    fn test_parse_guid() {
        let result = parse_guid("8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c");
        assert!(result.is_ok());
    }

    #[test]
    fn test_preset_engine_new() {
        let engine = PresetEngine::new();
        assert!(engine.applied_presets.is_empty());
    }
}
