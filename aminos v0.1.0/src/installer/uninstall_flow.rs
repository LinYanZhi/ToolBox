use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::bail;

use crate::{paths, registry};
use crate::software;

use super::detect::resolve_real_uninstaller;
use super::helpers::{
    scan_dirs_for_uninstaller, parse_cmdline,
};

/// 卸载主入口（供 cmd_uninstall、cmd_install 调用）。
pub fn uninstall_software(name: &str, force: bool) -> anyhow::Result<()> {
    let sd = match software::read_software_def(name) {
        Ok(sd) => Some(sd),
        Err(_) => None,
    };

    // 有源定义：走标准流程
    if let Some(ref sd) = sd {
        let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

        // 自研工具卸载
        if sd.kind == "self" {
            return uninstall_self_tool(name, display);
        }

        let version = sd.single_version().or_else(|| sd.first_version()).unwrap_or("unknown");
        let vi = sd.versions.get(version);
        let is_portable = vi.map(|v| v.installer_type == "portable").unwrap_or(false);
        let installed_info = vi.and_then(|v| v.detection.as_ref())
            .and_then(|d| registry::detect_installed(d));

        if installed_info.is_none() && !force && !is_portable {
            println!("  {} 未在系统中检测到安装记录", display);
            println!("  如需清理安装记录，请使用 --force");
            return Ok(());
        }

        if is_portable {
            // 便携版：直接删除目录
            let dir_name = format!("{}-{}", name, version);
            let portable_dir = paths::apps_dir().join(&dir_name);
            if portable_dir.exists() {
                fs::remove_dir_all(&portable_dir)?;
                println!("  已删除便携版目录: {}", portable_dir.display());
            }
        } else if let Some(ref info) = installed_info {
            let dir_candidates: &[String] = vi.map(|v| v.install_dir_candidates.as_slice()).unwrap_or(&[]);
            if let Some(uninstaller_path) = run_uninstall(name, info, dir_candidates)? {
                if let Some(vi) = vi {
                    let removed = spawn_uninstall_and_poll(&uninstaller_path, vi)?;
                    if !removed {
                        if force {
                            eprintln!("  卸载检测超时，--force 跳过继续清理");
                        } else {
                            eprintln!("  卸载可能未完成");
                            return Ok(());
                        }
                    }
                }
            } else {
                println!("  ! 未找到卸载程序，请手动卸载或使用 --force");
                return Ok(());
            }
        }

        // 清理快捷方式
        let app_lnk = paths::apps_dir().join(format!("{}.lnk", name));
        if app_lnk.exists() {
            fs::remove_file(&app_lnk)?;
            println!("  已删除快捷方式");
        }

        // 删除安装记录
        software::remove_installation_record(name)?;
        println!("  {} 卸载完成", display);
        return Ok(());
    }

    // ── 无源定义：回退到注册表搜索 ──
    eprintln!("  未找到 {} 的源定义，正在搜索注册表...", name);
    let reg_all = registry::scan_all_installed_unfiltered();
    let name_lower = name.to_lowercase();
    let matches: Vec<_> = reg_all.into_iter()
        .filter(|entry| {
            entry.get("display_name")
                .map(|dn| dn.to_lowercase().contains(&name_lower))
                .unwrap_or(false)
        })
        .collect();

    if matches.is_empty() {
        bail!("  未在注册表中找到匹配「{}」的软件", name);
    }

    for info in &matches {
        let dn = info.get("display_name").map(|s| s.as_str()).unwrap_or(name);
        println!("  找到: {}", dn);
        let uninstall_str = info.get("UninstallString").or_else(|| info.get("uninstall_string"));
        match uninstall_str {
            Some(cmd) => {
                let args = parse_cmdline(cmd);
                if !args.is_empty() && Path::new(&args[0]).exists() {
                    println!("  已启动卸载程序");
                    let _ = Command::new(&args[0]).args(&args[1..]).spawn();
                } else {
                    println!("  ! 卸载程序不存在，请手动卸载或使用 --force");
                }
            }
            None => {
                println!("  ! 未找到卸载命令，请手动卸载或使用 --force");
            }
        }
    }

    // 清理 apps/ 目录中的快捷方式
    let app_lnk = paths::apps_dir().join(format!("{}.lnk", name));
    if app_lnk.exists() {
        fs::remove_file(&app_lnk)?;
        println!("  已删除快捷方式: {}", app_lnk.display());
    }

    // 删除安装记录
    if software::remove_installation_record(name).is_ok() {
        println!("  已清理安装记录");
    }

    println!("  {} 卸载完成", name);
    Ok(())
}

