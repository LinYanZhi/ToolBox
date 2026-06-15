mod config;

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use clap::Parser;
use color::*;

#[derive(Parser)]
#[command(
    name = "e",
    version = "0.0.1",
    about,
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Cli {
    /// 要打开的路径（默认：当前目录）
    #[arg(value_name = "PATH")]
    path: Option<String>,

    /// 使用资源管理器打开指定路径
    #[arg(short = 'o', long = "open")]
    open: Option<String>,

    /// 显示所有环境变量（带颜色）
    #[arg(short = 's', long = "set")]
    set: bool,

    /// 显示 PATH 环境变量（带颜色）
    #[arg(short = 'p', long = "path")]
    show_path: bool,

    /// 变量名左对齐（配合 --set）
    #[arg(short = 'l', long = "left")]
    left: bool,

    /// 打开 Windows 环境变量对话框
    #[arg(short = 'g', long = "gui")]
    gui: bool,

    /// 不使用颜色输出
    #[arg(short = 'n', long = "no-color")]
    no_color: bool,

    /// 显示帮助信息
    #[arg(short = 'h', long = "help")]
    help: bool,

    /// 显示所有选项示例
    #[arg(short = 'e', long = "examples")]
    examples: bool,

    /// 显示版本信息
    #[arg(short = 'v', long = "version")]
    version: bool,
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", red("错误:"), e.to_string().lines().next().unwrap_or("参数解析失败"));
            eprintln!("{} 使用 -h 查看帮助", gray("提示:"));
            return;
        }
    };

    if cli.help {
        print_short_help();
        return;
    }

    if cli.examples {
        print_examples();
        return;
    }

    if cli.version {
        println!("{} {}", bold_cyan("e"), green("0.0.1"));
        println!("{}", gray("环境变量查看器 / 路径打开器"));
        return;
    }

    // --gui: 环境变量对话框
    if cli.gui {
        open_env_dialog();
        return;
    }

    // --set: 显示环境变量
    if cli.set {
        show_env_vars(cli.left, cli.no_color);
        return;
    }

    // --path: 显示 PATH
    if cli.show_path {
        show_path(cli.no_color);
        return;
    }

    // 默认模式：在资源管理器中打开路径
    let target = cli.open.or(cli.path).unwrap_or_else(|| ".".to_string());
    open_in_explorer(&target);
}

// ── 打开资源管理器 ──────────────────────────────────

/// 在资源管理器中打开指定路径
fn open_in_explorer(raw: &str) {
    let path_str = sanitize_path(raw);
    let path = Path::new(&path_str);

    // 如果路径不存在，尝试在当前目录下拼接
    let full_path = if path.exists() {
        path.to_path_buf()
    } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        let joined = cwd.join(&path_str);
        if joined.exists() {
            joined
        } else {
            eprintln!("{} 路径不存在: {}", red("错误:"), path_str);
            eprintln!("{} 使用 -h 查看帮助", gray("提示:"));
            return;
        }
    };

    // 如果是文件，打开父目录并选中文件
    if full_path.is_file() {
        let _ = Command::new("explorer")
            .args(["/select,", &*full_path.to_string_lossy()])
            .spawn();
    } else {
        let _ = Command::new("explorer")
            .arg(&*full_path.to_string_lossy())
            .spawn();
    }
}

/// 打开 Windows 环境变量对话框
fn open_env_dialog() {
    let _ = Command::new("rundll32.exe")
        .args(["sysdm.cpl,EditEnvironmentVariables"])
        .spawn();
}

// ── 显示环境变量 ──────────────────────────────────

fn show_env_vars(left_align: bool, no_color: bool) {
    let color_map = config::get_variable_color_map();
    let exclude_set = config::get_exclude_set();
    let mut env_vars: Vec<(String, String)> = std::env::vars().collect();

    env_vars.retain(|(name, _)| {
        !exclude_set.iter().any(|e| e.eq_ignore_ascii_case(name))
    });

    if env_vars.is_empty() {
        return;
    }

    let max_name_len = env_vars.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    env_vars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    println!();
    for (name, value) in &env_vars {
        if no_color {
            let formatted = if left_align {
                format!("{:<width$} = {}", name, value, width = max_name_len)
            } else {
                format!("{:>width$} = {}", name, value, width = max_name_len)
            };
            println!("{}", formatted);
        } else if let Some(color_name) = color_map.get(name) {
            let painted = paint(name, color_name);
            let formatted = if left_align {
                format!("{:<width$}", painted, width = max_name_len)
            } else {
                format!("{:>width$}", painted, width = max_name_len)
            };
            println!("{} = {}", formatted, value);
        } else {
            let formatted = if left_align {
                format!("{:<width$} = {}", name, value, width = max_name_len)
            } else {
                format!("{:>width$} = {}", name, value, width = max_name_len)
            };
            println!("{}", formatted);
        }
    }
}

