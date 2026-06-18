mod config;
mod display;
mod links;
mod scanner;

use std::path::{Path, PathBuf};

use clap::{Parser, CommandFactory, builder::styling};
use color::*;
use display::Formatter;
use scanner::ItemInfo;

fn styles() -> styling::Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default().bold())
        .usage(styling::AnsiColor::Green.on_default().bold())
        .literal(styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(styling::AnsiColor::Yellow.on_default().italic())
        .error(styling::AnsiColor::Red.on_default().bold())
        .valid(styling::AnsiColor::Cyan.on_default().bold())
        .invalid(styling::AnsiColor::Yellow.on_default())
}

#[derive(Parser)]
#[command(
    name = "ls",
    version = version_str(),
    about,
    styles = styles(),
    color = clap::ColorChoice::Always,
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Cli {
    /// 要列出的目录路径（默认为当前目录）
    #[arg(default_value = ".")]
    directory: String,

    /// 不使用颜色输出
    #[arg(short = 'n', long = "no-color")]
    no_color: bool,

    /// 排序方式: default(目录优先+名称), name, suffix(扩展名), size, create(创建时间), update(修改时间)
    #[arg(short = 's', long = "sort", default_value = "default", value_parser = ["default", "name", "suffix", "size", "create", "update"])]
    sort: String,

    /// 按文件大小排序（从大到小，等同于 -s size）
    #[arg(short = 'S', long = "size-sort")]
    size_sort: bool,

    /// 排除指定后缀（如 .txt .md）
    #[arg(long = "exclude", num_args = 1..)]
    exclude: Vec<String>,

    /// 只包含指定后缀（如 .rs .py）
    #[arg(short = 'i', long = "include", num_args = 1..)]
    include: Vec<String>,

    /// 只显示文件
    #[arg(short = 'f', long = "only-files")]
    only_files: bool,

    /// 只显示目录
    #[arg(short = 'd', long = "only-dirs")]
    only_dirs: bool,

    /// 显示完整路径
    #[arg(short = 'a', long = "abs-path")]
    abs_path: bool,

    /// 右对齐文件名
    #[arg(short = 'r', long = "right-align")]
    right_align: bool,

    /// 显示文件大小
    #[arg(short = 'z', long = "size")]
    size: bool,

    /// 树形显示（如 -t、-t 3、-t 路径）
    #[arg(short = 't', long = "tree", default_missing_value = "", num_args = 0..=1)]
    tree: Option<String>,

    /// 检查链接信息
    #[arg(long = "link")]
    link: Option<String>,

    /// 长格式输出（每行一个，显示详细信息）
    #[arg(short = 'l', long = "long")]
    long: bool,

    /// 递归列出子目录内容
    #[arg(short = 'R', long = "recursive")]
    recursive: bool,

    /// 显示帮助信息
    #[arg(short = 'h', long = "help", global = true)]
    help: bool,

    /// 显示所有选项示例
    #[arg(short = 'e', long = "examples")]
    examples: bool,

    /// 显示版本号
    #[arg(short = 'v', long = "version", global = true)]
    version: bool,
}

const fn version_str() -> &'static str {
    concat!(env!("CARGO_PKG_VERSION"), " (Rust版)")
}

fn version_info() -> String {
    format!(
        "{} {} ({})
{}
---
{}",
        bold_cyan("ls"),
        green(env!("CARGO_PKG_VERSION")),
        gray("Rust 版"),
        yellow("一个轻量级的目录列表工具"),
        format!("{}: {}", blue("GitHub"), Style::new(4).paint("https://github.com/LinYanZhi/ToolBox")),
    )
}

