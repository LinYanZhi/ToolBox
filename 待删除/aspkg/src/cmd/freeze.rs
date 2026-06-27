use crate::software;

/// as freeze — 导出当前电脑上的所有已安装软件清单
///
/// 输出内容覆盖两个来源：
///   1. as 管理的（便携版 / 安装版）
///   2. 注册表检测到的（含能匹配到源的和纯注册表的）
///
/// 输出格式兼容 `as install -r`，用户可编辑后批量安装。
pub fn run() -> anyhow::Result<()> {
    let installed = software::read_installed()?;
    let all_entries = software::read_all_entries()?;
    let (reg_matched, reg_unmatched) = software::scan_registry_installed(&all_entries);

    println!("# 导出时间: {}", software::now_str());
    println!("# 导出来源: as freeze");
    println!("# 说明: 名称=版本 的行可用 as install -r 批量重新安装");
    println!("#       以 # 开头的行仅为记录，不会安装");
    println!("#");
    println!("# ── as 管理的 ────────────────────────────────────");

    // 1) as 管理的版本（有源定义 → name=version，否则标记）
    let mut as_names: Vec<&String> = installed.keys().collect();
    as_names.sort();
    for name in &as_names {
        let rec = &installed[name.as_str()];
        if all_entries.contains_key(*name) {
            println!("{}{}", name, version_suffix(&rec.version));
        } else {
            println!("# {} ({}) — as 记录，源中无定义", name, rec.version);
        }
    }

    // 2) 注册表匹配到源的（未被 as 管理覆盖的）
    let mut has_reg_matched = false;
    for (name, infos) in &reg_matched {
        if installed.contains_key(name) {
            continue; // 已通过 as 管理输出
        }
        if !has_reg_matched {
            println!();
            println!("# ── 注册表+源（可安装）────────────────────────");
            has_reg_matched = true;
        }
        for info in infos {
            println!("# {}={} — 注册表检测，源中存在定义", name, info.version);
        }
    }

    // 3) 纯注册表（无源定义）
    let mut has_reg_only = false;
    for info in &reg_unmatched {
        if !has_reg_only {
            println!();
            println!("# ── 注册表（仅记录）───────────────────────────");
            has_reg_only = true;
        }
        let ver = if info.version.is_empty() { String::new() } else { format!(" ({})", info.version) };
        println!("# {}{} — 仅注册表检测，源中无定义", info.display_name, ver);
    }

    if installed.is_empty() && reg_matched.is_empty() && reg_unmatched.is_empty() {
        println!("（当前电脑未检测到任何已安装软件）");
    }

    Ok(())
}

/// 版本号后缀，空版本返回空字符串，非空返回 "=版本"
fn version_suffix(version: &str) -> String {
    if version.is_empty() {
        String::new()
    } else {
        format!("={}", version)
    }
}
