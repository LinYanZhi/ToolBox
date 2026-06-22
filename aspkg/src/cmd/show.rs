use color::*;
use crate::software;

/// as show <name> — 显示软件详细信息
pub fn run(name: &str) -> anyhow::Result<()> {
    let (entry_name, entry) = software::resolve_software(name, "查看")?;

    // 所有条目均为内置源（不再支持外部文件）
    let is_builtin = true;

    let installed = software::read_installed()?;
    let install_record = installed.get(&entry_name);

    // 标签宽度（所有标签左对齐，含冒号）
    let label_w = 5usize;

    // ── 基本信息 ──────────────────────────────────
    println!("{} {}", label("名称", label_w), bold_cyan(&entry_name));

    if !entry.aliases.is_empty() {
        println!("{} {}", label("别名", label_w), cyan(&entry.aliases.join(", ")));
    }

    if let Some(ref cat) = entry.category {
        println!("{} {}", label("分类", label_w), yellow(cat));
    }

    let source_label = if is_builtin { "内置源" } else { "本地扩展" };
    println!("{} {}", label("来源", label_w), bright_black(source_label));

    // ── 版本列表 ──────────────────────────────────
    let mut versions: Vec<&String> = entry.versions.keys().collect();
    versions.sort_by(|a, b| {
        let a_ver = parse_version(a);
        let b_ver = parse_version(b);
        b_ver.cmp(&a_ver)
    });

    // 判断是否有比当前更新版本（第一个版本号最大）
    let has_newer = if let (Some(rec), Some(first)) = (&install_record, versions.first()) {
        parse_version(first) > parse_version(&rec.version)
    } else {
        false
    };

    for (vi, ver) in versions.iter().enumerate() {
        let is_current = install_record.map(|r| r.version == **ver).unwrap_or(false);
        let ver_entry = &entry.versions[*ver];

        // 版本号着色
        let ver_colored = if is_current {
            bold_green(ver)        // 已安装 → 浅绿
        } else if has_newer && vi == 0 {
            bold_bright_cyan(ver)  // 新版本 → 浅蓝
        } else {
            cyan(ver)              // 默认 → 青色
        };

        // 版本后缀标记
        let ver_display = if is_current {
            format!("{} {}", ver_colored, gray("(当前版本)"))
        } else if has_newer && vi == 0 {
            format!("{} {}", ver_colored, gray("(新版本)"))
        } else {
            ver_colored.to_string()
        };

        if vi == 0 {
            println!();
            println!("{} {}", label("版本", label_w), ver_display);
        } else {
            println!("{} {}", label("", label_w), ver_display);
        }

        // URL 列表（对齐在版本号后面）
        let mut types: Vec<&String> = ver_entry.urls.keys().collect();
        types.sort();
        let indent = " ".repeat(label_w + 1);
        for t in &types {
            for url in &ver_entry.urls[*t] {
                let type_tag = match t.as_str() {
                    "portable"  => bright_magenta("便携版"),
                    "installer" => bright_yellow("安装版"),
                    _ => bright_black(t),
                };
                println!("{} {} {}", indent, type_tag, gray(url));
            }
        }
    }

    // ── 多个匹配提示 ──────────────────────────────
    // resolve_software 已处理多选的交互, 无需额外提示

    println!();
    Ok(())
}

/// 左对齐固定宽度标签，含冒号。
fn label(s: &str, w: usize) -> String {
    let cw = s.display_width();
    if cw >= w {
        format!("{}:", s)
    } else {
        format!("{}:{}", s, " ".repeat(w - cw))
    }
}

fn parse_version(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}