/// 解析树形参数：返回 (depth, path)
///
/// - `ls -t` → (-1, 用户目录)
/// - `ls -t 3` → (3, 用户目录)
/// - `ls -t 3 路径` → (3, 路径)
/// - `ls -t 路径` → (-1, 路径)  ← 自动识别非数字为路径
fn parse_tree_opt(tree: &Option<String>, dir: &str) -> (i32, String) {
    match tree.as_ref() {
        Some(s) if s.is_empty() => (-1, dir.to_string()),
        Some(s) => {
            // 先试解析为数字（深度）
            if let Ok(n) = s.parse::<i32>() {
                (n, dir.to_string())
            } else {
                // 不是数字，当作路径
                (-1, s.to_string())
            }
        }
        _ => (-1, dir.to_string()),
    }
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            print_clap_error(&e);
            return;
        }
    };

    if cli.help {
        let cmd = <Cli as CommandFactory>::command();
        cmd.next_help_heading("选项:").print_help().ok();
        println!();
        return;
    }

    if cli.examples {
        print_examples_help();
        return;
    }

    if cli.version {
        println!("{}", version_info());
        return;
    }

    // 加载配置
    config::ColorConfig::init();
    let color_config = config::ColorConfig::load();
    let formatter = Formatter::new(color_config, cli.no_color);

    if let Some(link_path) = &cli.link {
        handle_link_check(link_path);
        return;
    }

    // 清理路径：去除可能因 shell 转义残留的尾部引号、反斜杠、空白
    let dir_str = sanitize_path(&cli.directory);
    // 树形显示
    if cli.tree.is_some() {
        let (tree_depth, tree_path) = parse_tree_opt(&cli.tree, &dir_str);
        let target = Path::new(&tree_path);
        if !target.exists() {
            eprintln!("错误: 目录 '{}' 不存在", target.display());
            return;
        }
        if !target.is_dir() {
            eprintln!("错误: '{}' 不是一个目录", target.display());
            return;
        }
        print_tree(target, &formatter, &cli, "", tree_depth);
        return;
    }

    let target_dir = Path::new(&dir_str);
    if !target_dir.exists() {
        eprintln!("错误: 目录 '{}' 不存在（输入: {}）", target_dir.display(), cli.directory);
        return;
    }
    if !target_dir.is_dir() {
        eprintln!("错误: '{}' 不是一个目录（输入: {}）", target_dir.display(), cli.directory);
        return;
    }

    let mut items = scanner::scan_directory(target_dir);

    // 过滤
    if !cli.exclude.is_empty() {
        items.retain(|item| {
            !cli.exclude.iter().any(|ext| {
                item.name.to_lowercase().ends_with(&ext.to_lowercase())
            })
        });
    }

    if !cli.include.is_empty() {
        items.retain(|item| {
            if item.is_dir {
                return true;
            }
            cli.include.iter().any(|ext| {
                item.name.to_lowercase().ends_with(&ext.to_lowercase())
            })
        });
    }

    if cli.only_files {
        items.retain(|item| item.is_file);
    } else if cli.only_dirs {
        items.retain(|item| {
            item.is_dir
                || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction)
        });
    }

    // 排序（-S 优先级高于 -s）
    let sort_key = if cli.size_sort { "size" } else { &cli.sort };
    sort_items(&mut items, sort_key);

    // 递归显示
    if cli.recursive {
        print_recursive(target_dir, &formatter, &cli, sort_key);
        return;
    }

    // 输出
    if cli.abs_path || cli.long || cli.right_align {
        // 长格式、绝对路径或右对齐：每行一个
        let max_width = if cli.right_align {
            items.iter().map(|item| item.name.display_width()).max().unwrap_or(0)
        } else {
            0
        };

        let max_size_width = if cli.size {
            items.iter().map(|item| color::format_size(item.size).len()).max().unwrap_or(0)
        } else {
            0
        };

        for item in &items {
            let line = format_item_line(item, &formatter, &cli, max_width, max_size_width);
            println!("{}", line);
        }
    } else {
        // 多列输出
        print_multi_column(&items, &formatter, cli.no_color);
    }
}

