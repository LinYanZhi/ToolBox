use std::io::Write;
use crate::software;
use crate::installer;
use std::path::Path;

/// as install — 安装软件
pub fn run(
    names: Vec<String>,
    recursive: Option<String>,
    portable: bool,
    installer_force: bool,
) -> anyhow::Result<()> {
    // 批量安装
    if let Some(file) = recursive {
        return run_batch(&file);
    }

    if names.is_empty() {
        eprintln!("  {} 请指定要安装的软件名（如 as install ls pp ss）", color::yellow("提示:"));
        return Ok(());
    }

    for name in &names {
        if name.starts_with("http://") || name.starts_with("https://") {
            if let Err(e) = install_url(name, portable, installer_force) {
                eprintln!("  {} {}: {}", color::red("错误"), name, e);
            }
        } else {
            if let Err(e) = install_named(name, portable, installer_force) {
                eprintln!("  {} {}: {}", color::red("错误"), name, e);
            }
        }
    }

    Ok(())
}

fn install_named(input: &str, prefer_portable: bool, prefer_installer: bool) -> anyhow::Result<()> {
    // 解析 name[=version]
    let (name, requested_version) = if let Some(eq_pos) = input.find('=') {
        (input[..eq_pos].to_string(), Some(input[eq_pos + 1..].to_string()))
    } else {
        (input.to_string(), None)
    };

    // 查找软件（找不到时交互式询问）
    let (matched_name, entry) = match software::resolve_software(&name, "安装") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  {} {}", color::yellow("提示:"), e);
            eprintln!("  也可直接使用 URL 安装: as install <下载链接>");
            return Ok(());
        }
    };

    let display_name = &matched_name;

    // 检查所有已安装版本（as 记录 + 注册表）
    let installed = software::read_installed()?;
    let all_reg = software::detect_all_from_registry(&entry);

    // 收集所有已安装的 source key
    let mut installed_source_keys: Vec<String> = Vec::new();

    // as 管理的版本
    if let Some(rec) = installed.get(display_name) {
        installed_source_keys.push(rec.version.clone());
    }

    // 注册表检测的所有版本（通过 registry_version 映射回 source key）
    for reg_info in &all_reg {
        let sk = entry.versions.iter().find_map(|(sk, ve)| {
            ve.registry_version.as_deref().and_then(|rv| {
                if rv == reg_info.version { Some(sk.clone()) } else { None }
            })
        }).unwrap_or_else(|| reg_info.version.clone());
        if !installed_source_keys.contains(&sk) {
            installed_source_keys.push(sk);
        }
    }

    if !installed_source_keys.is_empty() {
        // 取最新版本
        let latest_ver = entry.versions.keys()
            .max_by(|a, b| cmp_versions(a, b))
            .map(|s| s.as_str());

        let has_latest = latest_ver.map_or(false, |lv| installed_source_keys.iter().any(|k| k == lv));

        println!("  检测到 {} 已安装以下版本：", color::cyan(display_name));
        for sk in &installed_source_keys {
            println!("    - {}", sk);
        }

        if has_latest {
            println!("  当前已是最新版本");
            print!("  是否安装其他版本？[y/N] ");
        } else {
            print!("  是否安装/更新？[Y/n] ");
        }
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let input = input.trim().to_lowercase();

        if has_latest {
            if input != "y" && input != "yes" {
                println!("  已取消");
                return Ok(());
            }
        } else {
            if input == "n" || input == "no" {
                println!("  已取消");
                return Ok(());
            }
        }
        // 继续到安装流程
    }

    // 选择版本和安装类型
    let (version_key, version_entry) = select_version(&entry, &requested_version)?;
    let install_type = select_install_type(&version_entry, display_name, prefer_portable, prefer_installer)?;

    // 获取下载 URL
    let urls = version_entry.urls.get(install_type)
        .ok_or_else(|| anyhow::anyhow!("版本 '{}' 没有 {} 类型的下载地址", version_key, install_type))?;

    println!("\n{} {} {} ({})", color::bold_cyan("安装"), display_name, version_key, install_type);

    // 显示下载链接
    for u in urls {
        println!("  {}", color::gray(u));
    }

    // 下载（传入所有 URL，自动尝试镜像回退）
    let dl_path = match download_file(urls, display_name, version_key) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} 下载失败: {}", color::red("错误"), e);
            return Ok(());
        }
    };

    // 执行安装
    let install_result = match install_type {
        "portable" => installer::install_portable(display_name, version_key, &dl_path)?,
        "installer" => installer::install_installer(display_name, version_key, &dl_path, entry.detect.as_ref())?,
        _ => anyhow::bail!("未知安装类型: {}", install_type),
    };

    software::record_install(display_name, version_key, install_type, &install_result)?;
    println!("  {} 安装程序已退出，请确认已完成安装", color::cyan("➜"));

    // 便携版 → 检查 shim 路径是否在 PATH 中
    if install_type == "portable" {
        installer::check_bin_path_warning();
    }

    Ok(())
}

