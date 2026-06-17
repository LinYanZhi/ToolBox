mod config;
mod display;
mod links;
mod scanner;

use std::path::Path;

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

    /// 排序: default name suffix create update
    #[arg(short = 's', long = "sort", default_value = "default")]
    sort: String,

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

    /// 显示配置文件路径
    #[arg(long = "config")]
    config: bool,

    /// 在资源管理器中打开配置目录
    #[arg(long = "config-open")]
    config_open: bool,

    /// 清除配置文件，恢复默认配色
    #[arg(long = "config-clear")]
    config_clear: bool,

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

    // 配置管理（--config / --config-open / --config-clear）
    if cli.config {
        let path = config::ensure_config();
        println!("{}", path.display());
        return;
    }
    if cli.config_open {
        let dir = config::get_config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::process::Command::new("explorer")
            .arg(&*dir.to_string_lossy())
            .spawn();
        return;
    }
    if cli.config_clear {
        if config::clear_config() {
            println!("{} 配置文件已清除，下次运行将自动恢复为默认配置", green("✓"));
        } else {
            println!("{} 当前没有配置文件，无需清除", gray("•"));
        }
        return;
    }

    // 加载配置（优先读取 YAML 文件，不存在则自动创建）
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
        items.retain(|item| item.is_dir);
    }

    // 排序
    sort_items(&mut items, &cli.sort);

    // 计算最大宽度
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

    // 输出
    for item in &items {
        let line = format_item_line(item, &formatter, &cli, max_width, max_size_width);
        println!("{}", line);
    }
}

/// 简洁帮助（仿 as -h 风格）
/// 全部示例（仿 as -e 风格）
fn print_examples_help() {
    println!("  {}", bold_cyan("ls — 轻量级目录列表工具"));
    println!();
    println!("  {}", bold_yellow("目录列表"));

    let examples: &[(&str, &str)] = &[
        ("ls",                  "列出当前目录"),
        ("ls PATH",             "列出指定目录"),
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
        ("ls --link PATH",      "检查链接信息"),
        ("ls -n",               "不使用颜色输出"),
        ("ls --config",         "显示配置文件路径"),
        ("ls --config-open",    "打开配置目录"),
        ("ls --config-clear",   "清除配置文件"),
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
        let abs_path = std::fs::canonicalize(&item.path)
            .unwrap_or_else(|_| item.path.clone())
            .to_string_lossy()
            .to_string();

        if cli.no_color {
            line.push_str(&abs_path);
        } else {
            let color = formatter.get_item_color(item);
            match color {
                Some(c) => line.push_str(&display::paint_by_code(&abs_path, c)),
                None => line.push_str(&abs_path),
            }
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

    // Python/Java 环境版本
    if item.is_dir && !is_link {
        let name_lower = item.name.to_lowercase();
        let looks_like_python = name_lower == ".venv" || name_lower == "venv"
            || name_lower == "env" || name_lower == ".env"
            || name_lower.starts_with("python");
        let looks_like_java = name_lower.contains("jdk") || name_lower.contains("jre")
            || name_lower.contains("java");

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
        }
    }

    line
}

/// 带引号的环境版本号输出
fn format_text_colored(text: &str, _cli: &Cli) -> String {
    let quoted = format!("\"{}\"", text);
    gray(&quoted)
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
            let color = formatter.get_item_color(item);
            match color {
                Some(c) => println!("{}{}{}", prefix, connector, display::paint_by_code(&item.name, c)),
                None => println!("{}{}{}", prefix, connector, item.name),
            }
        }

        if item.is_dir && !matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction) {
            let extension = if is_last { "    " } else { "│   " };
            let new_depth = if depth > 0 { depth - 1 } else { -1 };
            print_tree(&item.path, formatter, cli, &format!("{}{}", prefix, extension), new_depth);
        }
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
