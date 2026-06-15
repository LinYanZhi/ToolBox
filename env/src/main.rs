mod config;

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Option<Commands>,

    /// 要打开的路径
    #[arg(value_name = "PATH")]
    path: Option<String>,

    /// 使用资源管理器打开指定路径
    #[arg(short = 'o', long = "open")]
    open: Option<String>,

    /// 打开 Windows 环境变量对话框
    #[arg(short = 'g', long = "gui")]
    gui: bool,

    /// 不使用颜色输出
    #[arg(short = 'n', long = "no-color", global = true)]
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

#[derive(Subcommand)]
enum Commands {
    /// 显示所有环境变量（带颜色）
    Set {
        /// 变量名左对齐
        #[arg(short = 'l', long = "left")]
        left: bool,
        /// 按分号换行显示（不依赖终端宽度）
        #[arg(short = 's', long = "semicolon")]
        semicolon: bool,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 显示 PATH（带颜色）
    Path {
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
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

    // 子命令优先
    match cli.command {
        Some(Commands::Set { left: _, semicolon: _, help: true }) => {
            print_subcommand_help("set");
            return;
        }
        Some(Commands::Set { left, semicolon, help: false }) => {
            show_env_vars(left, semicolon, cli.no_color);
            return;
        }
        Some(Commands::Path { help: true }) => {
            print_subcommand_help("path");
            return;
        }
        Some(Commands::Path { help: false }) => {
            show_path(cli.no_color);
            return;
        }
        None => {}
    }

    // --gui: 环境变量对话框
    if cli.gui {
        open_env_dialog();
        return;
    }

    // 无参数：显示帮助
    if cli.path.is_none() && cli.open.is_none() {
        print_short_help();
        return;
    }

    // 在资源管理器中打开路径
    let target = cli.open.or(cli.path).unwrap();
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

fn show_env_vars(left_align: bool, semicolon: bool, no_color: bool) {
    let color_map = config::get_variable_color_map();
    let exclude_set = config::get_exclude_set();
    let mut env_vars: Vec<(String, String)> = std::env::vars().collect();

    // 过滤：排除隐藏变量（以 = 开头）和用户配置的排除项
    env_vars.retain(|(name, _)| {
        if name.starts_with('=') {
            return false;
        }
        !exclude_set.iter().any(|e| e.eq_ignore_ascii_case(name))
    });

    if env_vars.is_empty() {
        return;
    }

    // 按名字自然排序（不区分大小写）
    env_vars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let max_name_len = env_vars.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let term_w = terminal_width() as usize;
    let prefix_indent = max_name_len + 3; // 名字 + " = "

    println!();
    for (name, value) in &env_vars {
        // 大小写不敏感查找配色
        let color = color_map.get(name)
            .or_else(|| color_map.get(&name.to_uppercase()))
            .or_else(|| color_map.get(&name.to_lowercase()));

        // 构建前缀（变量名部分）
        let prefix = if no_color || color.is_none() {
            if left_align {
                format!("{:<width$} = ", name, width = max_name_len)
            } else {
                format!("{:>width$} = ", name, width = max_name_len)
            }
        } else {
            let painted = paint(name, color.unwrap());
            let padded = if left_align {
                pad_left(&painted, max_name_len)
            } else {
                pad_right(&painted, max_name_len)
            };
            format!("{} = ", padded)
        };

        // 续行缩进
        let indent = " ".repeat(prefix_indent);

        // 计算可用宽度
        let avail = if term_w > prefix_indent { term_w - prefix_indent } else { 60 };

        // 分行输出
        if semicolon && value.contains(';') {
            // 按分号逐段显示
            let segments: Vec<&str> = value.split(';').filter(|s| !s.is_empty()).collect();
            for (i, seg) in segments.iter().enumerate() {
                let line = if i == segments.len() - 1 { seg.to_string() } else { format!("{};", seg) };
                if i == 0 {
                    println!("{}{}", prefix, line);
                } else {
                    println!("{}{}", indent, line);
                }
            }
        } else {
            let lines = wrap_value(value, avail);
            for (i, line) in lines.iter().enumerate() {
                if i == 0 {
                    println!("{}{}", prefix, line);
                } else {
                    println!("{}{}", indent, line);
                }
            }
        }
    }
}

/// 将长值按可用宽度分行，优先在 `;` 处断开
fn wrap_value(value: &str, avail: usize) -> Vec<String> {
    if value.display_width() <= avail {
        return vec![value.to_string()];
    }

    let mut result = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();

    while pos < len {
        // 计算从 pos 开始最远能到哪
        let end = if pos + avail >= len {
            len
        } else {
            // 在 [pos, pos+avail) 范围内找最后一个 `;`
            let search_end = (pos + avail).min(len);
            let mut break_at = search_end;
            // 从后往前找 `;`
            for i in (pos..search_end).rev() {
                if chars[i] == ';' {
                    break_at = i + 1; // 包括分号
                    break;
                }
            }
            // 如果没有分号，找空格
            if break_at == search_end {
                for i in (pos..search_end).rev() {
                    if chars[i] == ' ' || chars[i] == ',' {
                        break_at = i + 1;
                        break;
                    }
                }
            }
            break_at
        };

        let slice: String = chars[pos..end].iter().collect();
        result.push(slice);
        pos = end;
    }

    result
}

/// 获取终端宽度（列数），失败默认 120
fn terminal_width() -> u16 {
    #[repr(C)]
    struct ConsoleScreenBufferInfo {
        dw_size: [u16; 2],
        dw_cursor: [u16; 2],
        w_attrs: u16,
        sr_window: [u16; 4],
        dw_max: [u16; 2],
    }

    unsafe extern "system" {
        fn GetStdHandle(id: u32) -> isize;
        fn GetConsoleScreenBufferInfo(h: isize, info: *mut ConsoleScreenBufferInfo) -> i32;
    }

    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5u32;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == -1 || handle == 0 {
            return 120;
        }
        let mut info: ConsoleScreenBufferInfo = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
            (info.sr_window[2] - info.sr_window[0] + 1).max(40)
        } else {
            120
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
    println!("    {}        {}", green("e"), gray("显示帮助信息（默认）"));
    println!("    {} [{}]", green("e"), gray("路径"));
    println!("    {} {} {}", green("e"), cyan("set"),   gray("显示所有环境变量"));
    println!("    {} {} {}", green("e"), cyan("path"),  gray("显示 PATH"));
    println!("    {} {}", green("e -g"),                gray("打开环境变量对话框"));
    println!();
    println!("  {}", bold_yellow("子命令:"));

    let cmds: &[(&str, &str)] = &[
        ("set",  "显示所有环境变量（带颜色）"),
        ("path", "显示 PATH（带颜色）"),
    ];

    let max_w = cmds.iter().map(|(c, _)| c.display_width()).max().unwrap_or(6);

    for (cmd, desc) in cmds {
        println!("    {}  {}",
            pad_left(&cyan(cmd), max_w),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("选项:"));
    let opts: &[(&str, &str)] = &[
        ("-h, --help",      "显示简洁帮助"),
        ("-e, --examples",  "显示所有选项示例"),
        ("-v, --version",   "显示版本信息"),
        ("-n, --no-color",  "不使用颜色输出"),
        ("-g, --gui",       "打开环境变量对话框"),
        ("-o, --open <路径>", "在资源管理器打开路径"),
    ];

    let max_opt_w = opts.iter().map(|(o, _)| o.display_width()).max().unwrap_or(22);

    for (opt, desc) in opts {
        println!("  {}  {}",
            pad_left(&cyan(opt), max_opt_w),
            gray(desc));
    }
    println!();

    println!("  {}  {}  {}",
        gray("提示:"),
        gray("查看子命令选项请使用"),
        cyan("e <子命令> -h"));
    println!("  {}  {}  {}",
        gray("提示:"),
        gray("查看完整示例请使用"),
        cyan("e -e"));
}

/// 显示子命令帮助
fn print_subcommand_help(cmd: &str) {
    match cmd {
        "set" => {
            println!("  {} — {}", bold_cyan("e set"), green("显示所有环境变量"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {}", green("e set"), gray("[选项]"));
            println!();
            println!("  {}", bold_yellow("选项:"));
            println!("  {}  {}",
                pad_left(&cyan("-l, --left"), 16),
                gray("变量名左对齐"));
            println!("  {}  {}",
                pad_left(&cyan("-s, --semicolon"), 16),
                gray("按分号逐行显示"));
            println!("  {}  {}",
                pad_left(&cyan("-n, --no-color"), 16),
                gray("不使用颜色输出"));
        }
        "path" => {
            println!("  {} — {}", bold_cyan("e path"), green("显示 PATH"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {}", green("e path"), gray("[选项]"));
            println!();
            println!("  {}", bold_yellow("选项:"));
            println!("  {}  {}",
                pad_left(&cyan("-n, --no-color"), 16),
                gray("不使用颜色输出"));
        }
        _ => {}
    }
}

fn print_examples() {
    println!("  {}", bold_cyan("e — 环境变量查看器 / 路径打开器"));
    println!();
    println!("  {}", bold_yellow("打开路径"));

    let path_examples: &[(&str, &str)] = &[
        ("e", "显示帮助信息（默认）"),
        ("e .", "打开当前目录"),
        ("e C:\\path", "打开指定目录"),
        ("e C:\\file.txt", "打开父目录并选中文件"),
        ("e -o C:\\path", "用 -o 打开"),
    ];

    let max_w1 = path_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    for (cmd, desc) in path_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w1),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("环境变量 / PATH"));

    let env_examples: &[(&str, &str)] = &[
        ("e set", "显示所有环境变量"),
        ("e set -l", "左对齐变量名"),
        ("e set -s", "按分号逐行显示"),
        ("e path", "显示 PATH"),
        ("e path -n", "不使用颜色输出"),
        ("e -g", "打开环境变量对话框"),
    ];

    let max_w2 = env_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    for (cmd, desc) in env_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w2),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("配置文件"));
    println!("  {}",
        gray("e.yaml   放在 e.exe 同级，覆盖默认配色"));
    println!();
}