/// 简洁帮助（仿 as -h 风格）
/// 全部示例（仿 as -e 风格）
fn print_examples_help() {
    println!("  {}", bold_cyan("ls — 轻量级目录列表工具"));
    println!();
    println!("  {}", bold_yellow("目录列表"));

    let examples: &[(&str, &str)] = &[
        ("ls",                  "列出当前目录（多列）"),
        ("ls PATH",             "列出指定目录"),
        ("ls -l",               "长格式（每行一个，显示详细信息）"),
        ("ls -l PATH",          "长格式指定目录"),
        ("ls -t",               "树形显示（无限深度）"),
        ("ls -t 3",            "树形显示（深度 3）"),
        ("ls -t 3 路径",        "树形显示指定目录"),
        ("ls -t 路径",          "树形显示（自动识别为路径）"),
        ("ls --exclude .txt .md", "排除指定后缀"),
        ("ls -i .rs .py",       "只包含指定后缀"),
        ("ls -a",               "显示完整路径"),
        ("ls -r",               "右对齐文件名"),
        ("ls -z",               "显示文件大小"),
        ("ls -f",               "只显示文件"),
        ("ls -d",               "只显示目录"),
        ("ls -s name",          "按名称排序"),
        ("ls -s suffix",        "按后缀排序"),
        ("ls -S",               "按文件大小排序（大→小）"),
        ("ls -R",               "递归列出子目录内容"),
        ("ls -R -l",            "递归列出（长格式）"),
        ("ls --link PATH",      "检查链接信息"),
        ("ls -n",               "不使用颜色输出"),
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(28);

    for (cmd, desc) in examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("环境变量/PATH（需安装 ss 工具）"));
    let env_examples: &[(&str, &str)] = &[
        ("ss", "显示所有环境变量"),
        ("ss -l", "左对齐变量名"),
        ("pp", "显示 PATH"),
        ("ss gui", "打开环境变量对话框"),
    ];

    let max_w2 = env_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(10);

    for (cmd, desc) in env_examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w2),
            gray(desc));
    }
    println!();
}

/// 拦截 clap 错误并输出中文提示
fn print_clap_error(err: &clap::error::Error) {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::UnknownArgument => {
            let raw = err.to_string();
            if let Some(flag) = raw.lines().find_map(|l| {
                let l = l.trim();
                l.strip_prefix("error: unexpected argument '")
                    .or(l.strip_prefix("error: unexpected argument \""))
                    .and_then(|s| s.split('\'').next())
                    .or(l.strip_prefix("error: unexpected argument \"")
                        .and_then(|s| s.split('"').next()))
            }) {
                eprintln!("{} 未知的选项 '{}'", red("错误:"), flag);
                eprintln!("{} 使用 --help 查看可用选项", gray("提示:"));
            } else {
                eprintln!("{} 未知的选项", red("错误:"));
                eprintln!("{} 使用 --help 查看可用选项", gray("提示:"));
            }
        }
        ErrorKind::MissingRequiredArgument => {
            eprintln!("{} 缺少必需参数", red("错误:"));
            eprintln!("{} 使用 --help 查看正确用法", gray("提示:"));
        }
        ErrorKind::ValueValidation => {
            let msg = err.to_string();
            if msg.contains("invalid value") {
                if let Some(val) = msg.split('\'').nth(1) {
                    if let Some(choices) = msg.split('[').nth(1).and_then(|s| s.split(']').next()) {
                        eprintln!("{} 无效的值 '{}'，可选值: {}", red("错误:"), val, choices);
                    } else {
                        eprintln!("{} 无效的值 '{}'", red("错误:"), val);
                    }
                } else {
                    eprintln!("{} 参数值无效", red("错误:"));
                }
            } else {
                eprintln!("{} {}", red("错误:"), msg.lines().next().unwrap_or("参数值无效"));
            }
            eprintln!("{} 使用 --help 查看正确用法", gray("提示:"));
        }
        _ => {
            let msg = err.to_string();
            let first_line = msg.lines().next().unwrap_or("未知错误");
            if first_line.contains("unexpected") || first_line.contains("error:") {
                eprintln!("{} 参数解析失败", red("错误:"));
            } else {
                eprintln!("{} {}", red("错误:"), first_line);
            }
            eprintln!("{} 使用 --help 查看正确用法", gray("提示:"));
        }
    }
}

