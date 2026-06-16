use std::collections::{BTreeMap, HashMap, HashSet};

use color::{self, DisplayWidth, pad_left as pad, truncate};

use crate::helpers::name_matches;
use crate::cmd_names;
use crate::opts::ListOpts;
use crate::{paths, registry, software};

/// 扫描下载缓存目录，返回 {软件名 → (状态, 颜色)} 的映射
pub fn scan_download_cache() -> HashMap<String, (&'static str, &'static str)> {
    let mut result = HashMap::new();
    let downloads = paths::downloads_dir();
    if !downloads.is_dir() {
        return result;
    }

    // 加载源定义，用于精确的软件名匹配
    let defs = software::list_software_defs().unwrap_or_default();

    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }

            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // 提取软件名
            let (raw_name, is_downloading) = if let Some(stripped) = fname.strip_suffix(".downloading") {
                (stripped.to_string(), true)
            } else {
                (fname.clone(), false)
            };

            // 从文件名匹配软件名：检查每个源名是否作为文件名前缀
            let name_part = defs.iter()
                .find(|sd| {
                    let prefix = format!("{}-", sd.name);
                    raw_name.starts_with(&prefix)
                })
                .map(|sd| sd.name.clone())
                .or_else(|| {
                    // 回退：取第一个 hyphen 前的内容
                    raw_name.find('-')
                        .map(|pos| raw_name[..pos].to_string())
                })
                .unwrap_or_default();

            if name_part.is_empty() || name_part == raw_name {
                continue;
            }

            let (status, color_code) = if is_downloading {
                ("下载中", color::ansi::YELLOW) // 黄色
            } else {
                ("已下载", color::ansi::CYAN) // 青色
            };

            // 优先保留"已下载"状态（覆盖"下载中"）
            let entry = result.entry(name_part).or_insert((status, color_code));
            if !is_downloading {
                *entry = (status, color_code);
            }
        }
    }

    result
}

