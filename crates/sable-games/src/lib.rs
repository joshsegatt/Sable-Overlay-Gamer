// sable-games: Game detection engine
// Scans Steam, Epic, GOG, EA App, and Windows registry for installed games.

#![allow(unused_imports, unused_variables, dead_code, unused_must_use, unreachable_code, unused_mut, unused_assignments)]

use anyhow::{bail, Context, Result};
use sable_core::{GameEntry, GamePlatform};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};
use uuid::Uuid;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Scan all supported platforms and return deduplicated game entries.
pub fn detect_all_games() -> Vec<GameEntry> {
    let mut games: Vec<GameEntry> = Vec::new();

    match scan_steam() {
        Ok(mut g) => games.append(&mut g),
        Err(e) => warn!("Steam scan failed: {e}"),
    }
    match scan_epic() {
        Ok(mut g) => games.append(&mut g),
        Err(e) => warn!("Epic scan failed: {e}"),
    }
    match scan_gog() {
        Ok(mut g) => games.append(&mut g),
        Err(e) => warn!("GOG scan failed: {e}"),
    }
    match scan_ea() {
        Ok(mut g) => games.append(&mut g),
        Err(e) => warn!("EA scan failed: {e}"),
    }
    match scan_registry() {
        Ok(mut g) => games.append(&mut g),
        Err(e) => warn!("Registry scan failed: {e}"),
    }

    // Deduplicate by exe path (case-insensitive on Windows)
    let mut seen_exes: HashMap<String, bool> = HashMap::new();
    games.retain(|g| {
        let key = g.exe_path.to_lowercase();
        if seen_exes.contains_key(&key) {
            false
        } else {
            seen_exes.insert(key, true);
            true
        }
    });

    games
}

// ─── Steam ────────────────────────────────────────────────────────────────────

fn steam_default_path() -> PathBuf {
    if let Ok(p) = std::env::var("ProgramFiles(x86)") {
        PathBuf::from(p).join("Steam")
    } else {
        PathBuf::from(r"C:\Program Files (x86)\Steam")
    }
}

fn scan_steam() -> Result<Vec<GameEntry>> {
    let steam_path = steam_default_path();
    if !steam_path.exists() {
        // Try registry fallback
        return Ok(vec![]);
    }

    let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
    if !vdf_path.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&vdf_path)
        .with_context(|| format!("Failed to read {}", vdf_path.display()))?;

    // Start with the default Steam path as first library, then add any others from VDF
    let mut library_paths = vec![steam_path.to_string_lossy().to_string()];
    library_paths.extend(parse_vdf_library_paths(&content));
    library_paths.dedup();

    let mut games = Vec::new();

    for lib in library_paths {
        let apps_dir = PathBuf::from(&lib).join("steamapps");
        if !apps_dir.exists() {
            continue;
        }
        let manifests = match fs::read_dir(&apps_dir) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for entry in manifests.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("acf") {
                if let Ok(game) = parse_steam_manifest(&path, &apps_dir) {
                    games.push(game);
                }
            }
        }
    }

    Ok(games)
}

