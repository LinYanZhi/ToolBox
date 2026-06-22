use std::collections::HashMap;

use winreg::enums::*;
use winreg::RegKey;
use winreg::HKEY;

/// 软件检测条件：显示名 + 发布者。
#[derive(Debug, Clone)]
pub struct Detection {
    pub display_name: Option<String>,
    pub publisher: Option<String>,
}

/// 检查某个软件是否已安装（通过注册表）。
///
/// 遍历三个 Uninstall 注册表路径：
///   - `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
///   - `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
///   - `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`
///
/// 返回匹配到的注册表键值对。
pub fn detect_installed(detection: &Detection) -> Option<HashMap<String, String>> {
    let dn_lower = detection.display_name.as_ref()?;
    let publisher = detection.publisher.as_deref();
    detect_installed_by(dn_lower, publisher)
}

/// 使用原始字符串参数检测已安装软件（无需构造 Detection）。
///
/// 当调用方有自己的 detection 类型（如带 serde）时，可直接使用此函数，
/// 避免重复实现注册表遍历逻辑。
pub fn detect_installed_by(display_name: &str, publisher: Option<&str>) -> Option<HashMap<String, String>> {
    let dn_lower = display_name.to_lowercase();
    let publisher_lower = publisher.map(|p| p.to_lowercase());

    for check_publisher in [true, false] {
        if check_publisher && publisher_lower.is_none() {
            continue;
        }
        let pred = |name: &str, pub_: Option<&str>| {
            if !name.to_lowercase().contains(&dn_lower) {
                return false;
            }
            if check_publisher {
                if let Some(ref pub_lower) = publisher_lower {
                    return pub_.map_or(false, |p| p.to_lowercase().contains(pub_lower));
                }
            }
            true
        };
        let hives: &[(HKEY, &str)] = &[
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        ];
        for &(root, path) in hives {
            if let Some(r) = try_match(&pred, RegKey::predef(root), path) {
                return Some(r);
            }
        }
    }
    None
}

/// 获取软件的 UninstallString。
pub fn get_uninstall_string(detection: &Detection) -> Option<String> {
    detect_installed(detection).and_then(|m| m.get("UninstallString").cloned())
}

/// 查找所有匹配的已安装软件（返回所有匹配项，而非第一个）。
///
/// 遍历三个 Uninstall 注册表路径，返回所有满足 DisplayName + Publisher 条件的条目。
pub fn detect_all_installed_by(display_name: &str, publisher: Option<&str>) -> Vec<HashMap<String, String>> {
    let dn_lower = display_name.to_lowercase();
    let publisher_lower = publisher.map(|p| p.to_lowercase());

    for check_publisher in [true, false] {
        if check_publisher && publisher_lower.is_none() {
            continue;
        }
        let mut results = Vec::new();
        let hives: &[(HKEY, &str)] = &[
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        ];
        for &(root, path) in hives {
            collect_matches(&dn_lower, &publisher_lower, check_publisher, RegKey::predef(root), path, &mut results);
        }
        if !results.is_empty() {
            return results;
        }
    }
    vec![]
}

/// 扫描所有已安装的软件（从全部 Uninstall 注册表）。
///
/// 自动过滤掉 Windows 系统组件（SystemComponent=1）和子组件（含 ParentDisplayName 的），
/// 保持与 Windows 卸载面板（appwiz.cpl）一致的列表。
pub fn scan_all_installed() -> Vec<HashMap<String, String>> {
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for_each_uninstall(|_subkey_name, subkey| {
        let dn: String = match subkey.get_value("DisplayName") {
            Ok(v) => v,
            Err(_) => return,
        };
        if dn.trim().is_empty() || !seen.insert(dn.clone()) {
            return;
        }

        // 过滤系统组件（SystemComponent = 1，可能存为 REG_DWORD 或 REG_SZ）
        if is_system_component(subkey) {
            return;
        }
        // 过滤子组件（有父级入口的）
        if let Ok(_parent) = subkey.get_value::<String, _>("ParentDisplayName") {
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

/// 扫描所有已安装的软件（无过滤），用于卸载等需要全量搜索的场景。
///
/// 与 `scan_all_installed` 不同，此函数不过滤 `SystemComponent` 或
/// `ParentDisplayName`，返回注册表中所有含 DisplayName 的条目。
pub fn scan_all_installed_unfiltered() -> Vec<HashMap<String, String>> {
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

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

// ── 内部函数 ───────────────────────────────────────────

fn for_each_uninstall(mut cb: impl FnMut(&str, &RegKey)) {
    let hives: &[(HKEY, &str)] = &[
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    for &(root, path) in hives {
        if let Ok(key) = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ) {
            for name in key.enum_keys().flatten() {
                if let Ok(sk) = key.open_subkey_with_flags(&name, KEY_READ) {
                    cb(&name, &sk);
                }
            }
        }
    }
}

fn try_match<F>(predicate: F, root: RegKey, path: &str) -> Option<HashMap<String, String>>
where
    F: Fn(&str, Option<&str>) -> bool,
{
    let key = root.open_subkey_with_flags(path, KEY_READ).ok()?;
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

/// 收集所有匹配的子键（不重复检查 DisplayName），供 `detect_all_installed_by` 使用。
fn collect_matches(
    dn_lower: &str,
    publisher_lower: &Option<String>,
    check_publisher: bool,
    root: RegKey,
    path: &str,
    results: &mut Vec<HashMap<String, String>>,
) {
    let mut seen = std::collections::HashSet::new();
    let key = match root.open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(_) => return,
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
        if dn.trim().is_empty() || !seen.insert(dn.clone()) {
            continue;
        }
        // 过滤系统组件和子组件（保持与 scan_all_installed 一致的过滤逻辑）
        if is_system_component(&subkey) {
            continue;
        }
        if subkey.get_value::<String, _>("ParentDisplayName").is_ok() {
            continue;
        }
        let dn_lc = dn.to_lowercase();
        if !dn_lc.contains(dn_lower) {
            continue;
        }
        if check_publisher {
            if let Some(pub_lower) = publisher_lower {
                let pub_ = subkey.get_value::<String, _>("Publisher").ok();
                if !pub_.map_or(false, |p| p.to_lowercase().contains(pub_lower.as_str())) {
                    continue;
                }
            }
        }
        let mut result = HashMap::new();
        result.insert("DisplayName".into(), dn);
        if let Ok(p) = subkey.get_value::<String, _>("Publisher") {
            result.insert("Publisher".into(), p);
        }
        for field in ["DisplayVersion", "InstallLocation", "UninstallString"] {
            if let Ok(val) = subkey.get_value::<String, _>(field) {
                result.insert(field.into(), val);
            }
        }
        results.push(result);
    }
}

fn is_system_component(subkey: &RegKey) -> bool {
    if let Ok(val) = subkey.get_value::<u32, _>("SystemComponent") {
        return val == 1;
    }
    if let Ok(val) = subkey.get_value::<String, _>("SystemComponent") {
        if val.trim() == "1" {
            return true;
        }
    }
    false
}