pub fn run_list(opts: ListOpts) -> anyhow::Result<()> {
    // Auto-init: if source dir is empty, suggest `as source update`
    let source = paths::apps_source_dir();
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("{}", color::yellow("  未找到源定义。首次使用请运行:"));
        println!("  {}\n", cmd_names::SOURCE_UPDATE_HINT);
        return Ok(());
    }

    let reg_installed = registry::scan_all_installed();
    let installed_db = software::read_installed_db().unwrap_or_default();
    let defs = software::list_software_defs()?;
    let dl_cache = scan_download_cache();

    // ── 类别概览模式 ──
    if opts.categories {
        return show_categories(&defs);
    }

    // ── 构建行数据 ──
    fn installer_marker(t: &str) -> &'static str {
        match t.to_lowercase().as_str() {
            "nsis" | "inno" | "exe" | "installer" => "EXE",
            "msi" | "appx" => "MSI",
            "portable" | "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" => "便携",
            _ => "EXE",
        }
    }

    // Rows: (名称, 版本, 安装状态, 安装颜色, 下载状态, 下载颜色,
    //        源标签, 源颜色, 分类, 安装标记)
    let mut rows: Vec<(String, String, &str, &str, &str, &str, &str, &str, String, &str)> = Vec::new();
    let mut seen_registry: HashSet<String> = HashSet::new();

    // 1. Registry entries
    for reg in &reg_installed {
        let rn = reg.get("display_name").cloned().unwrap_or_default();
        if rn.is_empty() || !seen_registry.insert(rn.clone()) {
            continue;
        }
        let (category, installer) = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            let cat = if sd.category.is_empty() { "未分类".to_string() } else { sd.category.clone() };
            let ins = sd.versions.get(&sd.default_version)
                .map(|vi| installer_marker(&vi.installer_type))
                .unwrap_or("EXE");
            (cat, ins)
        } else {
            ("其他".to_string(), "")
        };
        let has_source = defs.iter().any(|sd| name_matches(&rn, sd));
        let src_label = if has_source { "有" } else { "无" };
        let src_color = if has_source { color::ansi::GREEN } else { color::ansi::GRAY };
        let (dl_status, dl_color) = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            dl_cache.get(&sd.name).copied().unwrap_or(("未下载", color::ansi::GRAY))
        } else {
            ("未下载", color::ansi::GRAY)
        };
        let ver = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            installed_db.get(&sd.name)
                .map(|rec| rec.version.clone())
                .unwrap_or_else(|| reg.get("version").cloned().unwrap_or_default())
        } else {
            reg.get("version").cloned().unwrap_or_default()
        };
        rows.push((rn, ver, "已安装", color::ansi::GREEN, dl_status, dl_color, src_label, src_color, category, installer));
    }

    // 2. Source definitions not in registry
    for sd in &defs {
        let name = &sd.name;
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let category = if sd.category.is_empty() { "未分类".to_string() } else { sd.category.clone() };
        let installer = sd.versions.get(&sd.default_version)
            .map(|vi| installer_marker(&vi.installer_type))
            .unwrap_or("EXE");
        let already = reg_installed.iter().any(|r| {
            name_matches(&r.get("display_name").cloned().unwrap_or_default(), sd)
        });
        if already { continue; }
        let (dl_status, dl_color) = dl_cache.get(name)
            .copied()
            .unwrap_or(("未下载", color::ansi::GRAY));
        if let Some(rec) = installed_db.get(name) {
            rows.push((display.to_string(), rec.version.clone(),
                "已安装", color::ansi::GREEN, dl_status, dl_color, "有", color::ansi::GREEN, category, installer));
            continue;
        }
        rows.push((display.to_string(), sd.default_version.clone(),
            "未安装", color::ansi::GRAY, dl_status, dl_color, "有", color::ansi::GREEN, category, installer));
    }

    // ── 筛选 ──
    // 默认仅显示已安装；-a/--all 显示全部
    if !opts.all {
        rows.retain(|r| r.2 == "已安装");
    }
    if opts.downloaded   { rows.retain(|r| r.4 == "已下载"); }
    if opts.downloading  { rows.retain(|r| r.4 == "下载中"); }
    if opts.no_download  { rows.retain(|r| r.4 == "未下载"); }

    // 搜索增强：同时匹配名称、别名、描述
    if let Some(ref kw) = opts.search {
        let kw_lower = kw.to_lowercase();
        rows.retain(|r| {
            if r.0.to_lowercase().contains(&kw_lower) {
                return true;
            }
            for sd in &defs {
                if sd.display_name == r.0 || sd.name == r.0.to_lowercase() {
                    if sd.aliases.iter().any(|a| a.to_lowercase().contains(&kw_lower)) {
                        return true;
                    }
                    if sd.description.to_lowercase().contains(&kw_lower) {
                        return true;
                    }
                }
            }
            false
        });
    }

    // 分类过滤：修复为按 category 字段过滤
    if let Some(ref f) = opts.filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.8.to_lowercase().contains(&f_lower));
    }

    if rows.is_empty() {
        if let Some(ref f) = opts.filter {
            // 收集所有可用分类提示用户
            let mut cats: Vec<&str> = defs.iter()
                .map(|sd| if sd.category.is_empty() { "未分类" } else { sd.category.as_str() })
                .collect();
            cats.sort();
            cats.dedup();
            println!("没有匹配分类「{}」的软件。", f);
            println!("{}", color::gray(format!("可用分类: {}", cats.join(", "))));
        } else {
            println!("没有匹配的软件。");
        }
        return Ok(());
    }

    // Sort by name case-insensitive
    rows.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let max_name = rows.iter().map(|r| r.0.display_width()).max().unwrap_or(4).max(4).min(40);
    let max_ver = rows.iter().map(|r| r.1.display_width()).max().unwrap_or(4).max(4);

    // ── 分组显示 ──
    if opts.group {
        return show_grouped(&rows, max_name, max_ver);
    }

    // ── 平铺显示（默认） ──
    println!();
    let header = format!("{}{}{}{}{}{}",
        pad("名称", max_name + 2),
        pad("版本", max_ver + 2),
        pad("下载", 8 + 1),
        pad("状态", 8 + 1),
        pad("源", 4),
        pad("方式", 6));
    println!("{}", header);
    println!("{}", "-".repeat(header.display_width()));

    for (name, ver, _status, status_color, dl_status, dl_color, src_label, src_color, _cat, installer) in &rows {
        let name_d = truncate(name, max_name);
        let ver_d = truncate(ver, max_ver + 1);
        let ins_color = match *installer {
            "便携" => color::ansi::CYAN,
            _ => color::ansi::RESET,
        };
        println!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            pad(&name_d, max_name + 2),
            pad(&ver_d, max_ver + 2),
            dl_color,
            pad(dl_status, 8),
            color::ansi::RESET,
            " ",
            status_color,
            pad(_status, 8),
            color::ansi::RESET,
            " ",
            src_color,
            pad(src_label, 4),
            ins_color,
            pad(installer, 6),
            color::ansi::RESET,
        );
    }

    println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
    Ok(())
}

