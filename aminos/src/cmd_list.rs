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

    let defs = software::list_software_defs().unwrap_or_default();

    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }

            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let (raw_name, is_downloading) = if let Some(stripped) = fname.strip_suffix(".downloading") {
                (stripped.to_string(), true)
            } else {
                (fname.clone(), false)
            };

            let name_part = defs.iter()
                .find(|sd| {
                    let prefix = format!("{}-", sd.name);
                    raw_name.starts_with(&prefix)
                })
                .map(|sd| sd.name.clone())
                .or_else(|| {
                    raw_name.find('-')
                        .map(|pos| raw_name[..pos].to_string())
                })
                .unwrap_or_default();

            if name_part.is_empty() || name_part == raw_name {
                continue;
            }

            let (status, color_code) = if is_downloading {
                ("下载中", color::ansi::YELLOW)
            } else {
                ("已下载", color::ansi::CYAN)
            };

            let entry = result.entry(name_part).or_insert((status, color_code));
            if !is_downloading {
                *entry = (status, color_code);
            }
        }
    }

    result
}

// ── 行数据结构 ──────────────────────────────────────────

struct Row {
    name: String,
    version: String,
    install_status: &'static str,
    install_color: &'static str,
    dl_status: &'static str,
    dl_color: &'static str,
    src_label: &'static str,
    src_color: &'static str,
    category: String,
    installer: &'static str,
}

fn installer_marker(t: &str) -> &'static str {
    match t.to_lowercase().as_str() {
        "nsis" | "inno" | "exe" | "installer" => "EXE",
        "msi" | "appx" => "MSI",
        "portable" | "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" => "便携",
        _ => "EXE",
    }
}

// ── 构建行数据 ──────────────────────────────────────────

fn build_rows(
    reg_installed: &[HashMap<String, String>],
    installed_db: &HashMap<String, software::InstallRecord>,
    defs: &[software::SoftwareDef],
    dl_cache: &HashMap<String, (&'static str, &'static str)>,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut seen_registry: HashSet<String> = HashSet::new();

    // 1. Registry entries
    for reg in reg_installed {
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
        rows.push(Row {
            name: rn, version: ver,
            install_status: "已安装", install_color: color::ansi::GREEN,
            dl_status, dl_color, src_label, src_color,
            category, installer,
        });
    }

    // 2. Source definitions not in registry
    for sd in defs {
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
            rows.push(Row {
                name: display.to_string(), version: rec.version.clone(),
                install_status: "已安装", install_color: color::ansi::GREEN,
                dl_status, dl_color,
                src_label: "有", src_color: color::ansi::GREEN,
                category, installer,
            });
            continue;
        }
        rows.push(Row {
            name: display.to_string(), version: sd.default_version.clone(),
            install_status: "未安装", install_color: color::ansi::GRAY,
            dl_status, dl_color,
            src_label: "有", src_color: color::ansi::GREEN,
            category, installer,
        });
    }

    rows
}

// ── 筛选 ────────────────────────────────────────────────

