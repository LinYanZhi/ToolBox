use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};

use crate::{opts, paths, pe_version, registry, cmd_names};
use crate::software::{self, SoftwareDef, VersionInfo};
use color;

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

/// 返回安装类型的人类可读标签。
fn label_of_type(itype: &str) -> String {
    match itype {
        "portable" => "便携版".to_string(),
        "nsis" | "inno" | "exe" | "installer" => "安装版".to_string(),
        other => other.to_string(),
    }
}

pub fn install_software(name: &str, opts: &opts::InstallOpts) -> anyhow::Result<()> {
    let sd = software::read_software_def(name)?;
    install_software_by_def(name, &sd, opts)
}

/// 安装第三方软件（使用预读取的 SoftwareDef，供 cmd_install 直接调用）
pub fn install_software_by_def(name: &str, sd: &software::SoftwareDef, opts: &opts::InstallOpts) -> anyhow::Result<()> {
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    // 2. Resolve version — 如果未指定，检查是否有多种安装类型可供选择
    let ver = if opts.version.is_empty() {
        // 按安装类型分组版本
        let mut by_type: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
        for (vk, vi) in &sd.versions {
            let itype = if vi.installer_type.is_empty() { "installer" } else { &vi.installer_type };
            by_type.entry(itype).or_default().push(vk.as_str());
        }

        if by_type.len() >= 2 {
            // 检查用户是否通过 --type 指定了安装类型
            if let Some(ref preferred) = opts.inst_type {
                let matched = by_type.iter().find(|(itype, _)| **itype == preferred.as_str());
                match matched {
                    Some((_, vers)) => vers[0].to_string(),
                    None => {
                        let avail: Vec<&str> = by_type.keys().map(|t| *t).collect();
                        bail!("{}: 安装类型 '{}' 不可用（可用: {}）",
                            display, preferred, avail.join(", "));
                    }
                }
            } else {
                // 未指定 → 让用户交互选择
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

                match choice {
                    Some(idx) => {
                        // 选中了某个类型，取该类型的第一个版本
                        let (_, vers) = &options[idx];
                        let chosen_ver = vers[0];
                        println!("  已选择: {} {}", color::bold_green(label_of_type(options[idx].0)), color::cyan(chosen_ver));
                        chosen_ver.to_string()
                    }
                    None => sd.default_version.clone(), // 回车或无效输入 → 默认
                }
            }
        } else {
            // 只有一种安装类型
            sd.default_version.clone()
        }
    } else {
        opts.version.clone()
    };

    let vi = match sd.versions.get(&ver) {
        Some(vi) => vi,
        None => {
            // 精确 prefix 匹配
            let matched: Vec<&String> = sd.versions.keys()
                .filter(|k| k.starts_with(ver.as_str()))
                .collect();
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

    // ── 自研工具（kind="self"）专用安装路径 ──
    if sd.kind == "self" {
        return install_self_tool(name, &sd, display, &ver, vi, opts);
    }

    // 3. Check already installed (skip if download_only)
    if !opts.download_only {
        if let Some(ref detection) = vi.detection {
            if let Some(result) = registry::detect_installed(detection) {
                let installed_ver = result.get("DisplayVersion").map(|s| s.as_str()).unwrap_or(&ver);
                println!("⚠ {} {} 已安装在系统中。", display, installed_ver);
                println!("  如需重新安装，请先执行: as uninstall {}", name);
                return Ok(());
            }
        }
    }

    // 4. Download installer
    let installer_path = get_installer_path(name, &ver, &vi.urls, opts.renew)?;
    if opts.download_only {
        eprintln!("✓ {} {} 下载完成", display, ver);
        return Ok(());
    }

    // 5. Install
    eprintln!("\n▶ 安装 {} {} ...", display, ver);
    let (installed, portable_install_path) = run_installer(name, &ver, &installer_path, vi, opts.gui)?;
    if !installed {
        bail!("安装未完成（用户取消或安装失败）");
    }

    // 6. Find install location — 便携版已有返回路径，标准安装查注册表
    let install_path = if let Some(path) = portable_install_path {
        Some(path.to_string_lossy().to_string())
    } else {
        find_install_path(name, vi, &sd)
    };

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
        &ver,
        &vi.installer_type,
    )?;

    println!("\n✓ {} {} 安装完成", display, canonical_version);
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

// ── 自研工具（kind="self"）安装 ─────────────────────────

/// 安装自研工具：下载 ZIP → 解压到 tools/{name}/ → 硬链接到 tools/bin/{name}.exe
fn install_self_tool(
    name: &str,
    _sd: &SoftwareDef,
    display: &str,
    ver: &str,
    vi: &VersionInfo,
    opts: &opts::InstallOpts,
) -> anyhow::Result<()> {
    // 1. Download
    let installer_path = get_installer_path(name, ver, &vi.urls, opts.renew)?;
    if opts.download_only {
        eprintln!("✓ {} {} 下载完成", display, ver);
        return Ok(());
    }

    println!("\n▶ 安装 {} {} ...", display, ver);

    // 2. 确保 tools/bin 目录存在
    fs::create_dir_all(paths::tools_dir())?;
    fs::create_dir_all(paths::tools_bin_dir())?;

    // 3. 解压到 tools/{name}/
    let tool_dir = paths::tools_dir().join(name);
    if tool_dir.exists() {
        fs::remove_dir_all(&tool_dir)?;
    }
    extract_zip_to(&installer_path, &tool_dir)?;
    println!("  已解压到: {}", tool_dir.display());

    // 4. 找到入口 exe
    let entry_exe = find_entry_point_exe(
        tool_dir.to_string_lossy().as_ref(),
        vi,
        name,
    );
    let exe_path = entry_exe.unwrap_or_else(|| {
        // 回退：直接使用工具名
        tool_dir.join(format!("{}.exe", name)).to_string_lossy().to_string()
    });
    let exe_path = Path::new(&exe_path);

    if !exe_path.is_file() {
        bail!("在 {} 中未找到可执行文件", tool_dir.display());
    }

    // 5. 在 tools/bin/ 创建硬链接（覆盖旧链接）
    let link_path = paths::tools_bin_dir().join(format!("{}.exe", name));
    let _ = fs::remove_file(&link_path);
    fs::hard_link(exe_path, &link_path)
        .with_context(|| format!("创建硬链接失败: {} → {}", link_path.display(), exe_path.display()))?;
    println!("  硬链接: {} → {}", link_path.display(), exe_path.display());

    // 6. 记录安装
    software::record_installation(name, ver, tool_dir.to_string_lossy().as_ref(), "source", ver, &vi.installer_type)?;

    println!("\n✓ {} {} 安装完成", display, ver);
    println!("  位置: {}", tool_dir.display());

    // 检查 tools/bin 是否已加入用户 PATH（注册表）
    let bin_dir = paths::tools_bin_dir();
    let registered = is_tools_bin_in_user_path();
    let in_session = is_tools_bin_in_session_path(&bin_dir);

    if registered && in_session {
        println!("  {} 可直接在终端使用", color::cyan(&format!("{}", name)));
    } else if registered && !in_session {
        // 注册表已有，但当前终端未刷新
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

// ── 公开接口：自研工具安装/卸载（供 cm d_tool.rs 调用）────

/// 安装自研工具：从 source/tools/ 读取定义，下载 ZIP 并硬链接到 tools/bin/
pub fn install_tool(name: &str, opts: &opts::InstallOpts) -> anyhow::Result<()> {
    let sd = software::read_tool_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    let ver = if opts.version.is_empty() {
        sd.default_version.clone()
    } else {
        opts.version.clone()
    };

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

/// 卸载自研工具：删除 tools/{name}/ 目录和 tools/bin/{name}.exe 硬链接
pub fn uninstall_tool(name: &str) -> anyhow::Result<()> {
    let sd = software::read_tool_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };
    uninstall_self_tool(name, display)
}

// ── Uninstall ─────────────────────────────────────────────

pub fn uninstall_software(name: &str, gui: bool, force: bool) -> anyhow::Result<()> {
    let sd = software::read_software_def(name)?;
    let display = if sd.display_name.is_empty() { name } else { &sd.display_name };

    // Find installed version
    let version = sd.default_version.as_str();
    let vi = sd.versions.get(version);

    // ── 自研工具卸载 ──
    if sd.kind == "self" {
        return uninstall_self_tool(name, display);
    }

    let is_portable = vi.map(|v| v.installer_type == "portable").unwrap_or(false);
    let installed_info = vi.and_then(|v| v.detection.as_ref())
        .and_then(|d| registry::detect_installed(d));

    if installed_info.is_none() && !force && !is_portable {
        println!("{} 未在系统中检测到安装记录。", display);
        println!("如需清理安装记录，请使用 --force 参数。");
        return Ok(());
    }

    if is_portable {
        // 便携版：直接删除 apps/{name}-{version}/ 目录
        let dir_name = format!("{}-{}", name, version);
        let portable_dir = paths::apps_dir().join(&dir_name);
        if portable_dir.exists() {
            fs::remove_dir_all(&portable_dir)?;
            println!("  已删除便携版目录: {}", portable_dir.display());
        }
    } else {
        // 标准安装：运行卸载程序
        if let Some(ref info) = installed_info {
            let ok = run_uninstall(name, info, gui)?;
            if !ok {
                return Ok(());
            }
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

/// 卸载自研工具：删除 tools/{name}/ 目录和 tools/bin/{name}.exe 硬链接。
fn uninstall_self_tool(name: &str, display: &str) -> anyhow::Result<()> {
    // 删除工具目录
    let tool_dir = paths::tools_dir().join(name);
    if tool_dir.exists() {
        fs::remove_dir_all(&tool_dir)?;
        println!("  已删除: {}", tool_dir.display());
    }

    // 删除硬链接
    let link_path = paths::tools_bin_dir().join(format!("{}.exe", name));
    if link_path.exists() {
        fs::remove_file(&link_path)?;
        println!("  已删除硬链接: {}", link_path.display());
    }

    // 删除安装记录
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

    // 正确解析命令行：尊重引号保护的带空格路径
    let args = parse_cmdline(&uninstall_str);
    if args.is_empty() {
        return Ok(false);
    }

    // 组装完整命令参数
    let is_nsis = args[1..].iter().any(|a| a.to_uppercase().contains("UNINST"));
    let cmd_args: Vec<String> = if gui {
        args
    } else if is_nsis {
        let mut a = args;
        a.push("/S".to_string());
        a
    } else {
        args
    };

    // 先尝试直接运行
    match try_run_uninstall(&cmd_args) {
        Ok(result) => return Ok(result),
        Err(e) if e.raw_os_error() == Some(740) => {
            println!("  卸载程序需要管理员权限，正在提权运行...");
        }
        Err(e) => {
            eprintln!("  运行卸载程序失败: {}", e);
            return Ok(false);
        }
    }

    // ERROR_ELEVATION_REQUIRED → 通过 Start-Process -Verb RunAs 提权
    let ps_script = if cmd_args.len() <= 1 {
        format!(
            "Start-Process -FilePath '{}' -Verb RunAs -Wait -ErrorAction SilentlyContinue",
            cmd_args[0].replace('\'', "''"),
        )
    } else {
        format!(
            "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs -Wait -ErrorAction SilentlyContinue",
            cmd_args[0].replace('\'', "''"),
            cmd_args[1..].join(" ").replace('\'', "''"),
        )
    };
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .status();

    match status {
        Ok(_) => {
            // Start-Process -Verb RunAs 成功与否只能通过 $? 判断，
            // 但即使 UAC 被取消也返回 0，所以这里假定提权后用户手动操作完成
            Ok(true)
        }
        Err(e) => {
            eprintln!("  提权运行卸载程序失败: {}", e);
            Ok(false)
        }
    }
}

/// 尝试运行卸载程序，不处理提权（让调用者处理）。
fn try_run_uninstall(args: &[String]) -> std::io::Result<bool> {
    let status = Command::new(&args[0]).args(&args[1..]).status()?;
    Ok(status.success())
}

/// 解析 Windows 命令行字符串，正确处理引号保护的参数。
///
/// 例如 `"C:\Program Files\7-Zip\Uninstall.exe" /S` → ["C:\Program Files\7-Zip\Uninstall.exe", "/S"]
fn parse_cmdline(cmd: &str) -> Vec<String> {
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

// ── Download helpers ──────────────────────────────────────

/// 扫描目录，删除所有以 `.parts` 结尾的残留目录（RustRange 分片下载中断遗留）。
fn cleanup_stale_parts(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.ends_with(".parts"))
            {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
}

fn get_installer_path(name: &str, version: &str, urls: &[String], renew: bool) -> anyhow::Result<PathBuf> {
    let dl = paths::downloads_dir();
    fs::create_dir_all(&dl)?;

    // 清理遗留的 .parts 临时目录（上次中断的 RustRange 分片下载残留）
    cleanup_stale_parts(&dl);

    // 1) 从 URL 探测真实文件名（HEAD 请求，或 URL 路径推测）
    //    成功则扩展名已正确，无需魔数修正
    let (filename, needs_magic_fix) = match urls.first().and_then(|u| net::probe_filename(u)) {
        Some(fname) => (fname, false),
        None => (safe_installer_name(name, version, urls), true),
    };
    let target = dl.join(&filename);

    // 2) 精确匹配缓存
    if target.exists() && !renew {
        println!("  使用缓存: {}", target.display());
        return Ok(target);
    }

    // 前缀匹配：扫描 {name}-{version}.xxxxx（应对魔数修正或 probe 返回不同文件名）
    if !renew {
        let base = format!("{}-{}.",
            name.to_lowercase().replace(' ', "-"),
            version.to_lowercase().replace(' ', "-"));
        if let Ok(entries) = std::fs::read_dir(&dl) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with(&base) && fname != filename {
                    let p = entry.path();
                    if p.is_file() {
                        println!("  使用缓存: {}", p.display());
                        return Ok(p);
                    }
                }
            }
        }
    }

    // 3) 下载到临时文件
    let tmp = dl.join(format!("{}.downloading", filename));
    net::download::download_with_url_fallback(name, urls, &tmp, &net::DownloadConfig::default().renew(renew))?;

    // 4) 魔数修正扩展名（仅当 URL 探测失败、用猜测文件名时）
    let corrected = if needs_magic_fix {
        match net::detect_format(&tmp) {
            Some(fmt) => {
                let ext = fmt.extension();
                if !filename.ends_with(ext) {
                    let corrected = format!("{}-{}{}",
                        name.to_lowercase().replace(' ', "-"),
                        version.to_lowercase().replace(' ', "-"), ext);
                    let p = dl.join(&corrected);
                    fs::rename(&tmp, &p)?;
                    p
                } else {
                    let p = dl.join(&filename);
                    fs::rename(&tmp, &p)?;
                    p
                }
            }
            None => {
                let p = dl.join(&filename);
                fs::rename(&tmp, &p)?;
                p
            }
        }
    } else {
        let p = dl.join(&filename);
        fs::rename(&tmp, &p)?;
        p
    };

    // 5) 最终验证
    if !net::verify_downloaded_file(&corrected) {
        let _ = std::fs::remove_file(&corrected);
        bail!("{}: 下载后验证失败（文件损坏或反盗链页面）", name);
    }

    Ok(corrected)
}

/// 构造安全的安装包文件名。
fn safe_installer_name(name: &str, version: &str, urls: &[String]) -> String {
    let safe_name = name.to_lowercase().replace(' ', "-");
    let safe_ver = version.to_lowercase().replace(' ', "-");

    if let Some(first_url) = urls.first() {
        let path = first_url.split('?').next().unwrap_or(first_url);
        let seg = path.rsplit('/').next().unwrap_or("");
        if let Some(dot) = seg.rfind('.') {
            let e = &seg[dot..];
            if [
                ".exe", ".msi", ".zip", ".7z", ".rar", ".tar", ".gz", ".xz", ".bz2", ".iso",
                ".appx", ".dmg",
            ]
            .contains(&e.to_lowercase().as_str())
            {
                return format!("{}-{}{}", safe_name, safe_ver, e);
            }
        }
    }
    format!("{}-{}.exe", safe_name, safe_ver)
}

// ── Installer execution ───────────────────────────────────

fn run_installer(name: &str, version: &str, installer_path: &Path, vi: &VersionInfo, gui: bool) -> anyhow::Result<(bool, Option<PathBuf>)> {
    let itype = if vi.installer_type.is_empty() {
        detect_installer_type(installer_path)
    } else {
        &vi.installer_type
    };

    // Portable mode: extract archive
    if itype == "portable" {
        let path = install_portable(name, version, installer_path)?;
        return Ok((true, Some(path)));
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
        Ok(s) if s.success() => Ok((true, None)),
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            // Check for UAC required errors
            if code == 1223 || code == 740 {
                println!("  需要管理员权限，尝试提权...");
                return try_elevate(installer_path, &vi.install_args);
            }
            eprintln!("  安装程序返回错误码 {}", code);
            Ok((false, None))
        }
        Err(e) => {
            eprintln!("  运行安装程序失败: {}", e);
            Ok((false, None))
        }
    }
}

/// 将压缩包解压到指定目录（自研工具用）。
/// 如果压缩包内只有一个根目录，则提取该目录的内容；否则直接解压到目标目录。
pub fn extract_zip_to(archive_path: &Path, target_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(target_dir)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let staging = target_dir.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    match ext.as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）", ext);
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    // 检查 staging 是否只有一个根目录
    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        // 单根目录 → 把内容移出来
        let inner = entries[0].path();
        for entry in fs::read_dir(&inner)? {
            let entry = entry?;
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    } else {
        // 平铺文件 → 直接移入
        for entry in entries {
            let target = target_dir.join(entry.file_name());
            let _ = fs::remove_dir_all(&target);
            fs::rename(entry.path(), &target)?;
        }
    }

    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

/// 安装便携版（第三方）。
fn install_portable(name: &str, version: &str, archive_path: &Path) -> anyhow::Result<PathBuf> {
    let dir_name = format!("{}-{}", name, version);
    let target = paths::apps_dir().join(&dir_name);
    if target.exists() {
        bail!("便携版目录已存在: {}", target.display());
    }

    // 使用 staging 目录解压，检查目录结构后再整理
    let staging = target.with_extension("staging");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    let ext = archive_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    println!("  解压中 ...");

    match ext.to_lowercase().as_str() {
        "zip" => {
            let status = Command::new("powershell")
                .args([
                    "-NoProfile", "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                        archive_path.display(), staging.display()
                    ),
                ])
                .status()?;
            if !status.success() {
                bail!("解压 zip 失败");
            }
        }
        _ => {
            // 尝试多种可能的 7z 解压工具路径
            let candidates = [
                paths::builds_dir().join("7zr").join("7zr.exe"),
                paths::builds_dir().join("7zr.exe"),
                PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
                PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
            ];
            let seven_z = candidates.iter().find(|p| p.exists());
            let status = if let Some(exe) = seven_z {
                Command::new(exe)
                    .args(["x", &archive_path.to_string_lossy(), &format!("-o{}", staging.display()), "-y"])
                    .status()?
            } else {
                bail!("不支持的压缩格式 '{}'（未找到解压工具）。\n  提示：请安装 7-Zip 或将 7zr.exe 放入 {}",
                    ext, paths::builds_dir().display())
            };
            if !status.success() {
                bail!("解压失败");
            }
        }
    }

    // 检查 staging 目录的内容
    let entries: Vec<_> = fs::read_dir(&staging)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            // 跳过系统隐藏文件（如 Thumbs.db, .DS_Store, __MACOSX 等）
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && !name.starts_with("__MACOSX")
        })
        .collect();

    if entries.is_empty() {
        fs::remove_dir(&staging)?;
        bail!("压缩包为空或仅包含系统文件");
    }

    // 如果解压后只有一个根目录，直接用它
    // 否则创建目标目录，把文件移进去
    if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        let single_dir = entries[0].path();
        fs::rename(&single_dir, &target)?;
    } else {
        fs::create_dir(&target)?;
        for entry in &entries {
            let src = entry.path();
            let dest = target.join(entry.file_name());
            fs::rename(&src, &dest)?;
        }
    }

    // 清理 staging
    let _ = fs::remove_dir(&staging);

    println!("  ✓ 已解压到 {}", target.display());
    Ok(target)
}

fn try_elevate(installer_path: &Path, args: &[String]) -> anyhow::Result<(bool, Option<PathBuf>)> {
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
    Ok((status.success(), None))
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
///
/// 优先级：
/// 1. vi.entry_point 指定的文件名
/// 2. 目录中的第一个 .exe
fn find_entry_point_exe(install_dir: &str, vi: &VersionInfo, fallback_name: &str) -> Option<String> {
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

/// 创建指向可执行文件的快捷方式（含工作目录）。
fn create_shortcut_exe(target_exe: &str, output_lnk: &Path) -> anyhow::Result<()> {
    let out_dir = output_lnk.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(out_dir)?;

    let work_dir = Path::new(target_exe).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let cmd = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $sc = $ws.CreateShortcut('{}'); \
         $sc.TargetPath = '{}'; \
         $sc.WorkingDirectory = '{}'; \
         $sc.Save()",
        output_lnk.to_string_lossy().replace('\'', "''"),
        target_exe.replace('\'', "''"),
        work_dir.replace('\'', "''"),
    );
    ps_exec(&cmd)?;
    Ok(())
}

// ── PowerShell-backed Windows utilities ───────────────────

fn ps_exec(command: &str) -> anyhow::Result<String> {
    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", command])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("启动 PowerShell 失败")?;

    // 30 秒超时，防止 PowerShell 挂死
    const PS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let now = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap_or_else(|_| {
                    std::process::Output {
                        status,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    }
                });
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    bail!("PowerShell 错误: {}", stderr.trim());
                }
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
            Ok(None) => {
                if now.elapsed() > PS_TIMEOUT {
                    let _ = child.kill();
                    bail!("PowerShell 执行超时（{} 秒）", PS_TIMEOUT.as_secs());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                bail!("PowerShell 进程错误: {}", e);
            }
        }
    }
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
///
/// 正确处理 pre-release 标签（如 "1.0.0-beta", "1.0.0-rc1"）：
///   1. 比较点分数字部分（如 1.0.0）
///   2. 数字部分相同时，正式版 > 预发布版（无 `-` 后缀 > 有 `-` 后缀）
///   3. 均为预发布版时，按标签字符串降序（rc2 > rc1, beta > alpha）
fn sort_versions_desc(versions: &mut [&str]) {
    versions.sort_by(|a, b| {
        // 拆分为基础版本和 pre-release 后缀
        let a_pre = a.splitn(2, '-').collect::<Vec<_>>();
        let b_pre = b.splitn(2, '-').collect::<Vec<_>>();
        let a_base = a_pre[0];
        let b_base = b_pre[0];

        let a_segs: Vec<u32> = a_base.split('.').filter_map(|s| s.parse().ok()).collect();
        let b_segs: Vec<u32> = b_base.split('.').filter_map(|s| s.parse().ok()).collect();

        for i in 0..a_segs.len().max(b_segs.len()) {
            let av = a_segs.get(i).copied().unwrap_or(0);
            let bv = b_segs.get(i).copied().unwrap_or(0);
            match bv.cmp(&av) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }

        // 数字部分相同 → 比较 pre-release 后缀
        // 正式版（无后缀）> 预发布版（有后缀）
        match (a_pre.get(1), b_pre.get(1)) {
            (None, None) => b.cmp(a),            // 都无后缀：回退到字符串比较
            (None, Some(_)) => std::cmp::Ordering::Greater, // a 是正式版，b 是预发布
            (Some(_), None) => std::cmp::Ordering::Less,    // a 是预发布，b 是正式版
            (Some(pa), Some(pb)) => pb.cmp(pa),  // 都是预发布：后缀字符串降序
        }
    });
}

/// 检查 tools/bin 是否已注册到用户 PATH（读取注册表，不受当前会话影响）
fn is_tools_bin_in_user_path() -> bool {
    let bin_dir = paths::tools_bin_dir();
    let bin_path = bin_dir.to_string_lossy().to_string();

    let ps_cmd = format!("[Environment]::GetEnvironmentVariable('PATH', 'User')");
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output();
    match output {
        Ok(o) => {
            let user_path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            user_path.split(';')
                .any(|p| p.trim().to_lowercase() == bin_path.to_lowercase())
        }
        Err(_) => false,
    }
}

/// 检查 tools/bin 是否在当前终端会话的 PATH 中
fn is_tools_bin_in_session_path(bin_dir: &Path) -> bool {
    std::env::var("PATH").ok().map_or(false, |p| {
        let bin = bin_dir.to_string_lossy().to_lowercase();
        p.split(';').any(|s| s.trim().to_lowercase() == bin)
    })
}

/// 检测当前终端是否为 PowerShell（而非 cmd.exe）
fn detect_powershell() -> bool {
    if let Some(ppid) = get_parent_process_name() {
        let lower = ppid.to_lowercase();
        lower.contains("powershell") || lower.contains("pwsh")
    } else {
        false
    }
}

/// 获取父进程名称
fn get_parent_process_name() -> Option<String> {
    #[cfg(windows)]
    {
        let pid = std::process::id();
        let output = std::process::Command::new("wmic")
            .args(["process", "where", &format!("ProcessId={}", pid), "get", "ParentProcessId"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parent_pid = stdout
            .lines()
            .nth(1)
            .and_then(|l| l.trim().parse::<u32>().ok())?;

        let output = std::process::Command::new("wmic")
            .args(["process", "where", &format!("ProcessId={}", parent_pid), "get", "Name"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let name = stdout.lines().nth(1).map(|l| l.trim().to_string())?;
        Some(name)
    }
    #[cfg(not(windows))]
    {
        None
    }
}