fn parse_vdf_library_paths(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.contains("\"path\"") {
            // VDF line format: "path"		"C:\\Program Files\\..."
            // splitn(5) gives: ["", "path", "\t\t", "C:\\...", ""]
            let parts: Vec<&str> = line.splitn(5, '"').collect();
            if parts.len() >= 4 {
                let path = parts[3].replace("\\\\", "\\");
                if !path.is_empty() {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

fn parse_steam_manifest(path: &Path, apps_dir: &Path) -> Result<GameEntry> {
    let content = fs::read_to_string(path)?;
    let mut name = String::new();
    let mut app_id = String::new();
    let mut install_dir = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("\"name\"") {
            name = extract_vdf_value(line);
        } else if line.starts_with("\"appid\"") {
            app_id = extract_vdf_value(line);
        } else if line.starts_with("\"installdir\"") {
            install_dir = extract_vdf_value(line);
        }
    }

    if name.is_empty() || install_dir.is_empty() {
        bail!("Incomplete manifest: {}", path.display());
    }

    let game_dir = apps_dir.join("common").join(&install_dir);
    // Find primary exe by looking for an exe matching the install dir name
    let exe_path = find_exe_in_dir(&game_dir)
        .unwrap_or_else(|| game_dir.to_string_lossy().to_string());

    Ok(GameEntry {
        id: Uuid::new_v4(),
        name,
        exe_path,
        icon_path: None,
        platform: GamePlatform::Steam,
        last_played: None,
        install_size_mb: None,
        preset_id: None,
    })
}

fn extract_vdf_value(line: &str) -> String {
    let parts: Vec<&str> = line.splitn(4, '"').collect();
    if parts.len() >= 4 {
        parts[3].to_string()
    } else {
        String::new()
    }
}

// ─── Epic Games ───────────────────────────────────────────────────────────────

fn scan_epic() -> Result<Vec<GameEntry>> {
    let manifests_path = if let Ok(p) = std::env::var("ProgramData") {
        PathBuf::from(p)
            .join("Epic")
            .join("EpicGamesLauncher")
            .join("Data")
            .join("Manifests")
    } else {
        PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests")
    };

    if !manifests_path.exists() {
        return Ok(vec![]);
    }

    let mut games = Vec::new();
    let dir = fs::read_dir(&manifests_path)?;

    for item in dir.flatten() {
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) == Some("item") {
            if let Ok(game) = parse_epic_manifest(&path) {
                games.push(game);
            }
        }
    }

    Ok(games)
}

fn parse_epic_manifest(path: &Path) -> Result<GameEntry> {
    let content = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&content)?;

    let name = json["DisplayName"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let install_location = json["InstallLocation"].as_str().unwrap_or_default();
    let launch_exe = json["LaunchExecutable"].as_str().unwrap_or_default();

    if name.is_empty() || install_location.is_empty() {
        bail!("Incomplete Epic manifest: {}", path.display());
    }

    let exe_path = if launch_exe.is_empty() {
        find_exe_in_dir(Path::new(install_location))
            .unwrap_or_else(|| install_location.to_string())
    } else {
        PathBuf::from(install_location)
            .join(launch_exe)
            .to_string_lossy()
            .to_string()
    };

    Ok(GameEntry {
        id: Uuid::new_v4(),
        name,
        exe_path,
        icon_path: None,
        platform: GamePlatform::Epic,
        last_played: None,
        install_size_mb: None,
        preset_id: None,
    })
}

// ─── GOG Galaxy ───────────────────────────────────────────────────────────────

fn scan_gog() -> Result<Vec<GameEntry>> {
    #[cfg(target_os = "windows")]
    return scan_gog_registry();
    #[cfg(not(target_os = "windows"))]
    Ok(vec![])
}

#[cfg(target_os = "windows")]
fn scan_gog_registry() -> Result<Vec<GameEntry>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let root_keys = [
        r"SOFTWARE\GOG.com\Games",
        r"SOFTWARE\WOW6432Node\GOG.com\Games",
    ];

    let mut games = Vec::new();

    for root_key_path in root_keys {
        let wide_root: Vec<u16> = root_key_path.encode_utf16().chain(Some(0)).collect();
        let mut hroot = HKEY::default();
        let open_result = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(wide_root.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hroot,
            )
        };
        if open_result != windows::Win32::Foundation::ERROR_SUCCESS {
            continue;
        }

        let mut idx = 0u32;
        loop {
            let mut subkey_name = [0u16; 256];
            let mut subkey_len = 256u32;
            let enum_result = unsafe {
                RegEnumKeyExW(
                    hroot,
                    idx,
                    Some(windows::core::PWSTR(subkey_name.as_mut_ptr())),
                    &mut subkey_len,
                    None,
                    Some(windows::core::PWSTR::null()),
                    None,
                    None,
                )
            };
            if enum_result != windows::Win32::Foundation::ERROR_SUCCESS {
                break;
            }
            idx += 1;

            let subkey_str = OsString::from_wide(&subkey_name[..subkey_len as usize])
                .to_string_lossy()
                .to_string();
            let full_path = format!("{}\\{}", root_key_path, subkey_str);
            let wide_full: Vec<u16> = full_path.encode_utf16().chain(Some(0)).collect();
            let mut hgame = HKEY::default();
            if unsafe {
                RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR(wide_full.as_ptr()),
                    Some(0),
                    KEY_READ,
                    &mut hgame,
                )
            } != windows::Win32::Foundation::ERROR_SUCCESS {
                continue;
            }

            let name = read_reg_string(hgame, "GAMENAME")
                .or_else(|_| read_reg_string(hgame, "gameName"))
                .unwrap_or_else(|_| subkey_str.clone());
            let install_path = read_reg_string(hgame, "PATH")
                .or_else(|_| read_reg_string(hgame, "path"))
                .unwrap_or_default();
            let exe_path = read_reg_string(hgame, "EXE")
                .or_else(|_| read_reg_string(hgame, "GameEXE"))
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    find_exe_in_dir(Path::new(&install_path))
                        .unwrap_or_else(|| install_path.clone())
                });

            unsafe { RegCloseKey(hgame) };

            if name.is_empty() || install_path.is_empty() {
                continue;
            }

            games.push(GameEntry {
                id: Uuid::new_v4(),
                name,
                exe_path,
                icon_path: None,
                platform: GamePlatform::Gog,
                last_played: None,
                install_size_mb: None,
                preset_id: None,
            });
        }

        unsafe { RegCloseKey(hroot) };
    }

    Ok(games)
}