/// 格式化单个项目行
fn format_item_line(
    item: &ItemInfo,
    formatter: &Formatter,
    cli: &Cli,
    max_width: usize,
    max_size_width: usize,
) -> String {
    let mut line = String::new();

    if cli.abs_path {
        let raw = std::fs::canonicalize(&item.path)
            .unwrap_or_else(|_| item.path.clone());
        let abs_path = raw.to_string_lossy()
            .replace("\\\\?\\", "")
            .replace("\\??\\", "");

        if cli.no_color {
            line.push_str(&abs_path);
            return line;
        }

        let path = std::path::Path::new(&abs_path);
        let parent = path.parent().and_then(|p| p.to_str());
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(&abs_path);

        let is_link = matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction);

        // 父路径灰显
        if let Some(parent) = parent {
            if !parent.is_empty() && parent != "." {
                line.push_str(&display::paint_by_code(&format!("{}\\", parent), "90"));
            }
        }

        if item.is_dir || is_link {
            // 目录/链接目录：目录名用目录色
            let color = formatter.get_item_color(item);
            match color {
                Some(c) => line.push_str(&display::paint_by_code(file_name, c)),
                None => line.push_str(file_name),
            }
        } else {
            // 文件：文件名白色，后缀用扩展名颜色
            let ext = std::path::Path::new(file_name)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            if ext.is_empty() {
                line.push_str(&display::paint_by_code(file_name, "97"));
            } else {
                let name_part = file_name.strip_suffix(&ext).unwrap_or(file_name);
                let ext_color = formatter.config.ext_color(&ext);
                line.push_str(&display::paint_by_code(name_part, "97"));
                match ext_color {
                    Some(ec) => line.push_str(&display::paint_by_code(&ext, ec)),
                    None => line.push_str(&ext),
                }
            }
        }

        // 链接目标
        if let Some(ref target) = item.link_target {
            line.push_str(&formatter.print_link_arrow(item.is_dir));
            line.push_str(&formatter.print_link_target(target, item.link_type == links::LinkType::Shortcut));
        }

        return line;
    }

    let is_link = matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction);
    let is_shortcut = item.link_type == links::LinkType::Shortcut;

    // 类型标记
    if item.is_dir || is_link {
        line.push_str(&formatter.print_type_marker("<dir>"));
    } else {
        line.push_str(&formatter.print_type_marker("<file>"));
    }

    // 时间戳
    let show_time = cli.sort == "create" || cli.sort == "update";
    if show_time {
        let secs = if cli.sort == "create" {
            item.create_time_secs().unwrap_or(0)
        } else {
            item.modify_time_secs().unwrap_or(0)
        };
        line.push_str(&formatter.print_timestamp(secs));
    }

    // 文件名
    line.push_str(&formatter.print_file_name(item, cli.right_align, max_width));

    // 文件大小
    if cli.size && !item.is_dir {
        line.push_str(&formatter.print_size(item, max_size_width));
    }

    // 链接目标
    if is_link || is_shortcut {
        if let Some(ref target) = item.link_target {
            line.push_str(&formatter.print_link_arrow(item.is_dir));
            line.push_str(&formatter.print_link_target(target, is_shortcut));
        }
    }

    // Python/Java/Node 环境版本
    if item.is_dir && !is_link {
        let name_lower = item.name.to_lowercase();
        let looks_like_python = name_lower == ".venv" || name_lower == "venv"
            || name_lower == "env" || name_lower == ".env"
            || name_lower.starts_with("python");
        let looks_like_java = name_lower.contains("jdk") || name_lower.contains("jre")
            || name_lower.contains("java");
        let looks_like_node = name_lower.contains("node");

        if looks_like_python {
            if let Some(ver) = item.get_python_env() {
                let text = if item.name == ".venv" {
                    format!(".venv {}", ver)
                } else {
                    ver
                };
                line.push_str(&format_text_colored(&text, cli));
            }
        } else if looks_like_java {
            if let Some(ver) = item.get_java_env() {
                line.push_str(&format_text_colored(&ver, cli));
            }
        } else if looks_like_node {
            if let Some(ver) = item.get_node_info() {
                line.push_str(&format_text_colored(&ver, cli));
            }
        }
    }

    // Git 项目检测
    if item.is_dir && !is_link {
        if let Some(info) = item.get_git_info() {
            line.push_str(&format_text_colored(&info, cli));
        }
    }

    line
}

