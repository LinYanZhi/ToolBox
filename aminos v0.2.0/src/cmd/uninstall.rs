use std::io::Write;
use color::*;
use crate::software;

/// as uninstall — 卸载软件
///
/// 支持两种来源：
///   1. as 管理的便携版（通过 installed.json 记录）
///   2. 注册表检测到的安装版（有 UninstallString）
///
/// 当检测到多个版本时，询问用户选择卸载哪一个。
pub fn run(name: String) -> anyhow::Result<()> {
    let installed = software::read_installed()?;

    // 通过别名/模糊匹配找到软件（获取 entry 用于注册表检测）
    let (matched_name, entry) = match software::resolve_software(&name, "卸载") {
        Ok(r) => r,
        Err(e) => {
            // 无法解析但仍可能在 installed 中有精确匹配
            if let Some(rec) = installed.get(&name) {
                return uninstall_as_record(&name, rec, None);
            }
            eprintln!("  {} {}", yellow("提示:"), e);
            return Ok(());
        }
    };

    // 收集所有可卸载的版本候选
    struct Candidate {
        version: String,
        source: &'static str,       // "便携版" / "安装版"
        is_as_record: bool,
        registry_info: Option<software::RegistryInfo>, // 注册表信息
    }

    let mut candidates: Vec<Candidate> = Vec::new();

    // 1) as 管理的版本
    if let Some(rec) = installed.get(&matched_name) {
        let type_label = if rec.r#type == "portable" { "便携版" } else { "安装版" };
        candidates.push(Candidate {
            version: rec.version.clone(),
            source: type_label,
            is_as_record: true,
            registry_info: None,
        });
    }

    // 2) 注册表检测到的所有版本（去重）
    let all_reg = software::detect_all_from_registry(&entry);
    for info in &all_reg {
        // 跳过与 as 管理版本为同一 registry_version 的
        if candidates.iter().any(|c| {
            c.is_as_record && entry.versions.iter().any(|(sk, ve)| {
                sk == &c.version && ve.registry_version.as_deref() == Some(&info.version)
            })
        }) {
            continue;
        }
        candidates.push(Candidate {
            version: info.version.clone(),
            source: "安装版（注册表）",
            is_as_record: false,
            registry_info: Some(info.clone()),
        });
    }

    if candidates.is_empty() {
        eprintln!("  {} {} 未安装（as 未管理，注册表也未检测到）", yellow("跳过"), matched_name);
        return Ok(());
    }

    // 排序固定：按版本号降序（避免 HashMap 遍历顺序不一致）
    candidates.sort_by(|a, b| b.version.cmp(&a.version));

    if candidates.len() == 1 {
        let c = &candidates[0];
        if c.is_as_record {
            if let Some(ref rec) = installed.get(&matched_name) {
                let uninst = c.registry_info.as_ref().and_then(|i| i.uninstall_string.as_deref());
                uninstall_as_record(&matched_name, rec, uninst)?;
            }
        } else if let Some(ref info) = c.registry_info {
            uninstall_registry(&matched_name, info)?;
        }
        return Ok(());
    }

    // 多版本 → 询问用户
    println!("  检测到 {} 的多个版本：", bold_cyan(&matched_name));
    for (i, c) in candidates.iter().enumerate() {
        println!("    {}. {}（{}）", gray(&format!("{}", i + 1)), c.version, c.source);
    }
    println!("    {}. 全部卸载", gray(&format!("{}", candidates.len() + 1)));
    let choices: Vec<String> = (1..=candidates.len() + 1).map(|n| n.to_string()).collect();
    let choices_ref: Vec<&str> = choices.iter().map(|s| s.as_str()).collect();
    let choice = ask_choice("请选择要卸载的版本", &choices_ref)?;
    let idx: usize = choice.parse().unwrap_or(0);

    if idx == candidates.len() + 1 {
        // 全部卸载
        for c in &candidates {
            if c.is_as_record {
                if let Some(ref rec) = installed.get(&matched_name) {
                    let uninst = c.registry_info.as_ref().and_then(|i| i.uninstall_string.as_deref());
                    uninstall_as_record(&matched_name, rec, uninst)?;
                }
            } else if let Some(ref info) = c.registry_info {
                uninstall_registry(&matched_name, info)?;
            }
        }
    } else if idx >= 1 && idx <= candidates.len() {
        let c = &candidates[idx - 1];
        if c.is_as_record {
            if let Some(ref rec) = installed.get(&matched_name) {
                let uninst = c.registry_info.as_ref().and_then(|i| i.uninstall_string.as_deref());
                uninstall_as_record(&matched_name, rec, uninst)?;
            }
        } else if let Some(ref info) = c.registry_info {
            uninstall_registry(&matched_name, info)?;
        }
    }

    Ok(())
}