// ─── EA App ───────────────────────────────────────────────────────────────────

fn scan_ea() -> Result<Vec<GameEntry>> {
    let ea_path = if let Ok(p) = std::env::var("ProgramData") {
        PathBuf::from(p).join("EA Desktop").join("InstallData")
    } else {
        PathBuf::from(r"C:\ProgramData\EA Desktop\InstallData")
    };

    if !ea_path.exists() {
        return Ok(vec![]);
    }

    let mut games = Vec::new();
    if let Ok(dir) = fs::read_dir(&ea_path) {
        for item in dir.flatten() {
            if item.path().is_dir() {
                let name = item
                    .file_name()
                    .to_string_lossy()
                    .replace('_', " ")
                    .replace('-', " ");
                let exe = find_exe_in_dir(&item.path())
                    .unwrap_or_else(|| item.path().to_string_lossy().to_string());
                games.push(GameEntry {
                    id: Uuid::new_v4(),
                    name,
                    exe_path: exe,
                    icon_path: None,
                    platform: GamePlatform::Ea,
                    last_played: None,
                    install_size_mb: None,
                    preset_id: None,
                });
            }
        }
    }

    Ok(games)
}

// ─── Windows Uninstall Registry ───────────────────────────────────────────────

fn scan_registry() -> Result<Vec<GameEntry>> {
    // Generic scan of HKLM uninstall keys looking for games
    // Heuristic: filter by common game-related strings in DisplayName
    let mut games = Vec::new();

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::System::Registry::*;

        let keys = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ];

        for key_path in keys {
            if let Ok(entries) = read_uninstall_key(key_path) {
                for e in entries {
                    games.push(e);
                }
            }
        }
    }

    Ok(games)
}

#[cfg(target_os = "windows")]
fn read_uninstall_key(key_path: &str) -> Result<Vec<GameEntry>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let wide_key: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        ) != windows::Win32::Foundation::ERROR_SUCCESS
        {
            return Ok(vec![]);
        }
    }

    let mut games = Vec::new();
    let mut idx = 0u32;

    loop {
        let mut subkey_name = [0u16; 256];
        let mut subkey_len = 256u32;

        let result = unsafe {
            windows::Win32::System::Registry::RegEnumKeyExW(
                hkey,
                idx,
                Some(windows::core::PWSTR(subkey_name.as_mut_ptr())),
                &mut subkey_len,
                None,
                Some(windows::core::PWSTR::null()),
                None,
                None,
            )
        };

        if result != windows::Win32::Foundation::ERROR_SUCCESS {
            break;
        }

        idx += 1;
        let subkey_str =
            OsString::from_wide(&subkey_name[..subkey_len as usize]).to_string_lossy().to_string();

        // Try to open subkey and read DisplayName + InstallLocation
        let full_path = format!("{}\\{}", key_path, subkey_str);
        if let Ok(entry) = read_uninstall_entry(&full_path) {
            // Filter for likely games: skip small utilities, Microsoft/Windows entries
            if is_likely_game(&entry.name) {
                games.push(entry);
            }
        }
    }

    unsafe { RegCloseKey(hkey) };
    Ok(games)
}