/// 带引号的环境版本号输出
fn format_text_colored(text: &str, _cli: &Cli) -> String {
    let quoted = format!("\"{}\"", text);
    gray(&quoted)
}

// ── 多列输出 ──────────────────────────────────────────

/// 获取终端宽度（列数），失败时默认 80
fn terminal_width() -> usize {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct COORD { x: i16, y: i16 }
        #[repr(C)]
        struct SMALL_RECT { left: i16, top: i16, right: i16, bottom: i16 }
        #[repr(C)]
        struct CONSOLE_SCREEN_BUFFER_INFO {
            dw_size: COORD,
            dw_cursor_position: COORD,
            w_attributes: u16,
            sr_window: SMALL_RECT,
            dw_maximum_window_size: COORD,
        }
        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> isize;
            fn GetConsoleScreenBufferInfo(
                hConsoleOutput: isize,
                lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
            ) -> i32;
        }
        unsafe {
            const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5u32;
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle == -1 || handle == 0 {
                return 80;
            }
            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
                (info.sr_window.right - info.sr_window.left + 1) as usize
            } else {
                80
            }
        }
    }
    #[cfg(not(windows))]
    {
        std::env::var("COLUMNS").ok().and_then(|s| s.parse().ok()).unwrap_or(80)
    }
}

/// 生成多列模式下的条目显示名
fn format_item_name_multi(item: &ItemInfo, formatter: &Formatter) -> String {
    let name = &item.name;
    if formatter.no_color {
        if item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction) {
            name.to_string()
        } else {
            name.to_string()
        }
    } else {
        // 带颜色
        if item.is_dir || matches!(item.link_type, crate::links::LinkType::Symlink | crate::links::LinkType::Junction) {
            // 目录/链接目录：浅蓝色
            formatter.get_item_color(item)
                .map(|c| display::paint_by_code(name, c))
                .unwrap_or_else(|| name.to_string())
        } else if matches!(item.link_type, crate::links::LinkType::Shortcut) {
            formatter.get_item_color(item)
                .map(|c| display::paint_by_code(name, c))
                .unwrap_or_else(|| name.to_string())
        } else {
            // 文件：文件名白色，后缀用扩展名颜色
            let ext = std::path::Path::new(name)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            if ext.is_empty() {
                display::paint_by_code(name, "97")
            } else {
                let name_part = name.strip_suffix(&ext).unwrap_or(name);
                let ext_color = formatter.config.ext_color(&ext);
                match ext_color {
                    Some(ec) => format!("{}{}", display::paint_by_code(name_part, "97"), display::paint_by_code(&ext, ec)),
                    None => display::paint_by_code(name, "97"),
                }
            }
        }
    }
}