// ── 核心卸载操作 ──────────────────────────────────────────

/// 卸载 as 管理的记录
fn uninstall_as_record(name: &str, rec: &software::InstallRecord, uninstall_string: Option<&str>) -> anyhow::Result<()> {
    println!("  {} {} {} ({})", bold_cyan("卸载"), name, rec.version, rec.r#type);
    let ok = match rec.r#type.as_str() {
        "portable" => {
            uninstall_portable(name, rec)?;
            true
        }
        "installer" => uninstall_via_registry(name, uninstall_string)?,
        t => anyhow::bail!("未知安装类型: {}", t),
    };
    if ok {
        software::remove_install_record(name)?;
        println!("  {} 卸载程序已退出，请确认是否已完成卸载", cyan("➜"));
    } else {
        eprintln!("  {} 卸载程序返回了错误，可能未成功卸载", yellow("警告"));
    }
    Ok(())
}

/// 卸载注册表检测到的安装版
fn uninstall_registry(name: &str, info: &software::RegistryInfo) -> anyhow::Result<()> {
    println!("  {} {} {}（注册表检测）", bold_cyan("卸载"), name, info.version);
    if let Some(ref uninst) = info.uninstall_string {
        println!("    卸载程序: {}", cyan(uninst));
        let ok = run_uninstall_string(uninst)?;
        if ok {
            // 验证：检查注册表中该版本是否真的被移除
            if sys::registry::detect_installed_by(&info.display_name, info.publisher.as_deref())
                .and_then(|m| m.get("DisplayVersion").cloned())
                .map_or(false, |v| v == info.version)
            {
                eprintln!("  {} 注册表中仍能检测到 {} {}，卸载可能未成功", yellow("注意"), name, info.version);
                eprintln!("    请打开控制面板 → 程序和功能 确认是否已卸载");
            } else {
                println!("  {} 卸载程序已退出（注册表中已检测不到该版本）", cyan("➜"));
            }
        } else {
            eprintln!("  {} 卸载程序返回了错误，可能未成功卸载", yellow("警告"));
            println!("    请手动打开控制面板 → 程序和功能 → 卸载");
        }
    } else {
        println!("  {} 已检测到 {}，但没有卸载命令", yellow("提示"), name);
        println!("    请手动卸载（控制面板 → 程序和功能）");
    }
    Ok(())
}

// ── 底层操作 ──────────────────────────────────────────────

fn uninstall_portable(name: &str, rec: &software::InstallRecord) -> anyhow::Result<()> {
    let path = std::path::Path::new(&rec.install_path);
    // 安全校验：拒绝删除根目录和系统关键路径
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canonical_str = canonical.to_string_lossy().to_lowercase();
    let exact_dangerous = [
        "c:\\", "c:\\windows", "c:\\windows\\system32",
        "c:\\program files", "c:\\program files (x86)",
        "\\$recycle.bin",
    ];
    for pattern in &exact_dangerous {
        if canonical_str == *pattern || canonical_str.starts_with(&format!("{}\\", pattern)) {
            anyhow::bail!(
                "安全拦截：路径 '{}' 是系统目录，拒绝删除",
                canonical.display()
            );
        }
    }

    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
        println!("  已删除目录: {}", gray(&path.display().to_string()));
    }

    // 删除快捷桩
    let shim_path = crate::paths::bin_dir().join(format!("{}.bat", name));
    if shim_path.is_file() {
        std::fs::remove_file(&shim_path)?;
        println!("  已删除快捷桩: {}", gray(&shim_path.display().to_string()));
    }

    Ok(())
}

