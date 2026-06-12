use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};

use crate::downloader;
use crate::paths;
use crate::pe_version;
use crate::registry;
use crate::software::{self, SoftwareDef, VersionInfo};

// ── Installer type detection ──────────────────────────────

/// Detect if an executable is an NSIS installer by scanning for "Nullsoft" bytes.
fn is_nsis(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 65536];
    while let Ok(n) = f.read(&mut buf) {
        if n == 0 {
            break;
        }
        if buf[..n].windows(8).any(|w| w == b"Nullsoft") {
            return true;
        }
    }
    false
}

/// Detect if an executable is an Inno Setup installer.
fn is_inno(path: &Path) -> bool {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 65536];
    while let Ok(n) = f.read(&mut buf) {
        if n == 0 {
            break;
        }
        if buf[..n].windows(9).any(|w| w == b"Inno Setup") {
            return true;
        }
    }
    false
}

/// Detect installer type. Returns "nsis", "inno", or empty string if unknown.
pub fn detect_installer_type(path: &Path) -> &'static str {
    if is_nsis(path) {
        "nsis"
    } else if is_inno(path) {
        "inno"
    } else {
        ""
    }
}

// ── Install Flow ──────────────────────────────────────────

pub fn install_software(
    name: &str,
    version: &str,
    gui: bool,
    renew: bool,
    download_only: bool,
) -> anyhow::Result<()> {
    // 1. Read software definition
    let sd = software::read_software_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    // 2. Resolve version
    let ver = if version.is_empty() {
        sd.default_version.as_str()
    } else {
        version
    };
    if ver.is_empty() {
        bail!("{}: 未指定版本号", name);
    }

    let vi = match sd.versions.get(ver) {
        Some(vi) => vi,
        None => {
            // Try prefix matching
            let mut matched: Vec<&String> = sd.versions.keys()
                .filter(|k| k.starts_with(ver))
                .collect();
            // Also try contains matching
            if matched.is_empty() {
                matched = sd.versions.keys()
                    .filter(|k| k.to_lowercase().contains(&ver.to_lowercase()))
                    .collect();
            }
            let mut available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
            sort_versions_desc(&mut available);
            let hint = available.join(", ");
            match matched.len() {
                0 => {
                    bail!("{}: 未找到版本 '{}'\n  可用版本: {}", name, ver, hint);
                }
                1 => sd.versions.get(matched[0]).unwrap(),
                _ => {
                    let candidates = matched.iter()
                        .map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    bail!("{}: 版本 '{}' 匹配到多个: {}\n  可用版本: {}", name, ver, candidates, hint);
                }
            }
        }
    };
    if vi.urls.is_empty() {
        bail!("{}: 未配置下载地址", name);
    }

    // 3. Check already installed (skip if download_only)
    if !download_only {
        if let Some(ref detection) = vi.detection {
            if let Some(result) = registry::detect_installed(detection) {
                let installed_ver = result.get("DisplayVersion").map(|s| s.as_str()).unwrap_or(ver);
                println!("⚠ {} {} 已安装在系统中。", display, installed_ver);
                println!("  如需重新安装，请先执行: as uninstall {}", name);
                return Ok(());
            }
        }
    }

    // 4. Download installer
    let installer_path = get_installer_path(name, ver, &vi.urls, renew)?;
    if download_only {
        println!("✓ {} {} 下载完成", display, ver);
        return Ok(());
    }

    // 5. Install
    println!("\n▶ 安装 {} {} ...", display, ver);
    let installed = run_installer(name, &installer_path, vi, gui)?;
    if !installed {
        bail!("安装未完成（用户取消或安装失败）");
    }

    // 6. Find install location
    let install_path = find_install_path(name, vi, &sd);

    // 7. Create shortcut in apps/
    let shortcut_key = create_app_shortcut(name, vi, &install_path);

    // 8. Record
    let pe_ver = pe_version::get_pe_version(&installer_path);
    let (canonical_version, provenance) = if let Some(ref pv) = pe_ver {
        (pv.clone(), "pe")
    } else {
        (ver.to_string(), "source")
    };
    software::record_installation(
        name,
        &canonical_version,
        install_path.as_deref().unwrap_or_default(),
        provenance,
        ver,
    )?;

    println!("\n✓ {} {} 安装完成", display, canonical_version);
    if provenance == "pe" && canonical_version != ver {
        println!("  \x1b[90m(源声明 v{}, PE 真实 v{})\x1b[0m", ver, canonical_version);
    }
    if let Some(ref sk) = shortcut_key {
        println!("  快捷方式: {}", sk);
    }
    if let Some(ip) = &install_path {
        println!("  安装位置: {}", ip);
    }

    Ok(())
}