// ── 显示 PATH ──────────────────────────────────

fn show_path(no_color: bool) {
    let color_map = config::get_path_color_map();
    let path = std::env::var("PATH").unwrap_or_default();
    let paths: Vec<&str> = path.split(';').collect();

    println!();
    for p in &paths {
        if p.is_empty() { continue; }
        if no_color {
            println!("{}", p);
        } else if let Some(color_name) = get_matching_color(p, &color_map) {
            println!("{}", paint(p, &color_name));
        } else {
            println!("{}", p);
        }
    }
}

fn get_matching_color(path: &str, color_map: &HashMap<String, String>) -> Option<String> {
    for (pattern, color) in color_map {
        if config::wildmatch(pattern, path) {
            return Some(color.clone());
        }
    }
    None
}

// ── ANSI 着色 ──────────────────────────────────

const ANSI_COLORS: &[(&str, &str)] = &[
    ("black", "30"), ("gray", "90"),
    ("blue", "34"), ("lightblue", "94"),
    ("green", "32"), ("lightgreen", "92"),
    ("cyan", "36"), ("lightcyan", "96"),
    ("red", "31"), ("lightred", "91"),
    ("purple", "35"), ("lightpurple", "95"),
    ("yellow", "33"), ("lightyellow", "93"),
    ("white", "37"), ("brightwhite", "97"),
];

fn paint(text: &str, color_or_style: &str) -> String {
    let lower = color_or_style.to_lowercase();
    if let Some(code) = ANSI_COLORS.iter().find(|(n, _)| *n == lower) {
        format!("\x1b[{}m{}\x1b[0m", code.1, text)
    } else {
        format!("\x1b[{}m{}\x1b[0m", color_or_style, text)
    }
}

// ── 路径清理 ──────────────────────────────────

fn sanitize_path(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    while (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len()-1].to_string();
    }
    while s.ends_with('"') || s.ends_with('\'') {
        s.pop();
    }
    s
}

// ── 帮助 ──────────────────────────────────

fn print_short_help() {
    println!("  {}", bold_cyan("e — 环境变量查看器 / 路径打开器"));
    println!();
    println!("  {}", bold_yellow("用法:"));
    println!("    {} [{}]  {}", green("e"), gray("路径"), gray("在资源管理器中打开路径"));
    println!("    {} -o [{}]", green("e"), gray("路径"));
    println!("    {} -s", green("e"));
    println!("    {} -p", green("e"));
    println!();
    println!("  {}", bold_yellow("选项:"));
    println!("  {:<18} {}", cyan("-h, --help"),    gray("显示简洁帮助"));
    println!("  {:<18} {}", cyan("-e, --examples"), gray("显示所有选项示例"));
    println!("  {:<18} {}", cyan("-v, --version"),  gray("显示版本信息"));
    println!("  {:<18} {}", cyan("-n, --no-color"), gray("不使用颜色输出"));
    println!("  {:<18} {}", cyan("-s, --set"),      gray("显示所有环境变量（带颜色）"));
    println!("  {:<18} {}", cyan("-p, --path"),     gray("显示 PATH（带颜色）"));
    println!("  {:<18} {}", cyan("-l, --left"),     gray("变量名左对齐（配合 --set）"));
    println!("  {:<18} {}", cyan("-g, --gui"),      gray("打开环境变量对话框"));
    println!("  {:<18} {}", cyan("-o, --open <路径>"), gray("在资源管理器打开路径"));
    println!();
    println!("  {}  {}  {}",
        gray("提示:"),
        gray("查看完整示例请使用"),
        cyan("e -e"));
}

fn print_examples() {
    println!("  {}", bold_cyan("e — 环境变量查看器 / 路径打开器"));
    println!();
    println!("  {}", bold_yellow("打开路径"));
    println!("  {:<20} {}", cyan("e"), gray("打开当前目录"));
    println!("  {:<20} {}", cyan("e C:\\path"), gray("打开指定目录"));
    println!("  {:<20} {}", cyan("e C:\\file.txt"), gray("打开父目录并选中文件"));
    println!("  {:<20} {}", cyan("e -o C:\\path"), gray("用 -o 打开"));
    println!();
    println!("  {}", bold_yellow("环境变量 / PATH"));
    println!("  {:<20} {}", cyan("e -s"), gray("显示所有环境变量"));
    println!("  {:<20} {}", cyan("e -s -l"), gray("左对齐变量名"));
    println!("  {:<20} {}", cyan("e -p"), gray("显示 PATH"));
    println!("  {:<20} {}", cyan("e -g"), gray("打开环境变量对话框"));
    println!();
    println!("  {}", bold_yellow("配置文件"));
    println!("  {:<20} {}",
        gray("e.yaml"), gray("放在 e.exe 同级，覆盖默认配色"));
    println!();
}
