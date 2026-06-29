use color::*;
use crate::software;

pub fn run(name: &str) -> anyhow::Result<()> {
    let (matched_name, entry) = match software::resolve(name) {
        Some(r) => r,
        None => {
            eprintln!("  {} 未找到软件 '{}'", yellow("错误"), bold_cyan(name));
            return Ok(());
        }
    };

    println!();
    println!("  {} {}", bold_green("软件信息:"), bold_cyan(&matched_name));
    println!();

    if !entry.desc.is_empty() {
        println!("    {}: {}", bold_cyan("说明"), entry.desc);
    }

    if !entry.aliases.is_empty() {
        println!("    {}: {}", bold_cyan("别名"), entry.aliases.join(", "));
    }

    if let Some(category) = &entry.category {
        println!("    {}: {}", bold_cyan("分类"), category);
    }

    println!("    {}:", bold_cyan("版本"));
    let mut versions: Vec<(&String, &software::VersionEntry)> = entry.versions.iter().collect();
    versions.sort_by(|a, b| cmp_versions(b.0, a.0));
    for (ver, ve) in versions {
        println!("      {}", ver);
        for (url_type, urls) in &ve.urls {
            println!("        {}:", bold_cyan(url_type));
            for url in urls {
                println!("          {}", url);
            }
        }
    }

    let (status, install_path) = get_install_status(&entry);

    println!();
    println!("    {}: {}", bold_cyan("安装状态"), status);
    if !install_path.is_empty() {
        println!("    {}: {}", bold_cyan("安装路径"), install_path);
    }

    println!();

    Ok(())
}

fn get_install_status(entry: &software::SoftwareEntry) -> (String, String) {
    if let Some(detect) = &entry.detect {
        if let Some(info) = software::detect_from_registry(detect) {
            (bold_green("已安装"), info.install_path.unwrap_or_default())
        } else {
            (gray("未安装"), String::new())
        }
    } else {
        (gray("未安装"), String::new())
    }
}

fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let va: Vec<i64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let vb: Vec<i64> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    let max_len = va.len().max(vb.len());
    for i in 0..max_len {
        let na = va.get(i).copied().unwrap_or(0);
        let nb = vb.get(i).copied().unwrap_or(0);
        if na != nb {
            return na.cmp(&nb);
        }
    }
    if a == "latest" && b != "latest" { return std::cmp::Ordering::Greater; }
    if b == "latest" && a != "latest" { return std::cmp::Ordering::Less; }
    std::cmp::Ordering::Equal
}