/// 多列输出
fn print_multi_column(items: &[ItemInfo], formatter: &Formatter, _no_color: bool) {
    use color::DisplayWidth;

    if items.is_empty() {
        return;
    }

    // 生成每列的显示字符串
    let entries: Vec<String> = items.iter()
        .map(|item| format_item_name_multi(item, formatter))
        .collect();

    let term_width = terminal_width();

    // 计算最大项宽度
    let max_width = entries.iter().map(|s| s.display_width()).max().unwrap_or(0);
    let col_width = max_width + 2; // 2 格间距

    if col_width >= term_width || items.len() <= 1 {
        // 太窄了或只有一个，每行一个
        for e in &entries {
            println!("{}", e);
        }
        return;
    }

    let num_cols = ((term_width - 1) / col_width).max(1);
    let num_rows = (items.len() + num_cols - 1) / num_cols;

    for row in 0..num_rows {
        let mut line = String::new();
        for col in 0..num_cols {
            let idx = col * num_rows + row;
            if idx < entries.len() {
                let s = &entries[idx];
                line.push_str(s);
                if col < num_cols - 1 {
                    let padding = col_width.saturating_sub(s.display_width());
                    for _ in 0..padding {
                        line.push(' ');
                    }
                }
            }
        }
        println!("{}", line);
    }
}

