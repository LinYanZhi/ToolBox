use color::*;
use crate::software;

pub fn run() -> anyhow::Result<()> {
    let entries = software::all_entries()?;

    let mut frozen = Vec::new();

    for (name, entry) in &entries {
        if let Some(detect) = &entry.detect {
            if let Some(info) = software::detect_from_registry(detect) {
                frozen.push((name.clone(), info.version));
            }
        }
    }

    frozen.sort_by(|a, b| a.0.cmp(&b.0));

    println!();
    println!("  {} 已安装软件清单:", bold_green("Freeze"));
    println!();

    for (name, version) in &frozen {
        println!("  {}={}", bold_cyan(name), version);
    }

    if frozen.is_empty() {
        println!("  {} 未检测到已安装的软件", gray("提示"));
    }

    println!();

    Ok(())
}