// ── Uninstall ─────────────────────────────────────────────

pub fn uninstall_software(name: &str, gui: bool, force: bool) -> anyhow::Result<()> {
    let sd = software::read_software_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    // Find installed version
    let version = sd.default_version.as_str();
    let vi = sd.versions.get(version);

    let installed_info = vi.and_then(|v| v.detection.as_ref())
        .and_then(|d| registry::detect_installed(d));

    if installed_info.is_none() && !force {
        println!("{} 未在系统中检测到安装记录。", display);
        println!("如需清理安装记录，请使用 --force 参数。");
        return Ok(());
    }

    // Run uninstall
    if let Some(ref info) = installed_info {
        let ok = run_uninstall(name, info, gui)?;
        if !ok {
            return Ok(());
        }
    }

    // Clean up shortcuts
    let app_lnk = paths::apps_dir().join(format!("{}.lnk", name));
    if app_lnk.exists() {
        fs::remove_file(&app_lnk)?;
        println!("  已删除快捷方式: {}", app_lnk.display());
    }

    // Remove installation record
    software::remove_installation_record(name)?;
    println!("\n✓ {} 卸载完成", display);

    Ok(())
}

// ── Uninstall implementation ──────────────────────────────

fn run_uninstall(_name: &str, info: &std::collections::HashMap<String, String>, gui: bool) -> anyhow::Result<bool> {
    let uninstall_str = match info.get("UninstallString") {
        Some(s) => s.clone(),
        None => {
            println!("  未找到卸载程序。");
            return Ok(false);
        }
    };

    if gui {
        // For GUI mode, just run the uninstaller as-is
        let args: Vec<&str> = uninstall_str.split_whitespace().collect();
        if args.is_empty() {
            return Ok(false);
        }
        let status = Command::new(args[0])
            .args(&args[1..])
            .status()
            .context("运行卸载程序失败")?;
        return Ok(status.success());
    }

    // Silent: try to add silent flags for common installer types
    let cmd_args: Vec<&str> = uninstall_str.split_whitespace().collect();
    if cmd_args.is_empty() {
        return Ok(false);
    }

    let cmd = cmd_args[0];
    let rest = &cmd_args[1..];

    // Detect NSIS and add /S flag
    let is_nsis = rest.iter().any(|a| a.to_uppercase().contains("UNINST"));
    let status = if is_nsis {
        Command::new(cmd)
            .args(rest)
            .arg("/S")
            .status()
    } else {
        Command::new(cmd)
            .args(rest)
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(true),
        Ok(s) => {
            eprintln!("  卸载程序退出码: {}", s.code().unwrap_or(-1));
            Ok(false)
        }
        Err(e) => {
            eprintln!("  运行卸载程序失败: {}", e);
            Ok(false)
        }
    }
}

// ── Download helpers ──────────────────────────────────────

fn get_installer_path(name: &str, version: &str, urls: &[String], renew: bool) -> anyhow::Result<PathBuf> {
    let dl = paths::downloads_dir();
    fs::create_dir_all(&dl)?;

    let filename = downloader::safe_installer_name(name, version, urls);
    let target = dl.join(&filename);

    if target.exists() && !renew {
        println!("  使用缓存: {}", target.display());
        return Ok(target);
    }

    let tmp = dl.join(format!("{}.downloading", filename));
    downloader::download_with_fallback(name, urls, &tmp, renew)?;
    fs::rename(&tmp, &target)?;

    // 最终验证：rename 后的文件必须通过签名检查
    if !downloader::verify_downloaded_file(&target) {
        let _ = std::fs::remove_file(&target);
        bail!("{}: 下载后验证失败（文件损坏或反盗链页面）", name);
    }

    Ok(target)
}

// ── Installer execution ───────────────────────────────────

fn run_installer(name: &str, installer_path: &Path, vi: &VersionInfo, gui: bool) -> anyhow::Result<bool> {
    let itype = if vi.installer_type.is_empty() {
        detect_installer_type(installer_path)
    } else {
        &vi.installer_type
    };

    // Portable mode: extract archive
    if itype == "portable" {
        return install_portable(name, installer_path);
    }

    // Build command
    let mut cmd = Command::new(installer_path);
    if !gui {
        // Add silent args
        for arg in &vi.install_args {
            cmd.arg(arg);
        }
    }

    if gui {
        println!("  以交互界面模式启动安装器");
    } else {
        println!("  静默安装 {} ...", itype);
    }

    let status = cmd.status();

    match status {
        Ok(s) if s.success() => Ok(true),
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            // Check for UAC required errors
            if code == 1223 || code == 740 {
                println!("  需要管理员权限，尝试提权...");
                return try_elevate(installer_path, &vi.install_args);
            }
            eprintln!("  安装程序返回错误码 {}", code);
            Ok(false)
        }
        Err(e) => {
            eprintln!("  运行安装程序失败: {}", e);
            Ok(false)
        }
    }
}

