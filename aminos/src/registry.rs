/// 重新导出 sys::registry，保持 aminos 内部接口不变。
///
/// `aminos` 有自己的 `Detection` 类型（在 `software.rs` 中，含 serde），
/// `sys::registry::Detection` 不含 serde。这里适配两者的转换。
pub use sys::registry::scan_all_installed;

use std::collections::HashMap;

use winreg::enums::*;
use winreg::RegKey;
use winreg::HKEY;
pub fn detect_installed(detection: &crate::software::Detection) -> Option<HashMap<String, String>> {
    let dn = detection.display_name.as_deref()?;
    let publisher = detection.publisher.as_deref();
    // sys::registry::detect_installed 的原有逻辑在此重放，
    // 避免引入额外的类型转换耦合。
    detect_installed_impl(dn, publisher)
}

/// 内联的检测实现，复用 sys::registry 的核心逻辑但接受 &str 参数。
fn detect_installed_impl(display_name: &str, publisher: Option<&str>) -> Option<HashMap<String, String>> {
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
        for &(root, path) in SYS_REG_PATHS {
            if let Some(r) = sys_try_match(&pred, root, path) {
                return Some(r);
            }
        }
    }
    None
}

const SYS_REG_PATHS: &[(HKEY, &str)] = &[
    (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
];

fn sys_try_match<F>(predicate: F, root: HKEY, path: &str) -> Option<HashMap<String, String>>
where
    F: Fn(&str, Option<&str>) -> bool,
{
    let key = RegKey::predef(root).open_subkey_with_flags(path, KEY_READ).ok()?;
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
