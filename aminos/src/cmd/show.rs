use color::{bold_cyan, bright_black, gray, cyan, green};
use crate::software;

/// as show — 查看软件详细信息
pub fn run(name: &str) -> anyhow::Result<()> {
    let (matched, entry) = match software::resolve_software(name) {
        Some(r) => r,
        None => {
            eprintln!("  {} 未找到软件 '{}'", color::yellow("提示:"), name);
            return Ok(());
        }
    };

    let installed = software::read_installed()?;
    let is_installed = installed.contains_key(&matched);

    println!();
    println!("  {}", bold_cyan(&matched));
    if !entry.aliases.is_empty() {
        println!("  {} {}", bright_black("别名:"), entry.aliases.join(", "));
    }
    if let Some(cat) = &entry.category {
        println!("  {} {}", bright_black("分类:"), cat);
    }
    println!("  {} {}", bright_black("状态:"), if is_installed { green("已安装") } else { gray("未安装") });
    println!();
    println!("  {}", bright_black("版本:"));
    for (ver, vi) in &entry.versions {
        let inst = if is_installed && installed.get(&matched).map(|r| &r.version) == Some(ver) {
            " ← 当前版本"
        } else {
            ""
        };
        let types: Vec<&str> = vi.urls.keys().map(|s| s.as_str()).collect();
        let suffix = if inst.is_empty() { String::new() } else { green(inst) };
        println!("    {} ({}){}", cyan(ver), types.join("/"), suffix);
    }
    println!();

    Ok(())
}