fn install_portable(name: &str, archive_path: &Path) -> anyhow::Result<bool> {
    let target = paths::builds_dir().join(name);
    if target.exists() {
        bail!("便携版目录已存在: {}", target.display());
    }
    fs::create_dir_all(&target)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    println!("  解压到 {} ...", target.display());

    match ext.to_lowercase().as_str() {
        "zip" => {
            // Use PowerShell for zip extraction
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), target.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            // Try 7zr.exe from builds directory
            let seven_z = paths::builds_dir().join("7zr").join("7zr.exe");
            let status = if seven_z.exists() {
                Command::new(&seven_z)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", target.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到 7zr.exe 解压工具）", ext)
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    Ok(true)
}

fn try_elevate(installer_path: &Path, args: &[String]) -> anyhow::Result<bool> {
    // Use PowerShell Start-Process -Verb RunAs for elevation
    let mut ps_args = format!(
        "Start-Process -FilePath '{}'",
        installer_path.display()
    );
    if !args.is_empty() {
        let arg_str = args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        ps_args.push_str(&format!(" -ArgumentList {}", arg_str));
    }
    ps_args.push_str(" -Verb RunAs -Wait");

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_args])
        .status()?;

    if !status.success() {
        eprintln!("  UAC 提权被取消或失败");
    }
    Ok(status.success())
}

// ── Post-install helpers ──────────────────────────────────

fn find_install_path(_name: &str, vi: &VersionInfo, _sd: &SoftwareDef) -> Option<String> {
    // 1. Try registry detection
    if let Some(ref detection) = vi.detection {
        if let Some(result) = registry::detect_installed(detection) {
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

fn create_app_shortcut(name: &str, vi: &VersionInfo, install_path: &Option<String>) -> Option<String> {
    // Try shortcut_candidates first
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

    // Fallback: shortcut to install directory
    if let Some(ip) = install_path {
        let target = paths::apps_dir().join(format!("{}.lnk", name));
        let _ = create_shortcut_dir(ip, &target);
        return Some(target.to_string_lossy().into());
    }

    None
}

// ── PowerShell-backed Windows utilities ───────────────────

fn ps_exec(command: &str) -> anyhow::Result<String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("PowerShell 错误: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_shortcut_target(lnk_path: &str) -> Option<String> {
    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath",
        lnk_path.replace('\'', "''")
    );
    ps_exec(&cmd).ok()
}

fn create_shortcut_file(source_lnk: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let target = get_shortcut_target(source_lnk).unwrap_or_default();
    let dir = Path::new(&target).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    create_shortcut_dir(&dir, output_lnk)
}

fn create_shortcut_dir(target_dir: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let out_dir = output_lnk.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(out_dir)?;

    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath = '{}'; \
         $sc.Save()",
        output_lnk.to_string_lossy().replace('\'', "''"),
        target_dir.replace('\'', "''"),
    );
    ps_exec(&cmd)?;
    Ok(())
}

fn expand_env_vars(input: &str) -> String {
    // Simple %VAR% expansion
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let mut var = String::new();
            while let Some(&next) = chars.peek() {
                if next == '%' {
                    chars.next();
                    break;
                }
                var.push(next);
                chars.next();
            }
            if let Ok(val) = std::env::var(&var) {
                result.push_str(&val);
            } else {
                result.push('%');
                result.push_str(&var);
                result.push('%');
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Sort version strings in descending order (newest first).
/// Parses dotted numeric versions like "3.14.5" into segments for natural sort.
fn sort_versions_desc(versions: &mut [&str]) {
    versions.sort_by(|a, b| {
        let a_segs: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let b_segs: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        // Descending: compare b against a
        for i in 0..a_segs.len().max(b_segs.len()) {
            let av = a_segs.get(i).copied().unwrap_or(0);
            let bv = b_segs.get(i).copied().unwrap_or(0);
            match bv.cmp(&av) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        // If all numeric segments equal, fall back to lexicographic (descending)
        b.cmp(a)
    });
}
