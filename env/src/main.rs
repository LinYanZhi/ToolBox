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

    /// 打开 Windows 环境变量对话框（user / admin）
    #[arg(short = 'g', long = "gui", default_missing_value = "user", num_args = 0..=1, value_name = "模式")]
    gui: Option<String>,

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
    /// 管理环境配置 tag
    Tag {
        /// 在资源管理器中打开 tags 目录
        #[arg(short = 'o', long = "open")]
        open: bool,
        #[command(subcommand)]
        action: Option<TagCmd>,
    },
    /// 组合多个 tag 生成 cmd 脚本
    Gen {
        /// 复制到剪贴板
        #[arg(short = 'c', long = "copy")]
        copy: bool,
        /// tag 名称（至少一个）
        names: Vec<String>,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 显示/打开配置目录（%LOCALAPPDATA%\e\）
    Config {
        /// 在资源管理器中打开
        #[arg(short = 'o', long = "open")]
        open: bool,
        /// 清除配置文件，恢复默认配色
        #[arg(long = "clear", conflicts_with = "open")]
        clear: bool,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
}

#[derive(Subcommand)]
enum TagCmd {
    /// 列出所有 tag
    List {
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 创建一个新的 tag
    Create {
        /// tag 名称
        name: String,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 删除一个 tag
    Remove {
        /// tag 名称
        name: String,
        /// 显示该子命令帮助
        #[arg(short = 'h', long = "help")]
        help: bool,
    },
    /// 编辑 tag（在记事本中打开 YAML 文件）
    Edit {
        /// tag 名称
        name: String,
    },
    /// 为 tag 添加 PATH 目录
    #[command(name = "path-add")]
    PathAdd {
        /// tag 名称
        tag: String,
        /// 要添加的路径
        path: String,
    },
    /// 从 tag 移除 PATH 目录（按索引）
    #[command(name = "path-remove")]
    PathRemove {
        /// tag 名称
        tag: String,
        /// 路径索引（从 0 开始）
        index: usize,
    },
    /// 设置（新增或修改）tag 的环境变量
    #[command(name = "var-set")]
    VarSet {
        /// tag 名称
        tag: String,
        /// 变量名
        name: String,
        /// 变量值
        value: String,
    },
    /// 从 tag 移除环境变量
    #[command(name = "var-remove")]
    VarRemove {
        /// tag 名称
        tag: String,
        /// 变量名
        name: String,
    },
    /// 设置 tag 的 PROMPT（留空则清除）
    #[command(name = "prompt")]
    Prompt {
        /// tag 名称
        tag: String,
        /// PROMPT 值，留空则清除
        value: Option<String>,
    },
    /// 为 tag 添加别名
    #[command(name = "alias-add")]
    AliasAdd {
        /// tag 名称
        tag: String,
        /// 别名
        alias: String,
    },
    /// 从 tag 移除别名
    #[command(name = "alias-remove")]
    AliasRemove {
        /// tag 名称
        tag: String,
        /// 别名
        alias: String,
    },
}

fn main() {
    color::ansi::enable_ansi();

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
        Some(Commands::Tag { open: true, action: None }) => {
            cmd_tag_open_dir();
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::List { help: false }), .. }) => {
            activate::print_tag_list();
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::List { help: true, .. }), .. }) => {
            print_subcommand_help("tag");
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Create { name, help: false }), .. }) => {
            cmd_tag_create(&name);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Create { help: true, .. }), .. })
        | Some(Commands::Tag { open: false, action: None }) => {
            print_subcommand_help("tag");
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Remove { name, help: false }), .. }) => {
            cmd_tag_remove(&name);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Remove { help: true, .. }), .. }) => {
            print_subcommand_help("tag");
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Edit { name }), .. }) => {
            cmd_tag_edit(&name);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::PathAdd { tag, path }), .. }) => {
            cmd_tag_path_add(&tag, &path);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::PathRemove { tag, index }), .. }) => {
            cmd_tag_path_remove(&tag, index);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::VarSet { tag, name, value }), .. }) => {
            cmd_tag_var_set(&tag, &name, &value);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::VarRemove { tag, name }), .. }) => {
            cmd_tag_var_remove(&tag, &name);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::Prompt { tag, value }), .. }) => {
            cmd_tag_prompt(&tag, value.as_deref());
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::AliasAdd { tag, alias }), .. }) => {
            cmd_tag_alias_add(&tag, &alias);
            return;
        }
        Some(Commands::Tag { action: Some(TagCmd::AliasRemove { tag, alias }), .. }) => {
            cmd_tag_alias_remove(&tag, &alias);
            return;
        }
        Some(Commands::Gen { help: true, .. }) => {
            print_subcommand_help("gen");
            return;
        }
        Some(Commands::Gen { names, copy, help: false }) => {
            cmd_gen(&names, copy);
            return;
        }
        Some(Commands::Config { open: true, clear: false, help: false }) => {
            cmd_config(true);
            return;
        }
        Some(Commands::Config { open: false, clear: true, help: false }) => {
            cmd_config_clear();
            return;
        }
        Some(Commands::Config { open: _, clear: _, help: true }) => {
            print_subcommand_help("config");
            return;
        }
        Some(Commands::Config { open: false, clear: false, help: false }) => {
            cmd_config(false);
            return;
        }
        // open 和 clear 有 conflicts_with，实际不会到达，但编译器需要全覆盖
        Some(Commands::Config { .. }) => {
            cmd_config(false);
            return;
        }
        None => {
            // 用户可能打了旧的 e list，重定向
            if cli.path.as_deref() == Some("list") {
                println!("{} e list 已改为 e tag list", yellow("注意:"));
                println!("{} 请使用: {}", gray(""), cyan("e tag list"));
                return;
            }
        }
    }

    // --gui: 环境变量对话框
    if let Some(mode) = &cli.gui {
        match mode.as_str() {
            "admin" | "root" => open_env_dialog_admin(),
            _ => open_env_dialog(),
        }
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

/// 打开 Windows 环境变量对话框（用户模式）
fn open_env_dialog() {
    let _ = Command::new("rundll32.exe")
        .args(["sysdm.cpl,EditEnvironmentVariables"])
        .spawn();
}

/// 打开 Windows 环境变量对话框（管理员模式）
fn open_env_dialog_admin() {
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Start-Process rundll32.exe -ArgumentList 'sysdm.cpl,EditEnvironmentVariables' -Verb RunAs"])
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

// ── Tag 管理 ──────────────────────────────────

fn cmd_tag_create(name: &str) {
    match activate::create_tag(name) {
        Ok(path) => {
            println!("{} tag '{}' 已创建", green("✓"), cyan(name));
            println!("{} 编辑文件添加 PATH 目录和环境变量:", gray("请"));
            println!("  {}", cyan(path.to_string_lossy()));
        }
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

fn cmd_tag_remove(name: &str) {
    match activate::remove_tag(name) {
        Ok(()) => println!("{} tag '{}' 已删除", green("✓"), cyan(name)),
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

fn cmd_tag_open_dir() {
    let dir = activate::tags_dir();
    let _ = std::process::Command::new("explorer")
        .arg(&*dir.to_string_lossy())
        .spawn();
}

/// 在记事本中打开 tag YAML 文件编辑
fn cmd_tag_edit(name: &str) {
    match activate::resolve_tag(name) {
        Some((canonical, _)) => {
            let path = activate::tags_dir().join(format!("{}.yaml", canonical));
            let _ = std::process::Command::new("notepad")
                .arg(&*path.to_string_lossy())
                .spawn();
            println!("{} 已用记事本打开 tag '{}'", green("✓"), cyan(&canonical));
            println!("{} 保存后关闭记事本即可生效", gray("提示:"));
        }
        None => eprintln!("{} tag '{}' 不存在", red("错误:"), name),
    }
}

/// 为 tag 添加 PATH 目录
fn cmd_tag_path_add(tag: &str, path: &str) {
    match activate::tag_add_path(tag, path) {
        Ok(()) => println!("{} 已为 tag '{}' 添加路径: {}", green("✓"), cyan(tag), path),
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 从 tag 移除 PATH 目录
fn cmd_tag_path_remove(tag: &str, index: usize) {
    match activate::tag_remove_path(tag, index) {
        Ok(()) => println!("{} 已从 tag '{}' 移除索引为 [{}] 的路径", green("✓"), cyan(tag), index),
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 设置 tag 的环境变量
fn cmd_tag_var_set(tag: &str, name: &str, value: &str) {
    match activate::tag_set_var(tag, name, value) {
        Ok(()) => {}
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 移除 tag 的环境变量
fn cmd_tag_var_remove(tag: &str, name: &str) {
    match activate::tag_remove_var(tag, name) {
        Ok(()) => {}
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 设置或清除 tag 的 PROMPT
fn cmd_tag_prompt(tag: &str, value: Option<&str>) {
    match value {
        Some(v) if !v.is_empty() => match activate::tag_set_prompt(tag, v) {
            Ok(()) => {}
            Err(e) => eprintln!("{} {}", red("错误:"), e),
        },
        _ => match activate::tag_clear_prompt(tag) {
            Ok(()) => {}
            Err(e) => eprintln!("{} {}", red("错误:"), e),
        },
    }
}

/// 为 tag 添加别名
fn cmd_tag_alias_add(tag: &str, alias: &str) {
    match activate::tag_add_alias(tag, alias) {
        Ok(()) => {}
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 从 tag 移除别名
fn cmd_tag_alias_remove(tag: &str, alias: &str) {
    match activate::tag_remove_alias(tag, alias) {
        Ok(()) => {}
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

// ── 脚本生成 ──────────────────────────────────

fn cmd_gen(names: &[String], copy: bool) {
    if names.is_empty() {
        eprintln!("{} 请指定至少一个 tag", yellow("注意:"));
        eprintln!("{} e gen python git mysql", gray("示例:"));
        return;
    }

    match activate::generate_script(names) {
        Ok(script) => {
            if copy {
                // 用 clip.exe 写入剪贴板（Windows 内置，稳定可靠）
                let mut child = std::process::Command::new("clip.exe")
                    .stdin(std::process::Stdio::piped())
                    .spawn();
                match child.as_mut() {
                    Ok(c) => {
                        use std::io::Write;
                        let wrote = c.stdin.take()
                            .map(|mut s| s.write_all(script.as_bytes()).is_ok())
                            .unwrap_or(false);
                        if !wrote {
                            eprintln!("{} 剪贴板写入失败，直接输出脚本:", yellow("注意:"));
                            println!("{}", script);
                            return;
                        }
                        let _ = c.wait();
                        println!("{} 命令已复制到剪贴板", green("✓"));
                        println!("{} 按 Ctrl+V 粘贴到终端执行", gray("提示:"));
                    }
                    Err(_) => {
                        eprintln!("{} 剪贴板复制失败，直接输出脚本:", yellow("注意:"));
                        println!("{}", script);
                    }
                }
            } else {
                println!("{}", script);
            }
        }
        Err(e) => eprintln!("{} {}", red("错误:"), e),
    }
}

/// 显示/打开配置目录
fn cmd_config(open: bool) {
    // 确保配置文件存在（首次自动创建默认配置）
    let cfg_file = config::ensure_config();
    let cfg_dir = cfg_file.parent().unwrap_or(&cfg_file).to_path_buf();

    if open {
        // 确保目录存在
        let _ = std::fs::create_dir_all(&cfg_dir);
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
    println!();
    println!("  {}  {}", gray("打开目录:"), cyan("e config -o"));
    println!("  {}  {}", gray("清除配置:"), cyan("e config --clear"));
}

/// 清除配置文件，恢复默认配色
fn cmd_config_clear() {
    if config::clear_config() {
        println!("{} 配色配置已清除，下次运行将自动恢复为默认配置", green("✓"));
    } else {
        println!("{} 当前没有配置文件，无需清除", gray("•"));
    }
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
    println!("  {} — {}", bold_cyan("e"), green("环境变量管理 + 路径打开器"));
    println!();
    println!("  {}", bold_yellow("用法:"));
    println!("    {} {} {}", cyan("e"), green("<命令>"), gray("[参数]"));
    println!();
    println!("  {}", bold_yellow("命令:"));

    let cmds: &[(&str, &str)] = &[
        ("set",       "显示所有环境变量（带颜色）"),
        ("path",      "显示 PATH（带颜色）"),
        ("tag",       "管理 tag（list/create/remove；-o 打开目录）"),
        ("gen",       "组合 tag 生成 cmd 脚本"),
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
        ("-g, --gui [模式]", "打开环境变量对话框（user / admin / root，默认 user）"),
        ("-o, --open <路径>", "在资源管理器打开路径"),
        ("-e, --examples",  "显示所有示例"),
        ("-n, --no-color",  "不使用颜色输出"),
        ("-h, --help",      "显示帮助"),
        ("-v, --version",   "显示版本"),
    ];

    let max_opt_w = opts.iter().map(|(o, _)| o.display_width()).max().unwrap_or(24);

    for (opt, desc) in opts {
        println!("  {}  {}",
            pad_left(&cyan(opt), max_opt_w),
            gray(desc));
    }
    println!();

    println!("  {}  {}  {}",
        gray("提示:"),
        gray("子命令帮助:"),
        cyan("e <命令> -h"));
    println!("  {}  {}  {}",
        gray("提示:"),
        gray("完整示例:"),
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
        "activate" | "deactivate" => {
            println!("  {} — {}", bold_cyan(format!("e {}", cmd)), green(""));
            println!();
            println!("  {} 此命令已移除", bold_yellow("说明:"));
            println!("  {} 请使用 e tag create <名称> 创建 tag，然后用 e gen 生成脚本", gray(""));
            println!("  {} 生成的脚本仅适用于 cmd.exe，请勿在 PowerShell 中使用", gray(""));
        }
        "tag" => {
            println!("  {} — {}", bold_cyan("e tag"), green("管理环境配置 tag"));
            println!();
            println!("  {}", bold_yellow("子命令:"));
            println!("    {}  {}", pad_left(&cyan("list"), 28), gray("列出所有 tag"));
            println!("    {}  {}", pad_left(&cyan("create <名称>"), 28), gray("创建一个新的 tag"));
            println!("    {}  {}", pad_left(&cyan("remove <名称>"), 28), gray("删除一个 tag"));
            println!("    {}  {}", pad_left(&cyan("edit <名称>"), 28), gray("在记事本中打开 tag 文件编辑"));
            println!("    {}  {}", pad_left(&cyan("path-add <tag> <路径>"), 28), gray("添加 PATH 目录"));
            println!("    {}  {}", pad_left(&cyan("path-remove <tag> <索引>"), 28), gray("按索引移除 PATH 目录"));
            println!("    {}  {}", pad_left(&cyan("var-set <tag> <名称> <值>"), 28), gray("设置环境变量（新增或修改）"));
            println!("    {}  {}", pad_left(&cyan("var-remove <tag> <名称>"), 28), gray("移除环境变量"));
            println!("    {}  {}", pad_left(&cyan("prompt <tag> [值]"), 28), gray("设置 PROMPT（留空则清除）"));
            println!("    {}  {}", pad_left(&cyan("alias-add <tag> <别名>"), 28), gray("添加别名"));
            println!("    {}  {}", pad_left(&cyan("alias-remove <tag> <别名>"), 28), gray("移除别名"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 每个 tag 是一个 yaml 文件，可配置：", gray(""));
            println!("    {}  path: PATH 目录列表", gray("•"));
            println!("    {}  var:  环境变量", gray("•"));
            println!("    {}  prompt: PROMPT 覆盖（可选）", gray("•"));
            println!("    {}  aliases: 别名列表", gray("•"));
            println!();
            println!("  {}", bold_yellow("示例:"));
            let ex_cmds: &[&str] = &[
                "e tag create python",
                "e tag path-add python <路径>",
                "e tag var-set python <名称> <值>",
                "e tag path-remove python <索引>",
                "e tag var-remove python <名称>",
                "e tag alias-add python <别名>",
                "e tag prompt python <值>",
            ];
            let ex_w = ex_cmds.iter().map(|s| s.display_width()).max().unwrap_or(40);
            for cmd in ex_cmds {
                let colored = cmd.replace("<路径>", "\x1b[36m<路径>\x1b[0m")
                    .replace("<名称>", "\x1b[36m<名称>\x1b[0m")
                    .replace("<值>", "\x1b[36m<值>\x1b[0m")
                    .replace("<索引>", "\x1b[36m<索引>\x1b[0m")
                    .replace("<别名>", "\x1b[36m<别名>\x1b[0m");
                println!("    {}  {}", pad_left(&colored, ex_w), gray(""));
            }
            println!();
            println!("  {}", gray(format!("目录: %LOCALAPPDATA%\\e\\tags\\")));
        }
        "gen" => {
            println!("  {} — {}", bold_cyan("e gen"), green("组合多个 tag 生成 cmd 脚本"));
            println!();
            println!("  {}", bold_yellow("用法:"));
            println!("    {} {} {} {}  {}", green("e"), cyan("gen"), gray("<tag1>"), gray("<tag2>"), gray("..."));
            println!("    {} {} {} {} {}  {}", green("e"), cyan("gen"), cyan("--copy"), gray("<tag1>"), gray("<tag2>"), gray("..."));
            println!();
            println!("  {}", bold_yellow("选项:"));
            println!("  {}  {}", pad_left(&cyan("-c, --copy"), 16), gray("复制到剪贴板而非输出到终端"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 合并指定 tag 的配置，生成 cmd 脚本", gray(""));
            println!("  {} 生成的脚本仅适用于 cmd.exe，请勿在 PowerShell 中执行", gray(""));
            println!("  {} 脚本包含 PATH 追加、环境变量设置", gray(""));
            println!("  {} tag 顺序决定优先级：后列出的优先", gray(""));
            println!();
            println!("  {}", bold_yellow("示例:"));
            println!("    {}  {}", gray("e gen python git"), gray(""));
            println!("    {}  {}", gray("e gen --copy python"), gray(""));
            println!("    {}  {}", gray("e gen python git mysql"), gray(""));
            println!("    {}  {}", gray("复制后直接粘贴到终端执行"), gray(""));
        }
        "list" => {
            println!("  {} — {}", bold_cyan("e list"), green("列出所有 tag"));
            println!();
            println!("  {} 此命令已改为 e tag list", bold_yellow("注意:"));
            println!("  {} 请使用: {}", gray(""), cyan("e tag list"));
            println!();
            println!("  {}", bold_yellow("说明:"));
            println!("  {} 显示 %LOCALAPPDATA%\\e\\tags\\ 下的所有 tag", gray(""));
            println!("  {} 每个 tag 包含 PATH 目录列表和环境变量", gray(""));
            println!("  {} 使用 e gen 组合多个 tag 生成脚本", gray(""));
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
            println!("  {} tag 目录: {}  {}", gray("•"), cyan("tags\\"), gray(""));
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
        ("e -g", "打开环境变量对话框（用户）"),
        ("e -g admin", "以管理员打开环境变量对话框"),
        ("e -g root", "以管理员打开环境变量对话框（同 admin）"),
    ];

    let max_w2 = env_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    for (cmd, desc) in env_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w2),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("Tag 管理"));

    let act_examples: &[(&str, &str)] = &[
        ("e tag list", "列出所有 tag"),
        ("e tag create python", "创建 python tag"),
        ("e tag remove python", "删除 python tag"),
    ];

    let max_w3 = act_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(26);

    for (cmd, desc) in act_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w3),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("脚本生成"));

    let gen_examples: &[(&str, &str)] = &[
        ("e gen python git", "组合 python + git 生成 cmd 脚本"),
        ("e gen --copy python", "生成并复制到剪贴板"),
        ("e gen python git mysql", "组合多个 tag"),
    ];

    let max_w4 = gen_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(30);

    for (cmd, desc) in gen_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w4),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("使用方式:"));
    println!("    {}  {}", gray("1."), gray("e gen python git --copy"));
    println!("    {}  {}", gray("2."), gray("Ctrl+V 粘贴到 cmd.exe 终端执行"));
    println!("    {}  {}", gray("注: 生成的脚本仅适用于 cmd.exe，不可用于 PowerShell"), gray(""));
    println!();

    println!("  {}", bold_yellow("配置文件:"));
    println!("  {}  {}", gray("tag 目录:"), cyan("%LOCALAPPDATA%\\e\\tags\\"));
    println!();

    println!("  {}", bold_yellow("配置文件"));
    println!("  {}",
        gray("e.yaml   放在 e.exe 同级，覆盖默认配色"));
    println!();
}
