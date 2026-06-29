use std::io::Write;
use color::*;
use crate::download;
use crate::installer;
use crate::software;

pub fn run(names: Vec<String>, portable: bool, installer_force: bool) -> anyhow::Result<()> {
    if names.is_empty() {
        eprintln!("  {} 请指定软件名（如 as install 7zip everything）", yellow("提示:"));
        return Ok(());
    }

    let mut targets = Vec::new();

    for input in &names {
        let (name, requested_version) = if let Some(eq_pos) = input.find('=') {
            (input[..eq_pos].to_string(), Some(input[eq_pos + 1..].to_string()))
        } else {
            (input.to_string(), None)
        };

        let (matched_name, entry) = match software::resolve(&name) {
            Some(r) => r,
            None => {
                let all = software::all_entries().unwrap_or_default();
                let fuzzy: Vec<&String> = all.keys()
                    .filter(|k| k.contains(&name.to_lowercase()))
                    .collect();

                if fuzzy.is_empty() {
                    eprintln!("  {} 未找到软件 '{}'", yellow("跳过"), bold_cyan(&name));
                    continue;
                } else if fuzzy.len() == 1 {
                    (fuzzy[0].clone(), all.get(fuzzy[0]).unwrap().clone())
                } else {
                    eprintln!("  {} '{}' 匹配到多个:", yellow("提示"), bold_cyan(&name));
                    for k in &fuzzy {
                        eprintln!("    - {}", k);
                    }
                    continue;
                }
            }
        };

        let version = match &requested_version {
            Some(v) => {
                if entry.versions.contains_key(v) {
                    v.clone()
                } else {
                    eprintln!("  {} '{}' 没有版本 '{}'", yellow("跳过"), bold_cyan(&matched_name), v);
                    continue;
                }
            }
            None => {
                let mut versions: Vec<&String> = entry.versions.keys().collect();
                versions.sort_by(|a, b| cmp_versions(b, a));
                if versions.len() == 1 {
                    versions[0].clone()
                } else {
                    println!("  {} 可用版本:", bold_cyan(&matched_name));
                    for (i, v) in versions.iter().enumerate() {
                        println!("    {}. {}", i + 1, v);
                    }
                    print!("  请选择版本 (1-{}): ", versions.len());
                    std::io::stdout().flush().ok();
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();
                    match input.trim().parse::<usize>() {
                        Ok(n) if n >= 1 && n <= versions.len() => versions[n - 1].clone(),
                        _ => {
                            eprintln!("  {} 无效选择", yellow("跳过"));
                            continue;
                        }
                    }
                }
            }
        };

        let vi = entry.versions[&version].clone();

        let url_type = if portable {
            "portable"
        } else if installer_force {
            "installer"
        } else {
            if vi.urls.contains_key("portable") {
                "portable"
            } else if vi.urls.contains_key("installer") {
                "installer"
            } else {
                eprintln!("  {} '{}' 没有可用的安装类型", yellow("跳过"), bold_cyan(&matched_name));
                continue;
            }
        };

        targets.push((matched_name.clone(), version.clone(), vi, url_type.to_string(), entry.detect.clone()));
    }

    if targets.is_empty() {
        return Ok(());
    }

    let download_targets: Vec<(String, String, software::VersionEntry, String)> =
        targets.iter().map(|(n, v, ve, ut, _)| (n.clone(), v.clone(), ve.clone(), ut.clone())).collect();

    let results = download::download_all(download_targets)?;

    for ((name, version, _, url_type, detect), result) in targets.into_iter().zip(results.into_iter()) {
        if !result.success {
            continue;
        }

        match url_type.as_str() {
            "installer" => {
                println!();
                println!("  开始安装 {} {}", bold_cyan(&name), bold_cyan(&version));
                let _ = installer::install_installer(&name, &version, &result.file_path, detect.as_ref());
            }
            "portable" => {
                println!();
                println!("  {} 便携版已下载到: {}", bold_green("完成"), bold_cyan(&result.file_path.display()));
            }
            _ => {}
        }
    }

    println!();
    println!("  {} 所有任务已完成", bold_green("完成"));
    println!();

    Ok(())
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