/// 执行 UninstallString 中的卸载命令。
///
/// 如果传入了 `uninstall_string`，直接使用该字符串；否则通过注册表按名称查找。
/// 执行后验证注册表中是否已移除，返回 true 表示确认卸载成功。
fn uninstall_via_registry(name: &str, uninstall_string: Option<&str>) -> anyhow::Result<bool> {
    let uninst = match uninstall_string {
        Some(s) => s.to_string(),
        None => {
            sys::registry::detect_installed_by(name, None)
                .and_then(|m| m.get("UninstallString").cloned())
                .ok_or_else(|| anyhow::anyhow!("注册表中未找到卸载命令"))?
        }
    };
    println!("    卸载程序: {}", cyan(&uninst));
    let ok = run_uninstall_string(&uninst)?;
    Ok(ok)
}

/// 执行 UninstallString 中的卸载命令，处理提权错误。
///
/// 返回 true 表示卸载程序退出码为 0（成功），false 表示退出码非零。
/// 调用方应自行通过注册表验证确认。
///
/// 注册表中的 UninstallString 格式为：
///   `"C:\Program Files\App\unins000.exe"`         // 仅路径（带空格时需要引号包围）
///   `"C:\Program Files\App\unins000.exe" /SILENT` // 路径 + 参数
///   `MsiExec.exe /I{...GUID}`                    // MSI 安装
///   或 `C:\NoSpaces\unins.exe`                    // 无空格时无引号
fn run_uninstall_string(uninst: &str) -> anyhow::Result<bool> {
    let (exe, args) = parse_uninstall_string(uninst);

    let mut cmd = std::process::Command::new(exe);
    for arg in args.split_whitespace() {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }
    let result = cmd.status();

    match result {
        Ok(status) => Ok(status.success()),
        Err(e) if e.raw_os_error() == Some(740) => {
            elevation_fallback(exe, args)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("找不到卸载程序 '{}'，请检查该软件是否正确安装", exe)
        }
        Err(e) => Err(e.into()),
    }
}

/// 遇到提权错误 740 时的处理：提示 + 可选以管理员身份重试
fn elevation_fallback(exe: &str, args: &str) -> anyhow::Result<bool> {
    eprintln!("  {} 需要管理员权限才能卸载", yellow("注意"));
    print!("  是否以管理员身份重试？[y/N]: ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();

    if input.trim().eq_ignore_ascii_case("y") {
        let powershell_args = if args.is_empty() {
            format!(
                "Start-Process -Wait -Verb RunAs -FilePath '{}'",
                exe
            )
        } else {
            format!(
                "Start-Process -Wait -Verb RunAs -FilePath '{}' -ArgumentList '{}'",
                exe,
                args.replace('\'', "''")
            )
        };
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &powershell_args])
            .status()?;
        Ok(status.success())
    } else {
        eprintln!("  已取消卸载，请手动以管理员身份运行卸载程序：");
        eprintln!("    {}", cyan(exe));
        Ok(false)
    }
}

/// 解析 UninstallString，返回 (exe_path, args)。
///
/// 只处理两种情况：
///   - 引号括起来的：`"C:\path\unins.exe" /S` → ("C:\path\unins.exe", "/S")
///   - 包含 .exe 的：`C:\path\unins.exe /S` 或 `MsiExec.exe /I{GUID}` → ("C:\path\unins.exe", "/S")
///
/// 不包含 .exe 且无引号时不做猜测，直接视为整个字符串就是 exe 路径。
/// 调用方会在 run_uninstall_string 中遇到 "文件不存在" 错误而报错。
fn parse_uninstall_string(s: &str) -> (&str, &str) {
    let s = s.trim();
    // 1) 引号括起来的 → 取引号内
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let exe = &rest[..end];
            let args = rest[end + 1..].trim();
            return (exe, args);
        }
    }
    // 2) 用 .exe 定位路径结尾（从右向左找最后一个 .exe）
    let lower = s.to_lowercase();
    if let Some(exe_end) = lower.rfind(".exe") {
        let exe = &s[..exe_end + 4];
        let args = s[exe_end + 4..].trim();
        return (exe, args);
    }
    // 3) 既无引号也无 .exe → 不做猜测，整个字符串当 exe 路径
    (s, "")
}

/// 打印选项列表并读取用户选择，返回选中项。无效输入直接报错。
fn ask_choice(prompt: &str, options: &[&str]) -> anyhow::Result<String> {
    let opts = options.join("/");
    print!("  {}（{}）: ", prompt, opts);
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() {
        anyhow::bail!("已取消卸载");
    }
    for opt in options {
        if trimmed == *opt {
            return Ok(trimmed);
        }
    }
    anyhow::bail!("无效选择，请输入（{}）", opts)
}
