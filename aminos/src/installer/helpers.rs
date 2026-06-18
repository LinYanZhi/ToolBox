use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

use super::windows::{expand_env_vars, get_shortcut_target, create_shortcut_file, create_shortcut_exe, create_shortcut_dir};

/// 返回安装类型的人类可读标签。
pub(crate) fn label_of_type(itype: &str) -> String {
    match itype {
        "portable" => "便携版".to_string(),
        "nsis" | "inno" | "exe" | "installer" => "安装版".to_string(),
        other => other.to_string(),
    }
}

/// 查找安装路径（3 级回退：注册表 → 候选目录 → 快捷方式）。
pub(crate) fn find_install_path(
    _name: &str,
    vi: &crate::software::VersionInfo,
    _sd: &crate::software::SoftwareDef,
) -> Option<String> {
    // 1. Try registry detection
    if let Some(ref detection) = vi.detection {
        if let Some(result) = crate::registry::detect_installed(detection) {
            if let Some(loc) = result.get("InstallLocation") {
                if !loc.is_empty() {
                    return Some(loc.clone());
                }
            }
        }
    }

    // 2. Try install_dir_candidates
    for candidate in &vi.install_dir_candidates {
        let expanded = expand_env_vars(candidate);
        if Path::new(&expanded).is_dir() {
            return Some(expanded);
        }
    }

    // 3. Try shortcut target as fallback
    for lnk in &vi.shortcut_candidates {
        let expanded = expand_env_vars(lnk);
        if Path::new(&expanded).exists() {
            if let Some(target) = get_shortcut_target(&expanded) {
                let dir = Path::new(&target).parent()?;
                if dir.is_dir() {
                    return Some(dir.to_string_lossy().into());
                }
            }
        }
    }

    None
}

/// 创建快捷方式到 apps/{name}.lnk。
pub(crate) fn create_app_shortcut(
    name: &str,
    vi: &crate::software::VersionInfo,
    install_path: &Option<String>,
) -> Option<String> {
    // Try shortcut_candidates first（桌面已有的快捷方式）
    for lnk in &vi.shortcut_candidates {
        let expanded = expand_env_vars(lnk);
        if Path::new(&expanded).exists() {
            if get_shortcut_target(&expanded).is_some() {
                let target = paths::apps_dir().join(format!("{}.lnk", name));
                let _ = create_shortcut_file(&expanded, &target);
                return Some(target.to_string_lossy().into());
            }
        }
    }

    // 便携版：创建指向入口 exe 的快捷方式
    if vi.installer_type == "portable" {
        if let Some(ip) = install_path {
            let exe_path = find_entry_point_exe(ip, vi, name);
            if let Some(exe) = exe_path {
                let target = paths::apps_dir().join(format!("{}.lnk", name));
                let _ = create_shortcut_exe(&exe, &target);
                return Some(target.to_string_lossy().into());
            }
        }
    }

    // Fallback: shortcut to install directory
    if let Some(ip) = install_path {
        let target = paths::apps_dir().join(format!("{}.lnk", name));
        let _ = create_shortcut_dir(ip, &target);
        return Some(target.to_string_lossy().into());
    }

    None
}

/// 在便携版安装目录中找到入口可执行文件。
pub(crate) fn find_entry_point_exe(install_dir: &str, vi: &crate::software::VersionInfo, fallback_name: &str) -> Option<String> {
    let dir = Path::new(install_dir);

    // 1. entry_point 精确匹配
    if let Some(ref ep) = vi.entry_point {
        let p = dir.join(ep);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }

    // 2. 扫描 .exe
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "exe") && p.is_file() {
                candidates.push(p);
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // 3. 优先匹配软件名
    let lower_name = fallback_name.to_lowercase();
    for p in &candidates {
        let fname = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if fname == lower_name || fname.contains(&lower_name) {
            return Some(p.to_string_lossy().to_string());
        }
    }

    // 4. 回退：第一个 exe
    Some(candidates[0].to_string_lossy().to_string())
}

/// 询问用户是否已完成卸载。
pub(crate) fn prompt_uninstall_done(display: &str) -> bool {
    use std::io::Write;
    print!("  是否已完成 {} 的卸载? [y/N] ", display);
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim().to_lowercase();
    trimmed == "y" || trimmed == "yes"
}

/// 在单个目录中查找已知的卸载程序。
pub(crate) fn scan_dir_for_uninstaller(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let candidates = ["unins000.exe", "unins001.exe", "uninstall.exe", "Uninstall.exe", "UnInstall.exe", "uninst.exe"];
    for name in &candidates {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // 通配 unins???.exe
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if fname.starts_with("unins") && fname.ends_with(".exe") && path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// 扫描多个候选目录，查找卸载程序。
pub(crate) fn scan_dirs_for_uninstaller(dirs: &[String]) -> Option<PathBuf> {
    for dir_str in dirs {
        let dir = expand_env_vars(dir_str);
        let path = Path::new(&dir);
        if let Some(found) = scan_dir_for_uninstaller(path) {
            return Some(found);
        }
    }
    None
}

/// 解析 Windows 命令行字符串，正确处理引号保护的参数。
pub(crate) fn parse_cmdline(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for c in cmd.chars() {
        match c {
            '"' => in_quote = !in_quote,
            ' ' if !in_quote => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}
