use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::bail;

use crate::{paths, registry};
use crate::software;

use super::detect::resolve_real_uninstaller;
use super::helpers::{
    prompt_uninstall_done, scan_dirs_for_uninstaller, parse_cmdline,
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

        let version = sd.default_version.as_str();
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
            // 便携版：先询问确认
            if !prompt_uninstall_done(display) {
                println!("  已取消");
                return Ok(());
            }
            let dir_name = format!("{}-{}", name, version);
            let portable_dir = paths::apps_dir().join(&dir_name);
            if portable_dir.exists() {
                fs::remove_dir_all(&portable_dir)?;
                println!("  已删除便携版目录");
            }
        } else if let Some(ref info) = installed_info {
            let dir_candidates: &[String] = vi.map(|v| v.install_dir_candidates.as_slice()).unwrap_or(&[]);
            let ok = run_uninstall(name, info, dir_candidates)?;
            if !ok {
                return Ok(());
            }
            // 卸载程序已启动，询问用户确认
            if !prompt_uninstall_done(display) {
                println!("  已取消");
                return Ok(());
            }
        }

        // 清理快捷方式
        let app_lnk = paths::apps_dir().join(format!("{}.lnk", name));
        if app_lnk.exists() {
            fs::remove_file(&app_lnk)?;
            println!("  已删除快捷方式");
        }

        // 注册表二次确认（安装版且有 detection 配置时）
        if !is_portable {
            if let Some(ref vi) = vi {
                if let Some(ref detection) = vi.detection {
                    let still_installed = registry::detect_installed(detection);
                    if still_installed.is_some() {
                        if force {
                            eprintln!("  ! 注册表条目仍存在（--force，继续清理记录）");
                        } else {
                            eprintln!("  ! 卸载可能未完成（注册表条目仍存在）");
                            eprintln!("  如需强制清理，请使用 --force");
                            return Ok(());
                        }
                    } else {
                        println!("  OK 注册表确认已卸载");
                    }
                }
            }
        }

        // 删除安装记录
        software::remove_installation_record(name)?;
        println!("  OK {} 卸载完成", display);
        return Ok(());
    }

    // ── 无源定义：回退到注册表搜索 ──
    eprintln!("  [i] 未找到「{}」的源定义，正在搜索注册表...", name);
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
        bail!("  ! 未在注册表中找到匹配「{}」的软件", name);
    }

    for info in &matches {
        let dn = info.get("display_name").map(|s| s.as_str()).unwrap_or(name);
        println!("  找到: {}", dn);
        let ok = run_uninstall(name, info, &[])?;
        if !ok {
            println!("  跳过: {}", dn);
        }
    }

    // 无源卸载：尝试清理 apps/ 目录中的快捷方式
    let app_lnk = paths::apps_dir().join(format!("{}.lnk", name));
    if app_lnk.exists() {
        if prompt_uninstall_done(name) {
            fs::remove_file(&app_lnk)?;
            println!("  已删除快捷方式: {}", app_lnk.display());
        } else {
            println!("  已取消");
            return Ok(());
        }
    }

    // 卸载后二次确认：重新扫描注册表
    let still_there: Vec<_> = registry::scan_all_installed_unfiltered().into_iter()
        .filter(|entry| {
            entry.get("display_name")
                .map(|dn| dn.to_lowercase().contains(&name_lower))
                .unwrap_or(false)
        })
        .collect();

    if still_there.is_empty() {
        println!("  OK 注册表确认已卸载");
    } else if !force {
        eprintln!("  ! 以下软件在注册表中仍存在条目，卸载可能未完成:");
        for entry in &still_there {
            let dn = entry.get("display_name").map(|s| s.as_str()).unwrap_or("?");
            println!("     - {}", dn);
        }
        eprintln!("  如需强制清理，请使用 --force");
        return Ok(());
    } else {
        eprintln!("  ! 注册表条目仍存在（--force，继续）");
    }

    if software::remove_installation_record(name).is_ok() {
        println!("  已清理安装记录");
    }

    println!("  OK {} 卸载完成", name);
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
) -> anyhow::Result<bool> {
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

    gui_mode_uninstall(&real_args)
}

/// GUI 模式卸载：启动卸载程序，由注册表二次确认兜底。
fn gui_mode_uninstall(cmd_args: &[String]) -> anyhow::Result<bool> {
    println!("  已启动卸载程序");
    match Command::new(&cmd_args[0]).args(&cmd_args[1..]).spawn() {
        Ok(_) => {
            std::thread::sleep(std::time::Duration::from_secs(3));
            Ok(true)
        }
        Err(e) if e.raw_os_error() == Some(740) => {
            println!("  需要管理员权限，正在提权...");
            let exe = cmd_args[0].replace('\'', "''");
            let args = if cmd_args.len() > 1 {
                format!(" -ArgumentList '{}'", cmd_args[1..].join(" ").replace('\'', "''"))
            } else {
                String::new()
            };
            let ps = format!(
                "Start-Process -FilePath '{}'{} -Verb RunAs -ErrorAction SilentlyContinue -WindowStyle Normal",
                exe, args,
            );
            let _ = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps])
                .status();
            Ok(true)
        }
        Err(e) => {
            eprintln!("  启动卸载程序失败: {}", e);
            Ok(false)
        }
    }
}

/// 当注册表 UninstallString 不可用时，回退到候选安装目录中查找并运行卸载程序。
fn try_fallback_uninstaller(install_dir_candidates: &[String]) -> anyhow::Result<bool> {
    if install_dir_candidates.is_empty() {
        println!("  ! 未找到卸载程序，请手动卸载");
        return Ok(false);
    }
    println!("  [i] 注册表卸载程序不可用，尝试在候选安装目录中查找...");
    let found = match scan_dirs_for_uninstaller(install_dir_candidates) {
        Some(f) => f,
        None => {
            println!("  ! 未找到卸载程序，请手动卸载");
            return Ok(false);
        }
    };
    println!("  找到卸载程序: {}", found.display());
    let found_str = found.to_string_lossy().to_string();
    gui_mode_uninstall(&[found_str])
}
