mod config;
mod display;
mod links;
mod scanner;

use std::path::Path;

use clap::Parser;
use color::*;
use display::Formatter;
use scanner::ItemInfo;

#[derive(Parser)]
#[command(
    name = "ls",
    version = version_str(),
    about,
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Cli {
    #[arg(default_value = ".")]
    directory: String,

    #[arg(short = 'n', long = "no-color")]
    no_color: bool,

    #[arg(short = 's', long = "sort", default_value = "default")]
    sort: String,

    #[arg(long = "exclude", num_args = 1..)]
    exclude: Vec<String>,

    #[arg(short = 'i', long = "include", num_args = 1..)]
    include: Vec<String>,

    #[arg(short = 'f', long = "only-files")]
    only_files: bool,

    #[arg(short = 'd', long = "only-dirs")]
    only_dirs: bool,

    #[arg(short = 'a', long = "abs-path")]
    abs_path: bool,

    #[arg(short = 'r', long = "right-align")]
    right_align: bool,

    #[arg(short = 'z', long = "size")]
    size: bool,

    #[arg(short = 't', long = "tree", require_equals = true, default_missing_value = "-1", num_args = 0..=1)]
    tree: Option<i32>,

    #[arg(long = "link")]
    link: Option<String>,

    #[arg(short = 'h', long = "help")]
    help: bool,

    #[arg(short = 'e', long = "examples")]
    examples: bool,

    #[arg(short = 'v', long = "version")]
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

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            print_clap_error(&e);
            return;
        }
    };

    if cli.help {
        print_short_help();
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
    let color_config = config::ColorConfig::default();
    let formatter = Formatter::new(color_config, cli.no_color);

    if let Some(link_path) = &cli.link {
        handle_link_check(link_path);
        return;
    }

    // 清理路径：去除可能因 shell 转义残留的尾部引号、反斜杠、空白
    let dir_str = sanitize_path(&cli.directory);
    let target_dir = Path::new(&dir_str);
    if !target_dir.exists() {
        eprintln!("错误: 目录 '{}' 不存在（输入: {}）", target_dir.display(), cli.directory);
        return;
    }
    if !target_dir.is_dir() {
        eprintln!("错误: '{}' 不是一个目录（输入: {}）", target_dir.display(), cli.directory);
        return;
    }

    if let Some(depth) = cli.tree {
        print_tree(target_dir, &formatter, &cli, "", depth);
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
fn print_short_help() {
    println!("  {}", bold_cyan("ls — 轻量级目录列表工具"));
    println!();
    println!("  {}", bold_yellow("用法:"));
    println!("    {} [{}]", green("ls"), gray("目录路径"));
    println!();
    println!("  {}", bold_yellow("选项:"));

    let opts: &[(&str, &str)] = &[
        ("-h, --help",       "显示简洁帮助"),
        ("-e, --examples",   "显示所有选项示例"),
        ("-v, --version",    "显示版本信息"),
        ("-n, --no-color",   "不使用颜色输出"),
        ("-s, --sort <排序>", "排序: default name suffix create update"),
        ("--exclude <后缀> ...", "排除指定后缀"),
        ("-i, --include <后缀> ...", "只包含指定后缀"),
        ("-f, --only-files",  "只显示文件"),
        ("-d, --only-dirs",   "只显示目录"),
        ("-a, --abs-path",    "打印完整路径"),
        ("-r, --right-align", "右对齐文件名"),
        ("-z, --size",        "显示文件大小"),
        ("-t[=N]",           "树形显示（-t=3 指定深度）"),
        ("--link <路径>",     "检查链接信息"),
    ];

    let max_w = opts.iter().map(|(o, _)| o.display_width()).max().unwrap_or(22);

    for (opt, desc) in opts {
        println!("  {}  {}",
            pad_left(&cyan(opt), max_w),
            gray(desc));
    }
    println!();

    println!("  {}  {}  {}",
        gray("提示:"),
        gray("查看完整示例请使用"),
        cyan("ls -e"));
    println!("  {}  {}  {}",
        gray("提示:"),
        gray("环境变量工具请使用"),
        cyan("e -h"));
}

/// 全部示例（仿 as -e 风格）
fn print_examples_help() {
    println!("  {}", bold_cyan("ls — 轻量级目录列表工具"));
    println!();
    println!("  {}", bold_yellow("目录列表"));

    let examples: &[(&str, &str)] = &[
        ("ls",                  "列出当前目录"),
        ("ls PATH",             "列出指定目录"),
        ("ls -t[=N]",           "树形显示（-t=3 指定深度）"),
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
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(26);

    for (cmd, desc) in examples {
        println!("  {}  {}",
            pad_left(&cyan(cmd), max_w),
            gray(desc));
    }
    println!();

    println!("  {}", bold_yellow("环境变量/PATH（需安装 e 工具）"));
    let env_examples: &[(&str, &str)] = &[
        ("e -s", "显示所有环境变量"),
        ("e -p", "显示 PATH"),
        ("e -g", "打开环境变量对话框"),
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