#[cfg(target_os = "windows")]
fn read_uninstall_entry(key_path: &str) -> Result<GameEntry> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let wide_key: Vec<u16> = key_path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();

    unsafe {
        let open_err = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if open_err != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("Registry open failed: {:?}", open_err));
        }
    }

    let display_name = read_reg_string(hkey, "DisplayName")?;
    let install_location = read_reg_string(hkey, "InstallLocation").unwrap_or_default();
    let display_icon = read_reg_string(hkey, "DisplayIcon").unwrap_or_default();

    unsafe { RegCloseKey(hkey) };

    let exe_path = if !install_location.is_empty() {
        find_exe_in_dir(Path::new(&install_location))
            .unwrap_or_else(|| install_location.clone())
    } else {
        // DisplayIcon often contains path to exe
        display_icon
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    };

    Ok(GameEntry {
        id: Uuid::new_v4(),
        name: display_name,
        exe_path,
        icon_path: Some(display_icon),
        platform: GamePlatform::Custom,
        last_played: None,
        install_size_mb: None,
        preset_id: None,
    })
}

#[cfg(target_os = "windows")]
fn read_reg_string(hkey: windows::Win32::System::Registry::HKEY, name: &str) -> Result<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; 1024];
    let mut buf_len = (buf.len() * 2) as u32;
    let mut reg_type = REG_VALUE_TYPE::default();

    unsafe {
        let qry_err = RegQueryValueExW(
            hkey,
            PCWSTR(wide_name.as_ptr()),
            None,
            Some(&mut reg_type),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut buf_len),
        );
        if qry_err != windows::Win32::Foundation::ERROR_SUCCESS {
            return Err(anyhow::anyhow!("RegQueryValueEx failed: {:?}", qry_err));
        }
    }

    let len = buf_len as usize / 2;
    let trimmed = &buf[..len.min(buf.len())];
    let s = OsString::from_wide(trimmed)
        .to_string_lossy()
        .trim_end_matches('\0')
        .to_string();
    Ok(s)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn use_registry_scan(
    _key: &str,
    _path_val: &str,
    _name_val: &str,
    platform: GamePlatform,
) -> Result<Vec<GameEntry>> {
    // Generic stub — platform-specific implementations delegate here
    Ok(vec![])
}

fn find_exe_in_dir(dir: &Path) -> Option<String> {
    if !dir.exists() {
        return None;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        // First try: exact match with directory name
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut exes: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("exe")
            })
            .map(|e| e.path())
            .collect();

        // Prefer an exe whose name matches the directory (likely launcher)
        exes.sort_by_key(|p| {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            // Lower sort key = preferred
            if name == dir_name {
                0u8
            } else if name.contains("launcher") || name.contains("epic") || name.contains("setup") {
                2
            } else {
                1
            }
        });

        return exes.first().map(|p| p.to_string_lossy().to_string());
    }
    None
}

fn is_likely_game(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    // Exclude known non-game publisher/system entries
    let exclude_keywords = [
        "microsoft", "windows", "visual c++", "directx", "net framework",
        "redistributable", ".net", "driver", "runtime", "sdk", "update",
        "chrome", "firefox", "edge", "adobe", "office", "steam",
    ];
    for kw in exclude_keywords {
        if name_lower.contains(kw) {
            return false;
        }
    }
    true
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_game_filters_runtimes() {
        assert!(!is_likely_game("Microsoft Visual C++ 2015 Redistributable"));
        assert!(!is_likely_game("DirectX End-User Runtimes"));
        assert!(is_likely_game("Cyberpunk 2077"));
        assert!(is_likely_game("Counter-Strike 2"));
    }

    #[test]
    fn test_extract_vdf_value() {
        let line = r#"		"name"		"Half-Life 2""#;
        assert_eq!(extract_vdf_value(line), "Half-Life 2");
    }
}