fn apply_filters(
    rows: &mut Vec<Row>,
    opts: &ListOpts,
    defs: &[software::SoftwareDef],
) {
    if !opts.all {
        rows.retain(|r| r.install_status == "已安装");
    }
    if opts.downloaded   { rows.retain(|r| r.dl_status == "已下载"); }
    if opts.downloading  { rows.retain(|r| r.dl_status == "下载中"); }
    if opts.no_download  { rows.retain(|r| r.dl_status == "未下载"); }

    if let Some(ref kw) = opts.search {
        let kw_lower = kw.to_lowercase();
        rows.retain(|r| {
            if r.name.to_lowercase().contains(&kw_lower) {
                return true;
            }
            for sd in defs {
                if sd.display_name == r.name || sd.name == r.name.to_lowercase() {
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

    if let Some(ref f) = opts.filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.category.to_lowercase().contains(&f_lower));
    }
}

// ── 自适应列宽 ──────────────────────────────────────────

fn compute_widths(rows: &[Row]) -> (usize, usize, usize) {
    let index_w = format!("{}", rows.len()).len();
    let max_name_content = rows.iter().map(|r| r.name.display_width()).max().unwrap_or(4).max(4);
    let max_ver = rows.iter().map(|r| r.version.display_width()).max().unwrap_or(4).max(4);

    let tw = terminal_width() as usize;
    let fixed_other = index_w + 29 + max_ver + 2;
    let max_name = if tw > fixed_other + 10 {
        max_name_content.min(tw - fixed_other)
    } else {
        max_name_content.min(40)
    };

    (index_w, max_name, max_ver)
}

// ── 渲染：平铺 ──────────────────────────────────────────

fn render_flat(rows: &[Row]) {
    let (index_w, max_name, max_ver) = compute_widths(rows);

    println!();
    let source_updated = software::read_source_updated();
    if !source_updated.is_empty() {
        let tool_updated = software::read_tool_source_updated();
        if !tool_updated.is_empty() && tool_updated > source_updated {
            println!("  {}  {}", color::gray("源更新日期:"), color::yellow(&tool_updated));
        } else {
            println!("  {}  {}", color::gray("源更新日期:"), color::yellow(&source_updated));
        }
    }

    let header = format!("{}{}{}{}{}{}{}",
        pad("#", index_w + 1),
        pad("名称", max_name + 2),
        pad("版本", max_ver + 2),
        pad("下载", 8 + 1),
        pad("状态", 8 + 1),
        pad("源", 4),
        pad("方式", 6));
    println!("{}", header);
    println!("{}", "-".repeat(header.display_width()));

    for (i, r) in rows.iter().enumerate() {
        let idx = i + 1;
        let name_d = truncate(&r.name, max_name);
        let ver_d = truncate(&r.version, max_ver + 1);
        let ins_color = match r.installer {
            "便携" => color::ansi::CYAN,
            _ => color::ansi::RESET,
        };
        println!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            color::gray(&format!("{:>w$}", idx, w = index_w)),
            " ",
            pad(&name_d, max_name + 2),
            pad(&ver_d, max_ver + 2),
            r.dl_color,
            pad(r.dl_status, 8),
            color::ansi::RESET,
            " ",
            r.install_color,
            pad(r.install_status, 8),
            color::ansi::RESET,
            " ",
            r.src_color,
            pad(r.src_label, 4),
            ins_color,
            pad(r.installer, 6),
        );
    }

    println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
}

// ── 渲染：分组 ──────────────────────────────────────────

fn render_grouped(rows: &[Row]) {
    let (index_w, max_name, max_ver) = compute_widths(rows);

    let mut by_cat: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by_cat.entry(r.category.clone()).or_default().push(r);
    }
    let mut global_idx = 0usize;
    for (cat, entries) in &by_cat {
        println!("\n{}  {}", color::bold_yellow(&cat), color::gray(format!("({})", entries.len())));
        println!("  {}{}{}{}{}",
            pad("#", index_w + 1),
            pad("名称", max_name + 2),
            pad("版本", max_ver + 2),
            pad("安装", 8),
            pad("方式", 6));
        for r in entries {
            global_idx += 1;
            let name_d = truncate(&r.name, max_name);
            let ver_d = truncate(&r.version, max_ver + 1);
            let ins_color = match r.installer {
                "便携" => color::ansi::CYAN,
                _ => color::ansi::RESET,
            };
            println!("  {}{}{}{}{}{}{}{}{}",
                color::gray(&format!("{:>w$}", global_idx, w = index_w)),
                " ",
                pad(&name_d, max_name + 2),
                pad(&ver_d, max_ver + 2),
                r.install_color,
                pad(r.install_status, 8),
                color::ansi::RESET,
                ins_color,
                pad(r.installer, 6),
            );
        }
    }
    println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
}

// ── 公开入口 ────────────────────────────────────────────

pub fn run_list(opts: ListOpts) -> anyhow::Result<()> {
    let source = paths::apps_source_dir();
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("{}", color::yellow("  未找到源定义。首次使用请运行:"));
        println!("  {}\n", cmd_names::SOURCE_UPDATE_HINT);
        return Ok(());
    }

    let reg_installed: Vec<_> = {
        let raw = registry::scan_all_installed();
        let list_cfg = crate::list_config::ListConfig::load();
        raw.into_iter()
            .filter(|reg| {
                let dn = reg.get("display_name").map(|s| s.as_str()).unwrap_or("");
                !list_cfg.is_hidden(dn)
            })
            .collect()
    };
    let installed_db = software::read_installed_db().unwrap_or_default();
    let defs = software::list_software_defs()?;
    let dl_cache = scan_download_cache();

    // 类别概览模式
    if opts.categories {
        return show_categories(&defs);
    }

    // 构建 + 筛选
    let mut rows = build_rows(&reg_installed, &installed_db, &defs, &dl_cache);
    apply_filters(&mut rows, &opts, &defs);

    if rows.is_empty() {
        if let Some(ref f) = opts.filter {
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

    // 排序
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // 渲染
    if opts.group {
        render_grouped(&rows);
    } else {
        render_flat(&rows);
    }

    Ok(())
}

/// 获取终端宽度（列数），失败时默认 80。
fn terminal_width() -> u16 {
    terminal_size::terminal_size()
        .map(|(w, _h)| w.0)
        .unwrap_or(80)
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
