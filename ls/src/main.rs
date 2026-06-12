mod config;
mod display;
mod links;
mod scanner;

use std::path::Path;

use clap::Parser;
use display::Formatter;
use scanner::ItemInfo;

#[derive(Parser)]
#[command(
    name = "ls",
    version = version_info(),
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

    #[arg(short = 'e', long = "exclude", num_args = 1..)]
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

    #[arg(short = 't', long = "tree", num_args = 0..=1, default_missing_value = "-1")]
    tree: Option<i32>,

    #[arg(long = "link")]
    link: Option<String>,

    #[arg(short = 'h', long = "help")]
    help: bool,

    #[arg(short = 'v', long = "version")]
    version: bool,
}

fn version_info() -> &'static str {
    concat!(
        "\x1b[1;36mls \x1b[32m",
        env!("CARGO_PKG_VERSION"),
        "\x1b[0m (\x1b[90mRust 版\x1b[0m)\n\
         \x1b[33m一个轻量级的目录列表工具\x1b[0m\n\
         ---\n\
         \x1b[34mGitHub: \x1b[0m\x1b[4mhttps://github.com/LinYanZhi/ToolBox\x1b[0m"
    )
    .trim()
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
        print_help();
        return;
    }

    if cli.version {
        println!("{}", version_info());
        return;
    }

    // 加载配置
    let color_config = config::ColorConfig::default();
    let formatter = Formatter::new(color_config, cli.no_color);

    if let Some(link_path) = &cli.link {
        handle_link_check(link_path);
        return;
    }

    let target_dir = Path::new(&cli.directory);
    if !target_dir.exists() {
        eprintln!("错误: 目录 '{}' 不存在", cli.directory);
        return;
    }
    if !target_dir.is_dir() {
        eprintln!("错误: '{}' 不是一个目录", cli.directory);
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
        items.iter().map(|item| display::display_width(&item.name)).max().unwrap_or(0)
    } else {
        0
    };

    let max_size_width = if cli.size {
        items.iter().map(|item| display::format_size(item.size).len()).max().unwrap_or(0)
    } else {
        0
    };

    // 输出
    for item in &items {
        let line = format_item_line(item, &formatter, &cli, max_width, max_size_width);
        println!("{}", line);
    }
}