/// 卸载自研工具：删除 tools/{name}/ 目录和 tools/bin/{name}.exe 硬链接。
pub(crate) fn uninstall_self_tool(name: &str, display: &str) -> anyhow::Result<()> {
    let tool_dir = paths::tools_dir().join(name);
    if tool_dir.exists() {
        fs::remove_dir_all(&tool_dir)?;
        println!("  已删除: {}", tool_dir.display());
    }

    let link_path = paths::tools_bin_dir().join(format!("{}.exe", name));
    if link_path.exists() {
        fs::remove_file(&link_path)?;
        println!("  已删除硬链接: {}", link_path.display());
    }

    software::remove_installation_record(name)?;
    println!("\nOK {} 卸载完成", display);

    Ok(())
}

/// 卸载自研工具（供 cmd_tool 调用）。
pub fn uninstall_tool(name: &str) -> anyhow::Result<()> {
    let sd = software::read_tool_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };
    uninstall_self_tool(name, display)
}

// ── Uninstall implementation ──────────────────────────────

/// 运行卸载程序（有 UninstallString 或回退到候选目录扫描）。
fn run_uninstall(
    _name: &str,
    info: &std::collections::HashMap<String, String>,
    install_dir_candidates: &[String],
) -> anyhow::Result<Option<PathBuf>> {
    let uninstall_str = match info.get("UninstallString").or_else(|| info.get("uninstall_string")) {
        Some(s) => s.clone(),
        None => return try_fallback_uninstaller(install_dir_candidates),
    };

    let args = parse_cmdline(&uninstall_str);
    if args.is_empty() {
        return try_fallback_uninstaller(install_dir_candidates);
    }

    let install_path = info.get("InstallLocation").or_else(|| info.get("install_path")).map(|s| s.as_str());
    let (real_args, _installer_type) = resolve_real_uninstaller(&args, install_path);

    if real_args.first().map_or(true, |p| !Path::new(p).exists()) {
        return try_fallback_uninstaller(install_dir_candidates);
    }

    Ok(Some(PathBuf::from(&real_args[0])))
}

/// 回退到候选安装目录中查找卸载程序。
fn try_fallback_uninstaller(install_dir_candidates: &[String]) -> anyhow::Result<Option<PathBuf>> {
    if install_dir_candidates.is_empty() {
        return Ok(None);
    }
    Ok(scan_dirs_for_uninstaller(install_dir_candidates))
}

/// 启动卸载程序并轮询检测直到软件被移除（或超时）。
///
/// 与安装流程同理：不等待卸载器进程退出，而是轮询检测
/// 注册表条目/快捷方式/安装目录是否消失。
fn spawn_uninstall_and_poll(
    exe_path: &Path,
    vi: &crate::software::VersionInfo,
) -> anyhow::Result<bool> {
    println!("  已启动卸载程序");
    println!("  等待卸载完成...");

    let _ = Command::new(exe_path).spawn();

    let timeout = Duration::from_secs(60);
    let start = Instant::now();
    let mut printed_dot = false;

    loop {
        // 检测软件是否已被移除
        if super::helpers::check_software_removed(vi) {
            if !printed_dot {
                println!();
            }
            println!("  检测到软件已卸载");
            return Ok(true);
        }

        if start.elapsed() > timeout {
            if !printed_dot {
                println!();
            }
            eprintln!("  等待卸载超时（{} 秒），注册表条目可能仍存在", timeout.as_secs());
            return Ok(false);
        }

        if !printed_dot {
            print!("  ");
            printed_dot = true;
        }
        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::thread::sleep(Duration::from_millis(500));
    }
}