/// 排序项目
fn sort_items(items: &mut Vec<ItemInfo>, sort: &str) {
    match sort {
        "name" => {
            items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        "suffix" => {
            items.sort_by(|a, b| {
                let ext_a = std::path::Path::new(&a.name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let ext_b = std::path::Path::new(&b.name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                ext_a.cmp(&ext_b).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
        }
        "size" => {
            items.sort_by(|a, b| {
                // 目录优先，同级按大小降序
                if a.is_dir != b.is_dir {
                    return if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
                }
                b.size.cmp(&a.size)
            });
        }
        "create" => {
            items.sort_by(|a, b| {
                let ta = a.create_time_secs().unwrap_or(0);
                let tb = b.create_time_secs().unwrap_or(0);
                ta.cmp(&tb)
            });
        }
        "update" => {
            items.sort_by(|a, b| {
                let ta = a.modify_time_secs().unwrap_or(0);
                let tb = b.modify_time_secs().unwrap_or(0);
                ta.cmp(&tb)
            });
        }
        _ => {
            items.sort_by(|a, b| {
                if a.is_dir != b.is_dir {
                    return if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
    }
}

/// 树形显示
fn print_tree(
    path: &Path,
    formatter: &Formatter,
    cli: &Cli,
    prefix: &str,
    depth: i32,
) {
    if depth == 0 {
        return;
    }

    let all_items = scanner::scan_directory(path);
    let items: Vec<&ItemInfo> = all_items
        .iter()
        .filter(|item| {
            if !cli.exclude.is_empty() {
                if cli.exclude.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase())) {
                    return false;
                }
            }
            if !cli.include.is_empty() && !item.is_dir {
                return cli.include.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()));
            }
            true
        })
        .collect();

    let dirs: Vec<&ItemInfo> = items.iter().filter(|i| i.is_dir && i.link_type == links::LinkType::Dir).copied().collect();
    let files: Vec<&ItemInfo> = items.iter().filter(|i| !i.is_dir).copied().collect();

    let display_items: Vec<&ItemInfo> = match (cli.only_dirs, cli.only_files) {
        (true, _) => dirs,
        (_, true) => files,
        _ => {
            let mut merged: Vec<&ItemInfo> = dirs.iter().chain(files.iter()).copied().collect();
            merged.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            merged
        }
    };

    for (i, item) in display_items.iter().enumerate() {
        let is_last = i == display_items.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };

        if cli.no_color {
            println!("{}{}{}", prefix, connector, item.name);
        } else {
            let name = &item.name;
            if item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction) {
                // 目录：整体用目录色
                let color = formatter.get_item_color(item);
                match color {
                    Some(c) => println!("{}{}{}", prefix, connector, display::paint_by_code(name, c)),
                    None => println!("{}{}{}", prefix, connector, name),
                }
            } else {
                // 文件：白色主体 + 后缀颜色
                let ext = std::path::Path::new(name)
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                if ext.is_empty() {
                    println!("{}{}{}", prefix, connector, display::paint_by_code(name, "97"));
                } else {
                    let name_part = name.strip_suffix(&ext).unwrap_or(name);
                    let ext_color = formatter.config.ext_color(&ext);
                    let name_colored = display::paint_by_code(name_part, "97");
                    let ext_colored = match ext_color {
                        Some(ec) => display::paint_by_code(&ext, ec),
                        None => ext.clone(),
                    };
                    println!("{}{}{}{}", prefix, connector, name_colored, ext_colored);
                }
            }
        }

        if item.is_dir && !matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction) {
            let extension = if is_last { "    " } else { "│   " };
            let new_depth = if depth > 0 { depth - 1 } else { -1 };
            print_tree(&item.path, formatter, cli, &format!("{}{}", prefix, extension), new_depth);
        }
    }
}

// ── 递归列出 ──────────────────────────────────────────

/// 递归平铺列出目录内容（类似标准 `ls -R`）
fn print_recursive(root: &Path, formatter: &Formatter, cli: &Cli, sort_key: &str) {
    let mut dirs_to_visit: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut visited = std::collections::HashSet::new();

    while let Some(dir) = dirs_to_visit.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }

        let mut items = scanner::scan_directory(&dir);
        if items.is_empty() {
            continue;
        }

        // 过滤
        if !cli.exclude.is_empty() {
            items.retain(|item| {
                !cli.exclude.iter().any(|ext| {
                    item.name.to_lowercase().ends_with(&ext.to_lowercase())
                })
            });
        }
        if !cli.include.is_empty() {
            items.retain(|item| {
                if item.is_dir { return true; }
                cli.include.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()))
            });
        }
        if cli.only_files {
            items.retain(|item| item.is_file);
        } else if cli.only_dirs {
            items.retain(|item| {
                item.is_dir
                    || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction)
            });
        }

        sort_items(&mut items, sort_key);

        // 输出标题
        let display_path = dir.to_string_lossy().replace("\\\\?\\", "").replace("\\??\\", "");
        println!("{}:", display_path);

        if cli.long || cli.abs_path || cli.right_align {
            // 长格式输出
            let max_width = if cli.right_align {
                items.iter().map(|item| item.name.display_width()).max().unwrap_or(0)
            } else {
                0
            };
            let max_size_width = if cli.size {
                items.iter().map(|item| color::format_size(item.size).len()).max().unwrap_or(0)
            } else {
                0
            };
            for item in &items {
                let line = format_item_line(item, formatter, cli, max_width, max_size_width);
                println!("{}", line);
            }
        } else {
            // 多列输出
            print_multi_column(&items, formatter, cli.no_color);
        }
        println!();

        // 收集子目录供后续遍历
        let mut subdirs: Vec<PathBuf> = items.iter()
            .filter(|item| item.is_dir && !matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction))
            .map(|item| item.path.clone())
            .collect();
        subdirs.sort();
        dirs_to_visit.extend(subdirs.into_iter().rev()); // reverse so first dir is visited first (stack)
    }
}

/// 清理路径字符串：去除尾部引号、反斜杠、空白等 shell 转义残留。
fn sanitize_path(raw: &str) -> String {
    let mut s = raw.to_string();
    s = s.trim().to_string();
    while (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len()-1].to_string();
    }
    while s.ends_with('"') || s.ends_with('\'') {
        s.pop();
    }
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

/// 处理 --link 参数
fn handle_link_check(path: &str) {
    let cleaned = sanitize_path(path);
    let path = Path::new(&cleaned);
    if !path.exists() {
        eprintln!("{} 路径不存在: {}", red("错误:"), path.display());
        return;
    }

    let info = links::get_link_info(path);

    let type_name = match info.link_type {
        links::LinkType::Symlink => "符号链接 (Symbolic Link)",
        links::LinkType::Junction => "目录连接点 (Junction)",
        links::LinkType::Shortcut => "快捷方式 (.lnk)",
        links::LinkType::Dir => "普通目录",
        links::LinkType::File => "普通文件",
        links::LinkType::Unknown => "未知",
    };

    println!("{} {}", cyan("路径:"), path.display());
    println!("{} {}", cyan("类型:"), type_name);

    if let Some(ref target) = info.target {
        println!("{} {}", cyan("目标:"), target);
    }
}