fn select_version<'a>(
    entry: &'a software::SoftwareEntry,
    requested: &'a Option<String>,
) -> anyhow::Result<(&'a String, &'a software::VersionEntry)> {
    if let Some(ver) = requested {
        if let Some(ve) = entry.versions.get(ver) {
            return Ok((ver, ve));
        }
        anyhow::bail!("版本 '{}' 不存在", ver);
    }

    // 未指定版本 → 如果只有 1 个版本直接选，否则交互式询问
    let mut versions: Vec<(&String, &software::VersionEntry)> = entry.versions.iter().collect();
    versions.sort_by(|(a, _), (b, _)| cmp_versions(a, b).reverse()); // 从新到旧显示

    if versions.len() == 1 {
        return Ok((versions[0].0, versions[0].1));
    }

    println!("  该软件有多个可用版本：");
    for (i, (ver, ve)) in versions.iter().enumerate() {
        let types: Vec<&str> = ve.urls.keys().map(|s| s.as_str()).collect();
        let type_str = if types.len() >= 2 { types.join("+") } else { types[0].to_string() };
        println!("    {}. {} ({})", i + 1, color::cyan(ver), color::gray(&type_str));
    }
    print!("  请选择版本（1-{}，Enter=取消）: ", versions.len());
    std::io::stdout().flush().ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("已取消安装");
    }
    match input.parse::<usize>() {
        Ok(n) if n >= 1 && n <= versions.len() => {
            Ok((versions[n - 1].0, versions[n - 1].1))
        }
        _ => anyhow::bail!("无效选择，已取消安装"),
    }
}

/// 版本号比较：按数字段逐段比较（从大到小，即 3.14.5 > 3.10.11）
fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let a_nums: Vec<u32> = a.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();
    let b_nums: Vec<u32> = b.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();
    for i in 0..a_nums.len().max(b_nums.len()) {
        let av = a_nums.get(i).copied().unwrap_or(0);
        let bv = b_nums.get(i).copied().unwrap_or(0);
        match av.cmp(&bv) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

fn select_install_type(
    version_entry: &software::VersionEntry,
    name: &str,
    prefer_portable: bool,
    prefer_installer: bool,
) -> anyhow::Result<&'static str> {
    let has_installer = version_entry.urls.contains_key("installer");
    let has_portable = version_entry.urls.contains_key("portable");

    match (has_installer, has_portable, prefer_portable, prefer_installer) {
        (true, false, _, _) => Ok("installer"),
        (false, true, _, _) => Ok("portable"),
        (true, true, true, _) => Ok("portable"),
        (true, true, _, true) => Ok("installer"),
        (true, true, false, false) => {
            // 都有 → 交互式询问
            print!("  {} 选择安装类型 (1: 安装版, 2: 便携版): ", color::cyan(name));
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            match input.trim() {
                "1" | "安装版" | "installer" => Ok("installer"),
                "2" | "便携版" | "portable" => Ok("portable"),
                _ => anyhow::bail!("无效选择，请输入 1（安装版）或 2（便携版）"),
            }
        }
        (false, false, _, _) => anyhow::bail!("该版本没有配置下载地址"),
    }
}

