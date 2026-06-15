use crate::{paths, pe_version, software, cmd_names};
use color::{self, DisplayWidth, format_size, pad_left as pad, truncate};

pub fn run_cache(clear: bool, open: bool) -> anyhow::Result<()> {
    let downloads = paths::downloads_dir();

    if open {
        if downloads.exists() {
            let _ = std::process::Command::new("explorer").arg(&downloads).spawn();
            println!("已在资源管理器中打开: {}", downloads.display());
        } else {
            println!("缓存目录不存在，暂无已下载的文件。");
        }
        return Ok(());
    }

    if clear {
        return clear_cache(&downloads);
    }

    // List cached files
    if !downloads.is_dir() || downloads.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("暂无已下载的缓存文件。\n  目录: {}", downloads.display());
        return Ok(());
    }

    let source_defs = software::list_software_defs().unwrap_or_default();

    // entries: (文件名, 大小, PE版本, 一致性标记)
    let mut entries: Vec<(String, u64, String, String)> = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(&downloads) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                let pe_ver = pe_version::get_pe_version(&path).unwrap_or_else(|| "-".to_string());

                // 尝试匹配源定义，做一致性检查
                let consistency = if pe_ver == "-" || pe_ver.is_empty() {
                    String::new()
                } else {
                    // 从文件名推测软件名：依次检查每个源名是否匹配文件名前缀
                    let matched_sd = source_defs.iter().find(|sd| {
                        let prefix = format!("{}-", sd.name);
                        name.starts_with(&prefix)
                    });
                    match matched_sd {
                        Some(sd) if sd.default_version != pe_ver => {
                            color::yellow(" ⚠")
                        }
                        Some(_) => {
                            color::green(" ✓")
                        }
                        None => String::new(),
                    }
                };

                entries.push((name, size, pe_ver, consistency));
            }
        }
    }

    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let total_size: u64 = entries.iter().map(|(_, s, _, _)| s).sum();
    let max_name = entries.iter().map(|(n, _, _, _)| n.display_width()).max().unwrap_or(4).min(50);
    let max_ver = entries.iter().map(|(_, _, v, _)| v.display_width()).max().unwrap_or(4).max(4);

    println!("\n{}  {}\n", color::bold_yellow("下载缓存"), color::gray(format!("{}", downloads.display())));
    println!("  {}{}{}",
        pad("文件", max_name + 2),
        pad("版本", max_ver + 2),
        pad("大小", 12));

    for (name, size, ver, consistency) in &entries {
        println!("  {}{}{}{}",
            pad(&truncate(name, max_name), max_name + 2),
            pad(&truncate(ver, max_ver), max_ver + 2),
            pad(&format_size(*size), 12),
            consistency,
        );
    }

    // 图例
    if entries.iter().any(|(_, _, _, c)| !c.is_empty()) {
        println!();
        println!("  {} 版本与源定义一致  {} 与源定义不一致", color::green("✓"), color::yellow("⚠"));
    }

    println!("\n{}", color::gray(format!("共 {} 个文件，{} 空间", entries.len(), format_size(total_size))));

    println!("{}", color::gray(format!("  {}  清除缓存", cmd_names::CONFIG_CACHE_CLEAR)));
    println!("{}", color::gray(format!("  {}   在资源管理器中打开", cmd_names::CONFIG_CACHE_OPEN)));
    Ok(())
}

fn clear_cache(downloads: &std::path::Path) -> anyhow::Result<()> {
    if downloads.is_dir() {
        let mut count = 0u32;
        let mut total_size = 0u64;
        if let Ok(entries) = std::fs::read_dir(downloads) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    total_size += path.metadata().map(|m| m.len()).unwrap_or(0);
                    count += 1;
                }
            }
        }

        if count == 0 {
            println!("缓存目录为空，无需清除。");
            return Ok(());
        }

        // 确认提示
        let hint = format!(
            "将清除 {} 个缓存文件 ({} 空间)，是否继续? [y/N] ",
            count,
            format_size(total_size),
        );
        print!("  {} {}", color::yellow("⚠"), hint);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("  已取消。");
            return Ok(());
        }

        // Remove all files
        if let Ok(entries) = std::fs::read_dir(downloads) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        println!("{}", color::green(format!("已清除 {} 个缓存文件 ({} 空间)", count, format_size(total_size))));
    } else {
        println!("缓存目录不存在，无需清除。");
    }
    Ok(())
}
