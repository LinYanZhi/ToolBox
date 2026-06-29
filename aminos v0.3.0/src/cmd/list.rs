use color::{self, DisplayWidth, pad_left as pad, gray, cyan, bright_cyan, bright_green, bright_yellow};
use crate::software;

/// as list — 列出已安装软件
pub fn run(show_all: bool) -> anyhow::Result<()> {
    let entries = software::read_all_entries()?;
    let installed = software::read_installed()?;

    fn is_non_software(name: &str) -> bool {
        let n = name.to_lowercase();
        n.contains("microsoft visual c++")
            || n.contains("windows sdk")
            || n.contains("update for windows")
            || n.contains("microsoft update health")
            || n.starts_with("vs_")
    }

    if installed.is_empty() {
        println!();
        println!("  暂无已安装的软件");
        println!("  {} as install <软件名>", gray("使用:"));
        println!();
        return Ok(());
    }

    struct Row {
        name: String,
        version: String,
        inst_type: String,
        category: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (name, rec) in &installed {
        if !show_all && is_non_software(name) {
            continue;
        }
        seen.insert(name.clone());
        let entry = entries.get(name);
        let cat = entry.and_then(|e| e.category.clone()).unwrap_or_else(|| "-".to_string());
        rows.push(Row {
            name: name.clone(),
            version: rec.version.clone(),
            inst_type: rec.r#type.clone(),
            category: cat,
        });
    }

    // 注册表检测（仅 -a 时）
    if show_all {
        let reg_all = sys::registry::scan_all_installed_unfiltered();
        for info in &reg_all {
            let n = info.get("DisplayName").cloned().unwrap_or_default();
            if n.is_empty() || seen.contains(&n) { continue; }
            rows.push(Row {
                name: n.clone(),
                version: info.get("DisplayVersion").cloned().unwrap_or_default(),
                inst_type: "注册表".into(),
                category: "-".into(),
            });
        }
    }

    if rows.is_empty() {
        println!("  暂无已安装的软件");
        return Ok(());
    }

    let name_w = rows.iter().map(|r| r.name.display_width()).max().unwrap_or(10).max(4);
    let ver_w = rows.iter().map(|r| r.version.display_width()).max().unwrap_or(7).max(6);
    let type_w = rows.iter().map(|r| r.inst_type.display_width()).max().unwrap_or(6).max(6);
    let cat_w = rows.iter().map(|r| r.category.display_width()).max().unwrap_or(6).max(6);

    println!();
    println!("  {} {} {} {}",
        pad(color::bold("名称"), name_w),
        pad(color::bold("版本"), ver_w),
        pad(color::bold("类型"), type_w),
        pad(color::bold("分类"), cat_w),
    );
    println!("  {}",
        gray(format!("{:-<1$}", "", name_w + ver_w + type_w + cat_w + 6)));

    for row in &rows {
        if show_all && row.inst_type == "注册表" {
            println!("  {} {} {} {}",
                pad(gray(&row.name), name_w),
                pad(gray(&row.version), ver_w),
                pad(gray(&row.inst_type), type_w),
                pad(gray(&row.category), cat_w),
            );
        } else {
            println!("  {} {} {} {}",
                pad(bright_cyan(&row.name), name_w),
                pad(bright_green(&row.version), ver_w),
                pad(bright_yellow(&row.inst_type), type_w),
                pad(cyan(&row.category), cat_w),
            );
        }
    }
    println!();

    Ok(())
}
