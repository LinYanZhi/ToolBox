mod activate;
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
    /// 激活环境（输出 shell 脚本，请 eval 执行）
    Activate {
        /// 环境名称
        name: String,
        /// 输出 PowerShell 脚本（默认 cmd.exe）
        #[arg(long = "ps1")]
        ps1: bool,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 停用当前环境（输出恢复脚本）
    Deactivate {
        /// 输出 PowerShell 脚本（默认 cmd.exe）
        #[arg(long = "ps1")]
        ps1: bool,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 列出所有可用环境
    List {
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 创建新的环境定义（类 venv）
    Venv {
        /// 环境名称
        name: String,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 显示/打开配置目录（%LOCALAPPDATA%\e\）
    Config {
        /// 在资源管理器中打开
        #[arg(short = 'o', long = "open")]
        open: bool,
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
        Some(Commands::Activate { name: _, ps1: _, help: true }) => {
            print_subcommand_help("activate");
            return;
        }
        Some(Commands::Activate { name, ps1, help: false }) => {
            cmd_activate(&name, ps1);
            return;
        }
        Some(Commands::Deactivate { ps1: _, help: true }) => {
            print_subcommand_help("deactivate");
            return;
        }
        Some(Commands::Deactivate { ps1, help: false }) => {
            cmd_deactivate(ps1);
            return;
        }
        Some(Commands::List { help: true }) => {
            print_subcommand_help("list");
            return;
        }
        Some(Commands::List { help: false }) => {
            activate::print_env_list();
            return;
        }
        Some(Commands::Venv { name: _, help: true }) => {
            print_subcommand_help("venv");
            return;
        }
        Some(Commands::Venv { name, help: false }) => {
            cmd_venv(&name);
            return;
        }
        Some(Commands::Config { open: true, help: false }) => {
            cmd_config(true);
            return;
        }
        Some(Commands::Config { open: _, help: true }) => {
            print_subcommand_help("config");
            return;
        }
        Some(Commands::Config { open: false, help: false }) => {
            cmd_config(false);
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

// ── 环境激活/停用 ──────────────────────────────────

fn cmd_activate(name: &str, ps1: bool) {
    let def = match activate::load_env(name) {
        Some(d) => d,
        None => {
            eprintln!("{} 环境 '{}' 未找到", red("错误:"), name);
            eprintln!("{} 使用 e list 查看可用环境", gray("提示:"));
            return;
        }
    };

    if ps1 {
        activate::print_activate_ps1(&def, name);
    } else {
        activate::print_activate_cmd(&def, name);
    }
}

fn cmd_deactivate(ps1: bool) {
    if ps1 {
        activate::print_deactivate_ps1();
    } else {
        activate::print_deactivate_cmd();
    }
}

/// 创建新环境
fn cmd_venv(name: &str) {
    match activate::create_env(name) {
        Ok(path) => {
            println!("{} 环境 '{}' 已创建", green("✓"), cyan(name));
            println!("{} 编辑文件以配置环境变量:", gray("请"));
            println!("  {}", cyan(path.display().to_string()));
        }
        Err(e) => {
            eprintln!("{} {}", red("错误:"), e);
        }
    }
}

/// 显示/打开配置目录
fn cmd_config(open: bool) {
    let cfg_dir = config::get_config_dir();
    let cfg_file = cfg_dir.join("e.yaml");
    let envs_dir = cfg_dir.join("envs");

    if open {
        // 确保目录存在
        let _ = std::fs::create_dir_all(&cfg_dir);
        let _ = std::fs::create_dir_all(&envs_dir);
        let _ = std::process::Command::new("explorer")
            .arg(&*cfg_dir.to_string_lossy())
            .spawn();
        return;
    }

    println!("  {}", bold_cyan("e 配置目录:"));
    println!("    {}", cyan(&*cfg_dir.to_string_lossy()));
    println!();
    println!("  {}", bold_yellow("文件:"));
    println!("    {}    {}", pad_left(&cyan("配色配置"), 16), gray(&*cfg_file.to_string_lossy()));
    println!("    {}    {}", pad_left(&cyan("环境定义"), 16), gray(&*envs_dir.to_string_lossy()));
    println!();
    println!("  {}  {}", gray("打开目录:"), cyan("e config -o"));
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
    println!("    {} {} {}", green("e"), cyan("set"),         gray("显示所有环境变量"));
    println!("    {} {} {}", green("e"), cyan("path"),        gray("显示 PATH"));
    println!("    {} {} {}", green("e"), cyan("activate"),    gray("激活环境"));
    println!("    {} {} {}", green("e"), cyan("deactivate"),  gray("恢复环境"));
    println!("    {} {}", green("e -g"),                      gray("打开环境变量对话框"));
    println!();
    println!("  {}", bold_yellow("子命令:"));

    let cmds: &[(&str, &str)] = &[
        ("set",       "显示所有环境变量（带颜色）"),
        ("path",      "显示 PATH（带颜色）"),
        ("activate",  "激活环境（输出 shell 脚本）"),
        ("deactivate","恢复环境（输出恢复脚本）"),
        ("list",      "列出可用环境"),
        ("venv",      "创建新环境定义"),
        ("config",    "显示/打开配置目录"),
    ];

    let max_w = cmds.iter().map(|(c, _)| c.display_width()).max().unwrap_or(12);

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
        "activate" => {
            println!("  {} — {}", bold_cyan("e activate"), green("激活环境"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {} {}  {}", green("e"), cyan("activate"), gray("<环境名>"), gray("(输出 cmd.exe 脚本)"));
            println!("    {} {} {} {}  {}", green("e"), cyan("activate"), gray("<环境名>"), cyan("--ps1"), gray("(输出 PowerShell 脚本)"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 输出 shell 脚本到 stdout，请在终端中 eval 执行", gray(""));
            println!("  {} PowerShell: {}  {}", gray("•"), cyan("e activate home | iex"), gray(""));
            println!("  {} Cmd: {}  {}", gray("•"), cyan("e activate home > %TEMP%\\act.bat && call %TEMP%\\act.bat"), gray(""));
        }
        "deactivate" => {
            println!("  {} — {}", bold_cyan("e deactivate"), green("恢复环境"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {}  {}", green("e"), cyan("deactivate"), gray("(输出 cmd.exe 恢复脚本)"));
            println!("    {} {} {}  {}", green("e"), cyan("deactivate"), cyan("--ps1"), gray("(输出 PowerShell 恢复脚本)"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 恢复到 activate 前的环境状态", "");
            println!("  {} PowerShell: {}  {}", gray("•"), cyan("e deactivate | iex"), gray(""));
            println!("  {} Cmd: {}  {}", gray("•"), cyan("e deactivate > %TEMP%\\deact.bat && call %TEMP%\\deact.bat"), gray(""));
        }
        "list" => {
            println!("  {} — {}", bold_cyan("e list"), green("列出可用环境"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {}", green("e list"), gray(""));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 环境定义文件放在 e.exe 同级的 envs/ 目录下", gray(""));
            println!("  {} 格式: {}  {}", gray("•"), gray("<名称>.yaml"), gray(""));
        }
        "venv" => {
            println!("  {} — {}", bold_cyan("e venv"), green("创建新环境定义"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {} {}", green("e"), cyan("venv"), gray("<环境名>"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 创建一个环境定义文件（YAML），编辑后可配置", gray(""));
            println!("  {} 变量、PROMPT、PATH 前插路径", gray(""));
            println!("  {} 创建后使用 e activate <环境名> 激活", gray(""));
        }
        "config" => {
            println!("  {} — {}", bold_cyan("e config"), green("显示/打开配置目录"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {}  {}", green("e"), cyan("config"), gray("(显示配置目录路径)"));
            println!("    {} {} {}  {}", green("e"), cyan("config"), cyan("-o"), gray("(在资源管理器中打开)"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 配置目录: {}  {}", gray("•"), cyan("%LOCALAPPDATA%\\e\\"), gray(""));
            println!("  {} 配色配置: {}  {}", gray("•"), cyan("e.yaml"), gray(""));
            println!("  {} 环境定义: {}  {}", gray("•"), cyan("envs\\"), gray(""));
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

    println!("  {}", bold_yellow("环境管理（类 venv）"));

    let act_examples: &[(&str, &str)] = &[
        ("e list", "列出可用环境"),
        ("e activate home", "激活 home 环境（输出 cmd 脚本）"),
        ("e activate home --ps1", "激活 home 环境（输出 PowerShell 脚本）"),
        ("e deactivate", "恢复环境（输出 cmd 脚本）"),
        ("e deactivate --ps1", "恢复环境（输出 PowerShell 脚本）"),
    ];

    let max_w3 = act_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(34);

    for (cmd, desc) in act_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w3),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("使用方式:"));
    println!("  {}  {}  {}", gray("PowerShell:"), cyan("e activate home | iex"), gray(""));
    println!("  {}  {}  {}", gray("Cmd:"), cyan("e activate home > %TEMP%\\act.bat && call %TEMP%\\act.bat"), gray(""));
    println!();

    println!("  {}", bold_yellow("创建新环境 (venv):"));

    let venv_examples: &[(&str, &str)] = &[
        ("e venv myenv", "创建名为 myenv 的环境定义"),
    ];

    let max_w4 = venv_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(18);

    for (cmd, desc) in venv_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w4),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("文件存储:"));
    println!("  {}  {}", gray("配置/环境定义:"), cyan("%LOCALAPPDATA%\\e\\"));
    println!("  {}  {}", gray("状态快照:"), cyan("%TEMP%\\e-state-<PID>.json（每个会话独立）"));
    println!();

    println!("  {}", bold_yellow("配置文件"));
    println!("  {}",
        gray("e.yaml   放在 e.exe 同级，覆盖默认配色"));
    println!();
}