fn download_file(urls: &[String], _name: &str, _version: &str) -> anyhow::Result<std::path::PathBuf> {
    let dl_dir = crate::paths::downloads_dir();
    std::fs::create_dir_all(&dl_dir)?;

    // 从 URL 提取文件名（去掉查询参数 ?...）
    let url = urls.first().ok_or_else(|| anyhow::anyhow!("下载地址为空"))?;
    let raw_filename = url.rsplit('/').next().unwrap_or("download");
    let filename = raw_filename.split('?').next().unwrap_or(raw_filename);
    let target = dl_dir.join(filename);

    if target.is_file() {
        println!("  使用缓存: {}", target.display());
        return Ok(target);
    }

    println!("  下载中 ...");
    let config = net::DownloadConfig::default();
    net::download_with_fallback(urls, &target, &config)?;
    println!("  已下载: {}", target.display());

    // 检查文件扩展名，缺失时通过魔数检测补充
    let ext = target.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext.is_empty() {
        let real_type = installer::detect_archive_type(&target);
        let new_ext = match real_type {
            "zip" => "zip",
            "7z" => "7z",
            "single" => "exe",
            _ => "exe", // 默认 exe
        };
        let new_target = target.with_extension(new_ext);
        std::fs::rename(&target, &new_target)?;
        println!("  重命名: {} → {}", filename, Path::new(&new_target).file_name().unwrap_or_default().to_string_lossy());
        return Ok(new_target);
    }

    Ok(target)
}

/// 批量安装
fn run_batch(file: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(file)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        println!("\n{}", color::gray(format!("─── {} ───", line)));
        if let Err(e) = install_named(line, false, false) {
            eprintln!("  {} {}: {}", color::red("错误"), line, e);
        }
    }
    Ok(())
}

/// URL 安装（交互式）
fn install_url(url: &str, portable: bool, installer_force: bool) -> anyhow::Result<()> {
    let raw_filename = url.rsplit('/').next().unwrap_or("download");
    let filename = raw_filename.split('?').next().unwrap_or(raw_filename);

    // 检测文件扩展名
    let ext = Path::new(filename).extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 媒体/文档文件 → 仅下载
    if matches!(ext.as_str(), "mp4" | "mp3" | "pdf" | "jpg" | "png" | "gif" | "doc" | "docx") {
        println!("  检测到 {} 文件，不是安装软件。", ext);
        print!("  是否仅下载到缓存目录？[Y/n] ");
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let input = input.trim().to_lowercase();
        if input == "n" || input == "no" {
            println!("  已取消");
            return Ok(());
        }
        let dl_dir = crate::paths::downloads_dir();
        std::fs::create_dir_all(&dl_dir)?;
        let target = dl_dir.join(filename);
        net::download_with_fallback(&vec![url.to_string()], &target, &net::DownloadConfig::default())?;
        println!("  已下载到: {}", target.display());
        return Ok(());
    }

    // 尝试推测软件名
    let guessed_name = filename.rsplit('.').nth(1).unwrap_or("app")
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or("app")
        .to_string();

    println!("\n{}", color::bold_cyan("检测到自定义软件"));
    println!("  文件名: {}", color::cyan(filename));
    println!("  推测名称: {}", color::cyan(&guessed_name));

    if ext == "zip" || ext == "7z" || ext == "rar" {
        println!("  类型: 压缩包 → 便携版");
    } else if ext == "msi" {
        println!("  类型: MSI 安装包 → 安装版");
    } else {
        println!("  类型: 可执行文件 → {}", if portable { "便携版" } else if installer_force { "安装版" } else { "不确定 (请确认)" });
    }

    print!("  确认安装？[Y/n] ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let input = input.trim().to_lowercase();
    if input == "n" || input == "no" {
        println!("  已取消");
        return Ok(());
    }

    // 下载
    let dl_dir = crate::paths::downloads_dir();
    std::fs::create_dir_all(&dl_dir)?;
    let target = dl_dir.join(filename);
    if !target.is_file() {
        println!("  下载中 ...");
        net::download_with_fallback(&vec![url.to_string()], &target, &net::DownloadConfig::default())?;
    }

    // 判断安装类型
    let install_type = if ext == "zip" || ext == "7z" || ext == "rar" {
        "portable"
    } else if ext == "msi" {
        "installer"
    } else if portable {
        "portable"
    } else {
        "installer"
    };

    let result = match install_type {
        "portable" => installer::install_portable(&guessed_name, "1.0", &target)?,
        "installer" => installer::install_installer(&guessed_name, "1.0", &target, None)?,
        _ => unreachable!(),
    };

    // 本地源不再写入外部文件，仅记录安装
    software::record_install(&guessed_name, "1.0", install_type, &result)?;
    println!("  {} 安装程序已退出，请确认已完成安装", color::cyan("➜"));

    // 便携版 → 检查 shim 路径是否在 PATH 中
    if install_type == "portable" {
        installer::check_bin_path_warning();
    }

    Ok(())
}
