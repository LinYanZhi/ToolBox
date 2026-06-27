mod download;
mod software;

use std::io::Write;

use clap::Parser;
use color::{DisplayWidth, pad_left};
use color::*;
use terminal_size::{Width, terminal_size};

// ── CLI ────────────────────────────────────────────

#[derive(Parser)]
#[clap(name = "as", version, about = "极简 Windows 软件下载器 — 只下载，不安装")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    /// 下载软件
    #[clap(name = "install", aliases = &["i"])]
    Install {
        /// 软件名（可多个）
        names: Vec<String>,
    },
    /// 列出所有支持的软件
    #[clap(name = "list", aliases = &["l"])]
    List,
}

fn main() {
    color::ansi::enable_ansi();
    let cli = Cli::parse();

    match cli.command {
        Command::Install { names } => cmd_install(names),
        Command::List => cmd_list(),
    }
}

// ── as list ────────────────────────────────────────

fn cmd_list() {
    let entries = match software::all_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("错误: {}", e);
            std::process::exit(1);
        }
    };

    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    let mut sorted: Vec<(&String, &software::SoftwareEntry)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    // 计算各列最大宽度（含表头）
    let mut max_name_w = "名称".display_width();
    let mut max_desc_w = "说明".display_width();
    let mut max_ver_w = "版本".display_width();

    struct Line {
        name: String,
        desc: String,
        versions: Vec<String>,
        ver_line: String,
    }

    let mut lines: Vec<Line> = Vec::new();

    for (name, entry) in &sorted {
        let nw = name.display_width();
        let dw = entry.desc.display_width();
        if nw > max_name_w { max_name_w = nw; }
        if dw > max_desc_w { max_desc_w = dw; }
    }

    // 限制说明列宽度，防止过宽
    let max_desc_w = max_desc_w.min(20);

    for (name, entry) in &sorted {
        let mut versions: Vec<&str> = entry.versions.keys().map(|s| s.as_str()).collect();
        versions.sort_by(|a, b| cmp_versions(b, a));

        let ver_line = versions.join(", ");

        let vw = ver_line.display_width();
        if vw > max_ver_w { max_ver_w = vw; }

        lines.push(Line {
            name: name.to_string(),
            desc: truncate_display(&entry.desc, max_desc_w),
            versions: versions.iter().map(|s| s.to_string()).collect(),
            ver_line,
        });
    }

    let gap = 2;

    // 表头
    println!("{}{}{}{}{}",
        pad_left("名称", max_name_w),
        " ".repeat(gap),
        pad_left("说明", max_desc_w),
        " ".repeat(gap),
        "版本",
    );

    // 分隔线
    println!("{}{}{}{}{}",
        "-".repeat(max_name_w),
        " ".repeat(gap),
        "-".repeat(max_desc_w),
        " ".repeat(gap),
        "-".repeat(max_ver_w),
    );

    // 内容
    for line in &lines {
        let name_display = truncate_display(&line.name, max_name_w);
        let remaining = term_width.saturating_sub(max_name_w + gap + max_desc_w + gap);

        if line.ver_line.display_width() <= remaining {
            println!("{}{}{}{}{}",
                pad_left(name_display, max_name_w),
                " ".repeat(gap),
                pad_left(&line.desc, max_desc_w),
                " ".repeat(gap),
                line.ver_line,
            );
        } else {
            println!("{}{}{}{}{}",
                pad_left(name_display, max_name_w),
                " ".repeat(gap),
                pad_left(&line.desc, max_desc_w),
                " ".repeat(gap),
                line.versions[0],
            );
            for v in &line.versions[1..] {
                println!("{}{}{}{}{}",
                    " ".repeat(max_name_w),
                    " ".repeat(gap),
                    " ".repeat(max_desc_w),
                    " ".repeat(gap),
                    v,
                );
            }
        }
    }

    println!();
    println!("共 {} 个", sorted.len());
    println!();
}

// ── as install ─────────────────────────────────────

fn cmd_install(names: Vec<String>) {
    if names.is_empty() {
        eprintln!("  {} 请指定软件名（如 as install 7zip everything）", yellow("提示:"));
        return;
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
        targets.push((matched_name, version, vi));
    }

    if targets.is_empty() {
        return;
    }

    if let Err(e) = download::download_all(targets) {
        eprintln!("  {} {}", bold_red("错误:"), e);
    }
}

// ── 辅助函数 ──────────────────────────────────────

/// 版本号比较（降序）
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

/// 截断字符串到指定显示宽度
fn truncate_display(s: &str, max: usize) -> String {
    let w = s.display_width();
    if w <= max {
        s.to_string()
    } else {
        let mut result = String::new();
        let mut cur = 0usize;
        for c in s.chars() {
            let cw = c.to_string().display_width();
            if cur + cw > max.saturating_sub(1) {
                result.push('…');
                break;
            }
            result.push(c);
            cur += cw;
        }
        result
    }
}
