use std::io::Write;
use crate::software;
use crate::installer;

/// as install — 安装软件
pub fn run(
    names: Vec<String>,
    portable: bool,
    installer_force: bool,
) -> anyhow::Result<()> {
    if names.is_empty() {
        eprintln!("  {} 请指定要安装的软件名（如 as install 7zip everything）", color::yellow("提示:"));
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

    // 查找软件
    let (matched_name, entry) = match software::resolve_software(&name) {
        Some(r) => r,
        None => {
            eprintln!("  {} 未找到软件 '{}'", color::yellow("提示:"), name);
            eprintln!("  也可直接使用 URL 安装: as install <下载链接>");
            return Ok(());
        }
    };

    // 选择版本
    let ver = match &requested_version {
        Some(v) => {
            if entry.versions.contains_key(v) {
                v.clone()
            } else {
                eprintln!("  {} 版本 '{}' 不存在，可用版本:", color::yellow("警告:"), v);
                for vk in entry.versions.keys() {
                    eprintln!("    - {}", vk);
                }
                return Ok(());
            }
        }
        None => {
            // 取最新版本
            entry.versions.keys()
                .max_by(|a, b| {
                    let va: Vec<&str> = a.split('.').collect();
                    let vb: Vec<&str> = b.split('.').collect();
                    va.cmp(&vb)
                })
                .cloned()
                .unwrap_or_default()
        }
    };

    let vi = &entry.versions[&ver];
    if vi.urls.is_empty() {
        eprintln!("  {} {} 未配置下载地址", color::yellow("跳过"), matched_name);
        return Ok(());
    }

    // 确定安装类型
    let inst_type = if prefer_portable {
        "portable"
    } else if prefer_installer {
        "installer"
    } else if vi.urls.contains_key("portable") && !vi.urls.contains_key("installer") {
        "portable"
    } else if vi.urls.contains_key("installer") && !vi.urls.contains_key("portable") {
        "installer"
    } else {
        // 都有或都没有，询问用户
        if vi.urls.contains_key("portable") && vi.urls.contains_key("installer") {
            print!("  安装类型: [p]ortable 还是 [i]nstaller？[p/I] ");
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            match input.trim().to_lowercase().as_str() {
                "p" | "portable" => "portable",
                _ => "installer",
            }
        } else {
            // 尝试第一个可用的
            vi.urls.keys().next().map(|s| s.as_str()).unwrap_or("installer")
        }
    };

    println!("  {} {} v{} ({})", color::bold_cyan("安装"), matched_name, ver, inst_type);

    let urls = vi.urls.get(inst_type)
        .ok_or_else(|| anyhow::anyhow!("未找到 {} 类型的下载地址", inst_type))?;

    // 执行安装
    match inst_type {
        "portable" => installer::install_portable(&matched_name, &ver, urls)?,
        _ => installer::install_installer(&matched_name, &ver, urls)?,
    }

    Ok(())
}

fn install_url(url: &str, _portable: bool, _installer_force: bool) -> anyhow::Result<()> {
    // 简单下载并运行安装包
    installer::install_from_url(url)
}