/// 打印中文帮助信息
fn print_help() {
    println!("\x1b[1;33m用法:\x1b[0m ls \x1b[36m[-h]\x1b[0m \x1b[36m[-n]\x1b[0m \x1b[36m[-s\x1b[0m \x1b[90m{{\x1b[0m\x1b[90mdefault,name,suffix,create,update\x1b[0m\x1b[90m}}\x1b[0m\x1b[36m]\x1b[0m");
    println!("\x1b[36m            [-e\x1b[0m \x1b[4mEXCLUDE\x1b[0m\x1b[36m [\x1b[0m\x1b[4mEXCLUDE\x1b[0m\x1b[36m ...]]\x1b[0m");
    println!("\x1b[36m            [-i\x1b[0m \x1b[4mINCLUDE\x1b[0m\x1b[36m [\x1b[0m\x1b[4mINCLUDE\x1b[0m\x1b[36m ...]]\x1b[0m \x1b[36m[-f]\x1b[0m \x1b[36m[-d]\x1b[0m");
    println!("\x1b[36m            [-a]\x1b[0m \x1b[36m[-r]\x1b[0m \x1b[36m[-z]\x1b[0m \x1b[36m[-t\x1b[0m \x1b[90m[\x1b[0m\x1b[4mTREE\x1b[0m\x1b[90m]\x1b[0m\x1b[36m]\x1b[0m \x1b[36m[--link\x1b[0m \x1b[4mPATH\x1b[0m\x1b[36m]\x1b[0m");
    println!("\x1b[36m            [\x1b[0m\x1b[4mdirectory\x1b[0m\x1b[36m]\x1b[0m");
    println!();
    println!("\x1b[1;33m列出目录内容\x1b[0m");
    println!();
    println!("\x1b[1;33m位置参数:\x1b[0m");
    println!("  \x1b[4mdirectory\x1b[0m              要列出的目录路径（默认: 当前目录）");
    println!();
    println!("\x1b[1;33m选项:\x1b[0m");
    println!("  \x1b[36m-h\x1b[0m, \x1b[36m--help\x1b[0m              \x1b[90m显示帮助信息\x1b[0m");
    println!("  \x1b[36m-n\x1b[0m, \x1b[36m--no-color\x1b[0m          \x1b[90m不使用颜色输出\x1b[0m");
    println!("  \x1b[36m-s\x1b[0m, \x1b[36m--sort\x1b[0m \x1b[90m<\x1b[0m\x1b[4m排序\x1b[0m\x1b[90m>\x1b[0m       \x1b[90m排序方式: default name suffix create update\x1b[0m");
    println!("  \x1b[36m-e\x1b[0m, \x1b[36m--exclude\x1b[0m \x1b[90m<\x1b[0m\x1b[4m后缀\x1b[0m\x1b[90m>\x1b[0m    \x1b[90m排除指定后缀的文件，如 -e .txt .md\x1b[0m");
    println!("  \x1b[36m-i\x1b[0m, \x1b[36m--include\x1b[0m \x1b[90m<\x1b[0m\x1b[4m后缀\x1b[0m\x1b[90m>\x1b[0m    \x1b[90m只包含指定后缀的文件，如 -i .rs .py\x1b[0m");
    println!("  \x1b[36m-f\x1b[0m, \x1b[36m--only-files\x1b[0m         \x1b[90m只显示文件\x1b[0m");
    println!("  \x1b[36m-d\x1b[0m, \x1b[36m--only-dirs\x1b[0m          \x1b[90m只显示目录\x1b[0m");
    println!("  \x1b[36m-a\x1b[0m, \x1b[36m--abs-path\x1b[0m           \x1b[90m打印完整路径\x1b[0m");
    println!("  \x1b[36m-r\x1b[0m, \x1b[36m--right-align\x1b[0m        \x1b[90m右对齐文件名\x1b[0m");
    println!("  \x1b[36m-z\x1b[0m, \x1b[36m--size\x1b[0m               \x1b[90m显示文件大小\x1b[0m");
    println!("  \x1b[36m-t\x1b[0m \x1b[90m[\x1b[0m\x1b[4m深度\x1b[0m\x1b[90m]\x1b[0m                 \x1b[90m树形显示目录结构，-t 3 指定深度，不带数字无限递归\x1b[0m");
    println!("  \x1b[36m--link\x1b[0m \x1b[4mPATH\x1b[0m               \x1b[90m检查指定路径的链接信息\x1b[0m");
    println!("  \x1b[36m-v\x1b[0m, \x1b[36m--version\x1b[0m            \x1b[90m显示版本信息\x1b[0m");
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
                eprintln!("错误: 未知的选项 '{}'", flag);
                eprintln!("提示: 使用 --help 查看可用选项");
            } else {
                eprintln!("错误: 未知的选项");
                eprintln!("提示: 使用 --help 查看可用选项");
            }
        }
        ErrorKind::InvalidSubcommand => {
            eprintln!("错误: 未知的子命令");
            eprintln!("提示: 使用 --help 查看可用选项");
        }
        ErrorKind::MissingRequiredArgument => {
            eprintln!("错误: 缺少必需参数");
            eprintln!("提示: 使用 --help 查看正确用法");
        }
        ErrorKind::TooManyValues => {
            eprintln!("错误: 参数值过多");
            eprintln!("提示: 使用 --help 查看正确用法");
        }
        ErrorKind::TooFewValues => {
            eprintln!("错误: 参数值不足");
            eprintln!("提示: 使用 --help 查看正确用法");
        }
        ErrorKind::ValueValidation => {
            let msg = err.to_string();
            // 提取更友好的错误信息
            if msg.contains("invalid value") {
                if let Some(val) = msg.split('\'').nth(1) {
                    if let Some(choices) = msg.split('[').nth(1).and_then(|s| s.split(']').next()) {
                        eprintln!("错误: 无效的值 '{}'，可选值: {}", val, choices);
                    } else {
                        eprintln!("错误: 无效的值 '{}'", val);
                    }
                } else {
                    eprintln!("错误: 参数值无效");
                }
            } else {
                eprintln!("错误: {}", msg.lines().next().unwrap_or("参数值无效"));
            }
            eprintln!("提示: 使用 --help 查看正确用法");
        }
        _ => {
            // 其他错误直接显示中文包装
            let msg = err.to_string();
            let first_line = msg.lines().next().unwrap_or("未知错误");
            if first_line.contains("unexpected") || first_line.contains("error:") {
                eprintln!("错误: 参数解析失败");
                eprintln!("提示: 使用 --help 查看正确用法");
            } else {
                eprintln!("错误: {}", first_line);
            }
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
                Some(c) => line.push_str(&display::colored(&abs_path, c)),
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
                line.push_str(&format_text_colored(&text, &formatter, &cli));
            }
        } else if looks_like_java {
            if let Some(ver) = item.get_java_env() {
                line.push_str(&format_text_colored(&ver, &formatter, &cli));
            }
        }
    }

    line
}

/// 带引号的环境版本号输出
fn format_text_colored(text: &str, _formatter: &Formatter, cli: &Cli) -> String {
    let quoted = format!("\"{}\"", text);
    if cli.no_color {
        quoted
    } else {
        display::colored(&quoted, "90")
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
                Some(c) => println!("{}{}{}", prefix, connector, display::colored(&item.name, c)),
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

/// 处理 --link 参数
fn handle_link_check(path: &str) {
    let path = Path::new(path);
    if !path.exists() {
        eprintln!("错误: 路径不存在: {}", path.display());
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

    println!("路径: {}", path.display());
    println!("类型: {}", type_name);

    if let Some(ref target) = info.target {
        println!("目标: {}", target);
    }
}

