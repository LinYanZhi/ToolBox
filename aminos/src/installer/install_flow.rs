use std::fs;
use std::path::Path;

use anyhow::{bail, Context};

use crate::{opts, paths, pe_version, registry, cmd_names};
use crate::software::{self, SoftwareDef, VersionInfo};
use color;

use super::download::{get_installer_path, file_sha256};
use super::executor::{run_installer, extract_zip_to};
use super::helpers::{label_of_type, find_install_path, create_app_shortcut, find_entry_point_exe};
use super::windows::{
    sort_versions_desc, is_tools_bin_in_user_path, is_tools_bin_in_session_path,
    detect_powershell,
};

/// 安装第三方软件（使用预读取的 SoftwareDef，供 cmd_install 直接调用）
pub fn install_software_by_def(
    name: &str,
    sd: &software::SoftwareDef,
    opts: &opts::InstallOpts,
) -> anyhow::Result<()> {
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    // 2. Resolve version
    let ver = resolve_version(display, sd, opts)?;

    let vi = match sd.versions.get(&ver) {
        Some(vi) => vi,
        None => {
            let matched: Vec<&String> = sd.versions.keys()
                .filter(|k| k.starts_with(ver.as_str()))
                .collect();
            let mut available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
            sort_versions_desc(&mut available);
            let hint = available.join(", ");
            match matched.len() {
                0 => bail!("{}: 未找到版本 '{}'\n  可用版本: {}", name, ver, hint),
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

    // 自研工具
    if sd.kind == "self" {
        return install_self_tool(name, sd, display, &ver, vi, opts);
    }

    // 3. Check already installed
    if !opts.download_only {
        if let Some(ref detection) = vi.detection {
            if let Some(result) = registry::detect_installed(detection) {
                let installed_ver = result.get("DisplayVersion").map(|s| s.as_str()).unwrap_or(&ver);
                println!("{} {} 已安装在系统中", display, installed_ver);
                println!("  如需重新安装，请先执行: {} {}", cmd_names::UNINSTALL, name);
                return Ok(());
            }
        }
    }

    // 4. Download
    let installer_path = get_installer_path(name, &ver, &vi.urls, opts.renew, &sd.downloader)?;
    if opts.download_only {
        println!("{} {} 下载完成", display, ver);
        return Ok(());
    }

    // 5. Install
    println!("\n安装 {} {}...", display, ver);
    let (installed, portable_install_path) = run_installer(name, &ver, &installer_path, vi, opts.gui)?;
    if !installed {
        bail!("安装未完成（用户取消或安装失败）");
    }

    // 6. Find install location
    let install_path = if let Some(path) = portable_install_path {
        Some(path.to_string_lossy().to_string())
    } else {
        find_install_path(name, vi, sd)
    };

    // 7. Create shortcut
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
        &ver,
        &vi.installer_type,
        "",
    )?;

    println!("\n{} {} 安装完成", display, canonical_version);
    if provenance == "pe" && canonical_version != ver {
        println!("  {}", color::gray(format!("(源声明 v{}, PE 真实 v{})", ver, canonical_version)));
    }
    if let Some(ref sk) = shortcut_key {
        println!("  快捷方式: {}", sk);
    }
    if let Some(ip) = &install_path {
        println!("  安装位置: {}", ip);
    }

    Ok(())
}

/// 解析版本号：自动选择安装类型或交互式选择。
fn resolve_version(
    display: &str,
    sd: &SoftwareDef,
    opts: &opts::InstallOpts,
) -> anyhow::Result<String> {
    if let Some(v) = &opts.version {
        return Ok(v.clone());
    }

    // 只有单版本 → 自动选择
    if let Some(v) = sd.single_version() {
        return Ok(v.to_string());
    }

    // 多版本：按安装类型分组，让用户选择
    let mut by_type: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for (vk, vi) in &sd.versions {
        let itype = if vi.installer_type.is_empty() { "installer" } else { &vi.installer_type };
        by_type.entry(itype).or_default().push(vk.as_str());
    }

    if by_type.len() >= 2 {
        if let Some(ref preferred) = opts.inst_type {
            let matched = by_type.iter().find(|(itype, _)| **itype == preferred.as_str());
            return match matched {
                Some((_, vers)) => Ok(vers[0].to_string()),
                None => {
                    let avail: Vec<&str> = by_type.keys().map(|t| *t).collect();
                    bail!("{}: 安装类型 '{}' 不可用（可用: {}）", display, preferred, avail.join(", "));
                }
            };
        }

        println!("{} 有多种安装方式，请选择：", display);
        let mut options: Vec<(&str, Vec<&str>)> = by_type.into_iter().collect();
        options.sort_by(|a, b| a.0.cmp(b.0));

        for (i, (itype, vers)) in options.iter().enumerate() {
            let label = match *itype {
                "portable" => "便携版（免安装，解压即用）",
                "nsis" | "inno" | "exe" | "installer" => "安装版（写入注册表，需管理员）",
                other => other,
            };
            let ver_str = if vers.len() == 1 {
                vers[0].to_string()
            } else {
                format!("{}（共 {} 个版本）", vers[0], vers.len())
            };
            println!("  {}. {}  — {}  [{}]", i + 1, label, color::cyan(&ver_str), itype);
        }
        println!("  输入 1-{} 选择，或按 Enter 使用默认：", options.len());

        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
        let choice = input.trim().parse::<usize>().ok()
            .and_then(|n| n.checked_sub(1))
            .filter(|&i| i < options.len());

        return match choice {
            Some(idx) => {
                let (_, vers) = &options[idx];
                println!("  已选择: {} {}", color::bold_green(label_of_type(options[idx].0)), color::cyan(vers[0]));
                Ok(vers[0].to_string())
            }
            None => {
                let available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
                bail!("请指定版本: 可用版本 {}", available.join(", "))
            }
        };
    }

    // 单类型但有多个版本
    if by_type.len() == 1 {
        let (_, vers) = by_type.into_iter().next().unwrap();
        let available: Vec<&str> = vers.clone();
        bail!("{}: 有多个版本，请用 --version 指定\n  可用版本: {}", display, available.join(", "))
    }

    // 没有版本（不可能发生）
    let available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
    bail!("{}: 无可用的版本定义\n  可用版本: {}", display, available.join(", "))
}

/// 安装自研工具：下载 ZIP → 解压到 tools/{name}/ → 硬链接到 tools/bin/{name}.exe
pub(crate) fn install_self_tool(
    name: &str,
    sd: &SoftwareDef,
    display: &str,
    ver: &str,
    vi: &VersionInfo,
    opts: &opts::InstallOpts,
) -> anyhow::Result<()> {
    let installer_path = get_installer_path(name, ver, &vi.urls, opts.renew, &sd.downloader)?;
    if opts.download_only {
        println!("{} {} 下载完成", display, ver);
        return Ok(());
    }

    println!("\n安装 {} {}...", display, ver);

    fs::create_dir_all(paths::tools_dir())?;
    fs::create_dir_all(paths::tools_bin_dir())?;

    let tool_dir = paths::tools_dir().join(name);
    if tool_dir.exists() {
        fs::remove_dir_all(&tool_dir)?;
    }
    extract_zip_to(&installer_path, &tool_dir)?;
    println!("  已解压到: {}", tool_dir.display());

    let entry_exe = find_entry_point_exe(tool_dir.to_string_lossy().as_ref(), vi, name);
    let exe_path = entry_exe.unwrap_or_else(|| {
        tool_dir.join(format!("{}.exe", name)).to_string_lossy().to_string()
    });
    let exe_path = Path::new(&exe_path);

    if !exe_path.is_file() {
        bail!("在 {} 中未找到可执行文件", tool_dir.display());
    }

    let link_path = paths::tools_bin_dir().join(format!("{}.exe", name));
    let _ = fs::remove_file(&link_path);
    fs::hard_link(exe_path, &link_path)
        .with_context(|| format!("创建硬链接失败: {} → {}", link_path.display(), exe_path.display()))?;
    println!("  硬链接: {} → {}", link_path.display(), exe_path.display());

    let sha256 = file_sha256(&installer_path);
    software::record_installation(
        name, ver, tool_dir.to_string_lossy().as_ref(), "source", ver, &vi.installer_type, &sha256,
    )?;

    println!("\n{} {} 安装完成", display, ver);
    println!("  位置: {}", tool_dir.display());

    let bin_dir = paths::tools_bin_dir();
    let registered = is_tools_bin_in_user_path();
    let in_session = is_tools_bin_in_session_path(&bin_dir);

    if registered && in_session {
        println!("  {} 可直接在终端使用", color::cyan(&format!("{}", name)));
    } else if registered && !in_session {
        println!("  {} 可直接在终端使用", color::cyan(&format!("{}", name)));
        println!("  如需在当前终端立即生效，请执行:");
        let bin_path = bin_dir.to_string_lossy();
        if detect_powershell() {
            println!("    {}", color::bold_green(&format!("$env:PATH = \"{};$env:PATH\"", bin_path)));
        } else {
            println!("    {}", color::bold_green(&format!("set PATH={};%PATH%", bin_path)));
        }
    } else {
        println!("  {} 请先运行 {} 将 tools/bin 加入 PATH", name, color::cyan(cmd_names::TOOL_INIT));
    }

    Ok(())
}

/// 安装自研工具：从 source/tools/ 读取定义，下载 ZIP 并硬链接到 tools/bin/
pub fn install_tool(name: &str, opts: &opts::InstallOpts) -> anyhow::Result<()> {
    let sd = software::read_tool_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    let ver = opts.version.clone().unwrap_or_else(|| {
        sd.single_version()
            .or_else(|| sd.first_version())
            .map(|v| v.to_string())
            .unwrap_or_else(|| {
                let available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
                panic!("{}: 有多个版本，请用 --version 指定\n  可用版本: {}", name, available.join(", "))
            })
    });

    let vi = match sd.versions.get(&ver) {
        Some(vi) => vi,
        None => {
            let available: Vec<&str> = sd.versions.keys().map(|v| v.as_str()).collect();
            bail!("{}: 未找到版本 '{}'\n  可用版本: {}", name, ver, available.join(", "));
        }
    };

    if vi.urls.is_empty() {
        bail!("{}: 未配置下载地址", name);
    }

    install_self_tool(name, &sd, display, &ver, vi, opts)
}
