use crate::{registry, software};
use anyhow::bail;
use color;

pub fn run_info(name: &str, _show_urls: bool) -> anyhow::Result<()> {
    let sd = software::read_software_def(name).ok();

    // ── 有源定义：完整展示 ──
    if let Some(ref sd) = sd {
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let default_ver = &sd.default_version;

        println!();
        println!("  {}  {}", color::bold_cyan(display), color::gray(format!("({})", &sd.name)));
        println!("  {}", color::gray("─".repeat(50)));

        if !sd.description.is_empty() {
            println!("  {}", sd.description);
            println!();
        }

        if !sd.category.is_empty() {
            println!("  {}  {}", color::gray("分类:"), sd.category);
        }
        if !sd.aliases.is_empty() {
            println!("  {}  {}", color::gray("别名:"), sd.aliases.join(", "));
        }
        println!("  {}  {}", color::gray("默认版本:"), color::cyan(default_ver));
        if !sd.kind.is_empty() {
            println!("  {}  {}", color::gray("类型:"), sd.kind);
        }

        // Installation detection
        let installed_db = software::read_installed_db().unwrap_or_default();
        if let Some(rec) = installed_db.get(&sd.name) {
            println!("\n  {} (版本 {})", color::green("已安装"), rec.version);
            if !rec.install_path.is_empty() {
                println!("  路径: {}", rec.install_path);
            }
        } else {
            let mut found = false;
            for reg in registry::scan_all_installed() {
                if crate::helpers::name_matches(&reg.get("display_name").cloned().unwrap_or_default(), sd) {
                    println!("\n  {}", color::green("已安装"));
                    print_registry_info(&reg);
                    found = true;
                    break;
                }
            }
            if !found {
                println!("\n  {}", color::gray("未安装"));
            }
        }

        // Version list
        println!("\n  {}", color::gray("可用版本:"));
        let mut sorted_versions: Vec<&String> = sd.versions.keys().collect();
        sorted_versions.sort_by(|a, b| {
            let a_segs: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
            let b_segs: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
            for i in 0..a_segs.len().max(b_segs.len()) {
                let av = a_segs.get(i).copied().unwrap_or(0);
                let bv = b_segs.get(i).copied().unwrap_or(0);
                match bv.cmp(&av) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
            b.cmp(a)
        });
        for vk in &sorted_versions {
            let vi = &sd.versions[*vk];
            let marker = if vk.as_str() == default_ver { " ← 默认" } else { "" };
            let urls = &vi.urls;
            let first_url = urls.first().map(|s| s.as_str()).unwrap_or("无下载地址");
            let installer_type = if vi.installer_type.is_empty() { "(auto)" } else { &vi.installer_type };
            println!("    {}", color::green(format!("{}{}", vk, marker)));
            println!("      {} {}", color::gray("类型:"), installer_type);
            println!("      {} {}", color::gray("下载:"), first_url);
            for url in urls.iter().skip(1) {
                println!("           {}", url);
            }
        }
        println!();
        return Ok(());
    }

    // ── 无源定义：回退到注册表搜索 ──
    eprintln!("    ℹ  未找到「{}」的源定义，正在搜索注册表...", name);
    let reg_all = crate::registry::scan_all_installed_unfiltered();
    let name_lower = name.to_lowercase();
    let matches: Vec<_> = reg_all.into_iter()
        .filter(|entry| {
            entry.get("display_name")
                .map(|dn| dn.to_lowercase().contains(&name_lower))
                .unwrap_or(false)
        })
        .collect();

    if matches.is_empty() {
        bail!("未在注册表中找到匹配「{}」的软件", name);
    }

    for info in &matches {
        let dn = info.get("display_name").map(|s| s.as_str()).unwrap_or(name);
        println!("\n{}", color::bold_cyan(dn));
        println!("  {}", color::gray("─".repeat(50)));
        print_registry_info(info);
        // 提示无源
        println!("\n  {} 没有对应的源定义，无法管理更新或安装。", color::yellow("注意:"));
        println!("  如需提交源定义，请前往: {}", crate::repo::SOURCE_GITHUB_URL);
    }
    println!();

    Ok(())
}

/// 打印注册表条目中的基本信息。
fn print_registry_info(reg: &std::collections::HashMap<String, String>) {
    if let Some(v) = reg.get("version") {
        if !v.is_empty() {
            println!("  版本: {}", v);
        }
    }
    if let Some(p) = reg.get("publisher") {
        if !p.is_empty() {
            println!("  发行商: {}", p);
        }
    }
    if let Some(p) = reg.get("install_path") {
        if !p.is_empty() {
            println!("  路径: {}", p.trim_matches('"'));
        }
    }
}
