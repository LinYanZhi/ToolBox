mod download;
mod software;

use std::io::Write;

use arg::*;
use color::{DisplayWidth, pad_left};
use color::*;
use terminal_size::{Width, terminal_size};

// ── CLI 定义 ────────────────────────────────────────

fn build_cmd() -> Cmd {
    Cmd::new("as")
        .about("极简 Windows 软件下载器 — 只下载，不安装")
        .arg(flag("help", 'h', "显示帮助").global())
        .arg(flag("examples", 'e', "显示使用示例"))
        .arg(flag("version", 'V', "显示版本号").global())
        .sub_alias(
            Cmd::new("install").about("下载软件")
                .arg(arg::ArgDef::value("names", None, "软件名（可多个）").positional().multi()),
            &["i"],
        )
        .sub_alias(
            Cmd::new("list").about("列出所有支持的软件"),
            &["l"],
        )
}

fn main() {
    init();
    let cmd = build_cmd();

    let argv: Vec<String> = std::env::args().collect();
    let args = match parse(&cmd, &argv) {
        Ok(a) => a,
        Err(e) => {
            print_error(&e);
            return;
        }
    };

    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "as".into());

    // 全局 flag 处理
    if args.flag("help") {
        print_help(&cmd, &exe_path);
        return;
    }
    if args.flag("examples") {
        print_examples(&cmd);
        return;
    }
    if args.flag("version") {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        print_version(&cmd, VERSION, "github.com/LinYanZhi/ToolBox");
        return;
    }

    match args.sub.as_deref() {
        Some("install") => {
            let sub = args.sub_args.as_ref().unwrap();
            let names: Vec<String> = sub.values("names").iter().map(|s| s.to_string()).collect();
            cmd_install(names);
        }
        Some("list") => cmd_list(),
        _ => {
            print_help(&cmd, &exe_path);
        }
    }
}

fn print_examples(cmd: &Cmd) {
    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| cmd.name.clone());

    println!("{}", bright_blue(&exe_path));
    println!();
    println!("{}是一个{}", bright_cyan(&cmd.name), gray("示例："));
    println!();

    let examples: &[(&str, &str)] = &[
        ("list",               "列出所有支持的软件"),
        ("install 7zip",       "下载 7zip（交互选择版本）"),
        ("i everything",       "下载 everything（简写 i）"),
        ("install rust=1.85",  "指定版本下载"),
        ("install py rust",    "一次下载多个软件"),
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    println!("{}", bright_blue("使用示例:"));
    for (cmd_str, desc) in examples {
        let cmd_display = format!("{} {}", cmd.name, cmd_str);
        println!("  {}  {}",
            pad_left(&bright_cyan(&cmd_display), max_w),
            gray(desc));
    }
    println!();
}

// ── 颜色快捷 ──

fn bright_blue(text: &str) -> String  { color::Style::new(94).paint(text) }
fn bright_cyan(text: &str) -> String  { color::Style::new(96).paint(text) }
fn gray(text: &str) -> String         { color::Style::new(90).paint(text) }

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

    println!("{}{}{}{}{}",
        pad_left("名称", max_name_w),
        " ".repeat(gap),
        pad_left("说明", max_desc_w),
        " ".repeat(gap),
        "版本",
    );

    println!("{}{}{}{}{}",
        "-".repeat(max_name_w),
        " ".repeat(gap),
        "-".repeat(max_desc_w),
        " ".repeat(gap),
        "-".repeat(max_ver_w),
    );

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