/// 显示类别概览
fn show_categories(defs: &[software::SoftwareDef]) -> anyhow::Result<()> {
    let mut cat_count: HashMap<&str, usize> = HashMap::new();
    for sd in defs {
        let c = if sd.category.is_empty() { "未分类" } else { &sd.category };
        *cat_count.entry(c).or_insert(0) += 1;
    }
    let mut sorted: Vec<(&&str, &usize)> = cat_count.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

    let max_cat_w = sorted.iter().map(|(c, _)| c.display_width()).max().unwrap_or(4).max(4).min(20);
    let max_num = sorted.iter().map(|(_, n)| n.to_string().len()).max().unwrap_or(2);
    let bar_max: usize = 30;
    let total: usize = defs.len();

    println!("\n{}", color::bold_yellow("类别概览"));
    println!("{}", color::gray("─".repeat(50)));
    for &(cat, count) in &sorted {
        let bar_w = (*count as f64 / total as f64 * bar_max as f64).round() as usize;
        let bar = "█".repeat(bar_w.max(1));
        println!("  {}  {:>w$}  {}",
            pad(cat, max_cat_w + 2),
            count,
            color::cyan(bar),
            w = max_num,
        );
    }
    println!("{}", color::gray("─".repeat(50)));
    println!("  共计 {} 个软件", total);
    Ok(())
}

/// 按分类分组显示
fn show_grouped(
    rows: &[(String, String, &str, &str, &str, &str, &str, &str, String, &str)],
    max_name: usize,
    max_ver: usize,
) -> anyhow::Result<()> {
    let mut by_cat: BTreeMap<String, Vec<&(String, String, &str, &str, &str, &str, &str, &str, String, &str)>> = BTreeMap::new();
    for r in rows {
        by_cat.entry(r.8.clone()).or_default().push(r);
    }
    for (cat, entries) in &by_cat {
        println!("\n{}  {}", color::bold_yellow(format!("{}", cat)), color::gray(format!("({})", entries.len())));
        // 子表头
        println!("  {}{}{}{}",
            pad("名称", max_name + 2),
            pad("版本", max_ver + 2),
            pad("安装", 8),
            pad("方式", 6));
        for (name, ver, _status, status_color, _dl_status, _dl_color, _src_label, _src_color, _cat, installer) in entries {
            let name_d = truncate(name, max_name);
            let ver_d = truncate(ver, max_ver + 1);
            let ins_color = match *installer {
                "便携" => color::ansi::CYAN,
                _ => color::ansi::RESET,
            };
            println!("  {}{}{}{}{}{}{}{}",
                pad(&name_d, max_name + 2),
                pad(&ver_d, max_ver + 2),
                status_color,
                pad(_status, 8),
                color::ansi::RESET,
                ins_color,
                pad(installer, 6),
                color::ansi::RESET,
            );
        }
    }
    println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
    Ok(())
}
