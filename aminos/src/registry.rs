use std::collections::HashMap;

use winreg::enums::*;
use winreg::RegKey;

use crate::software::Detection;

// ── Public API ────────────────────────────────────────────

/// Check if a software is installed via registry.
pub fn detect_installed(detection: &Detection) -> Option<HashMap<String, String>> {
    let dn_lower = detection.display_name.as_ref()?.to_lowercase();
    let publisher_lower = detection.publisher.as_ref().map(|p| p.to_lowercase());

    for check_publisher in [true, false] {
        if check_publisher && publisher_lower.is_none() {
            continue;
        }
        let pred = |name: &str, publisher: Option<&str>| {
            if !name.to_lowercase().contains(&dn_lower) {
                return false;
            }
            if check_publisher {
                if let Some(ref pub_lower) = publisher_lower {
                    return publisher.map_or(false, |p| p.to_lowercase().contains(pub_lower));
                }
            }
            true
        };
        if let Some(r) = try_match(&pred,
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall") { return Some(r); }
        if let Some(r) = try_match(&pred,
            RegKey::predef(HKEY_LOCAL_MACHINE),
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall") { return Some(r); }
        if let Some(r) = try_match(&pred,
            RegKey::predef(HKEY_CURRENT_USER),
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall") { return Some(r); }
    }
    None
}

/// Get the UninstallString for a software.
#[allow(dead_code)]
pub fn get_uninstall_string(detection: &Detection) -> Option<String> {
    detect_installed(detection)
        .and_then(|m| m.get("UninstallString").cloned())
}

/// Scan all installed software from every uninstall registry hive.
pub fn scan_all_installed() -> Vec<HashMap<String, String>> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut results: Vec<HashMap<String, String>> = Vec::new();

    for_each_uninstall(|_subkey_name, subkey| {
        let dn: String = match subkey.get_value("DisplayName") {
            Ok(v) => v,
            Err(_) => return,
        };
        if dn.trim().is_empty() || !seen.insert(dn.clone()) {
            return;
        }
        let mut entry = HashMap::new();
        entry.insert("display_name".into(), dn);
        for f in ["DisplayVersion", "Publisher", "InstallLocation", "UninstallString"] {
            if let Ok(val) = subkey.get_value::<String, _>(f) {
                let k = match f {
                    "DisplayVersion" => "version",
                    "Publisher" => "publisher",
                    "InstallLocation" => "install_path",
                    "UninstallString" => "uninstall_string",
                    _ => continue,
                };
                entry.insert(k.into(), val);
            }
        }
        results.push(entry);
    });
    results
}

// ── Internals ─────────────────────────────────────────────

fn for_each_uninstall(mut cb: impl FnMut(&str, &RegKey)) {
    // HKLM - 64-bit
    if let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", KEY_READ)
    {
        for name in root.enum_keys().flatten() {
            if let Ok(sk) = root.open_subkey_with_flags(&name, KEY_READ) {
                cb(&name, &sk);
            }
        }
    }
    // HKLM - 32-bit on WoW64
    if let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall", KEY_READ)
    {
        for name in root.enum_keys().flatten() {
            if let Ok(sk) = root.open_subkey_with_flags(&name, KEY_READ) {
                cb(&name, &sk);
            }
        }
    }
    // HKCU
    if let Ok(root) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Uninstall", KEY_READ)
    {
        for name in root.enum_keys().flatten() {
            if let Ok(sk) = root.open_subkey_with_flags(&name, KEY_READ) {
                cb(&name, &sk);
            }
        }
    }
}

/// Try to match a predicate against one registry hive path.
fn try_match<F>(
    predicate: F,
    root: RegKey,
    path: &str,
) -> Option<HashMap<String, String>>
where
    F: Fn(&str, Option<&str>) -> bool,
{
    let key = match root.open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return None,
    };
    for subkey_name in key.enum_keys().flatten() {
        let subkey = match key.open_subkey_with_flags(&subkey_name, KEY_READ) {
            Ok(k) => k,
            Err(_) => continue,
        };
        let dn: String = match subkey.get_value("DisplayName") {
            Ok(v) => v,
            Err(_) => continue,
        };
        let publisher: Option<String> = subkey.get_value("Publisher").ok();
        if predicate(&dn, publisher.as_deref()) {
            let mut result = HashMap::new();
            result.insert("DisplayName".into(), dn);
            if let Some(p) = publisher {
                result.insert("Publisher".into(), p);
            }
            for field in ["DisplayVersion", "InstallLocation", "UninstallString"] {
                if let Ok(val) = subkey.get_value::<String, _>(field) {
                    result.insert(field.into(), val);
                }
            }
            return Some(result);
        }
    }
    None
}
