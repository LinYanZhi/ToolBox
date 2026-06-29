mod config;
mod display;
mod links;
mod scanner;

use std::path::{Path, PathBuf};

use arg::*;
use color::*;
use display::Formatter;
use scanner::ItemInfo;

// ── CLI 定义 ────────────────────────────────────────

fn build_cmd() -> Cmd {
    Cmd::new("ls")
        .about("轻量级目录列表工具 — 支持颜色、排序、过滤、树形、递归等多种显示")
        .arg(arg::ArgDef::value("directory", None, "要列出的目录路径").positional().default("."))
        .arg(flag("no-color", 'n', "不使用颜色输出"))
        .arg(ArgDef::value("sort", Some('s'), "排序方式")
            .default("default")
            .choices(&["default", "name", "suffix", "size", "create", "update"]))
        .arg(flag("size-sort", 'S', "按文件大小排序（大→小，等同 -s size）"))
        .arg(ArgDef::value("exclude", None, "排除指定后缀（如 .txt .md）").multi())
        .arg(ArgDef::value("include", Some('i'), "只包含指定后缀（如 .rs .py）").multi())
        .arg(flag("only-files", 'f', "只显示文件"))
        .arg(flag("only-dirs", 'd', "只显示目录"))
        .arg(flag("abs-path", 'a', "显示完整路径"))
        .arg(flag("right-align", 'r', "右对齐文件名"))
        .arg(flag("size", 'z', "显示文件大小"))
        .arg(ArgDef::value("tree", Some('t'), "树形显示（-t / -t 3）").optional())
        .arg(ArgDef::value("link", None, "检查链接信息"))
        .arg(flag("recursive", 'R', "递归列出子目录内容"))
        .arg(flag("help", 'h', "显示帮助").global())
        .arg(flag("examples", 'e', "显示所有选项示例"))
        .arg(flag("version", 'v', "显示版本号").global())
}

// ── main ────────────────────────────────────────────

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
        .unwrap_or_else(|_| "ls".into());

    if args.flag("help") {
        print_help(&cmd, &exe_path);
        return;
    }
    if args.flag("examples") {
        print_examples_help();
        return;
    }
    if args.flag("version") {
        print_version(&cmd, "0.0.1", "github.com/LinYanZhi/ToolBox");
        return;
    }

    let no_color = args.flag("no-color");
    let color_config = config::ColorConfig::load();
    let formatter = Formatter::new(color_config, no_color);

    // --link
    if let Some(link_path) = args.value("link") {
        handle_link_check(link_path);
        return;
    }

    let dir_str = sanitize_path(args.value("directory").unwrap_or("."));

    // 树形显示
    if let Some(_tree_val) = args.value("tree") {
        let tree_val = args.value("tree").map(|s| s.to_string());
        let (tree_depth, tree_path) = parse_tree_opt(&tree_val, &dir_str);
        let target = Path::new(&tree_path);
        if !target.exists() {
            eprintln!("错误: 目录 '{}' 不存在", target.display());
            return;
        }
        if !target.is_dir() {
            eprintln!("错误: '{}' 不是一个目录", target.display());
            return;
        }
        print_tree(target, &formatter, &args, "", tree_depth);
        return;
    }

    let target_dir = Path::new(&dir_str);
    if !target_dir.exists() {
        eprintln!("错误: 目录 '{}' 不存在", target_dir.display());
        return;
    }
    if !target_dir.is_dir() {
        eprintln!("错误: '{}' 不是一个目录", target_dir.display());
        return;
    }

    let mut items = scanner::scan_directory(target_dir);

    // 过滤
    let excludes: Vec<String> = args.values("exclude").iter().map(|s| s.to_string()).collect();
    if !excludes.is_empty() {
        items.retain(|item| {
            !excludes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()))
        });
    }

    let includes: Vec<String> = args.values("include").iter().map(|s| s.to_string()).collect();
    if !includes.is_empty() {
        items.retain(|item| {
            if item.is_dir { return true; }
            includes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()))
        });
    }

    if args.flag("only-files") {
        items.retain(|item| item.is_file);
    } else if args.flag("only-dirs") {
        items.retain(|item| {
            item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction)
        });
    }

    let size_sort = args.flag("size-sort");
    let sort_key = if size_sort { "size" } else { args.value("sort").unwrap_or("default") };
    sort_items(&mut items, sort_key);

    // 递归显示
    if args.flag("recursive") {
        let excluded: Vec<String> = args.values("exclude").iter().map(|s| s.to_string()).collect();
        let included: Vec<String> = args.values("include").iter().map(|s| s.to_string()).collect();
        print_recursive(target_dir, &formatter, no_color, args.flag("only-files"), args.flag("only-dirs"),
            &excluded, &included, args.flag("right-align"), args.flag("size"), sort_key);
        return;
    }

    // 普通输出
    let max_width = if args.flag("right-align") {
        items.iter().map(|item| item.name.display_width()).max().unwrap_or(0)
    } else { 0 };
    let max_size_width = if args.flag("size") {
        items.iter().map(|item| color::format_size(item.size).len()).max().unwrap_or(0)
    } else { 0 };

    for item in &items {
        let line = format_item_line(item, &formatter, &args, max_width, max_size_width, sort_key);
        println!("{}", line);
    }
}

// ── 示例帮助 ────────────────────────────────────────

fn print_examples_help() {
    println!("{}", bright_cyan("ls — 轻量级目录列表工具"));
    println!();
    println!("{}", bright_green("目录列表"));

    let examples: &[(&str, &str)] = &[
        ("ls", "列出当前目录"),
        ("ls PATH", "列出指定目录"),
        ("ls -t", "树形显示（无限深度）"),
        ("ls -t 3", "树形显示（深度 3）"),
        ("ls -t 3 路径", "树形显示指定目录"),
        ("ls -t 路径", "树形显示（自动识别为路径）"),
        ("ls --exclude .txt .md", "排除指定后缀"),
        ("ls -i .rs .py", "只包含指定后缀"),
        ("ls -a", "显示完整路径"),
        ("ls -r", "右对齐文件名"),
        ("ls -z", "显示文件大小"),
        ("ls -f", "只显示文件"),
        ("ls -d", "只显示目录"),
        ("ls -s name", "按名称排序"),
        ("ls -s suffix", "按后缀排序"),
        ("ls -S", "按文件大小排序（大→小）"),
        ("ls -R", "递归列出子目录内容"),
        ("ls --link PATH", "检查链接信息"),
        ("ls -n", "不使用颜色输出"),
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(28);
    for (cmd, desc) in examples {
        println!("  {}  {}", pad_left(&cyan(cmd), max_w), gray(desc));
    }
    println!();
}

// ── 格式化 ──────────────────────────────────────────

fn format_item_line(
    item: &ItemInfo,
    formatter: &Formatter,
    args: &ParsedArgs,
    max_width: usize,
    max_size_width: usize,
    sort_key: &str,
) -> String {
    let mut line = String::new();
    let no_color = args.flag("no-color");
    let abs_path = args.flag("abs-path");
    let right_align = args.flag("right-align");
    let show_size = args.flag("size");

    if abs_path {
        let raw = std::fs::canonicalize(&item.path).unwrap_or_else(|_| item.path.clone());
        let abs_path_str = raw.to_string_lossy().replace("\\\\?\\", "").replace("\\??\\", "");

        if no_color {
            line.push_str(&abs_path_str);
            return line;
        }

        let path = Path::new(&abs_path_str);
        let parent = path.parent().and_then(|p| p.to_str());
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(&abs_path_str);
        let is_link = matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction);

        if let Some(parent) = parent {
            if !parent.is_empty() && parent != "." {
                line.push_str(&display::paint_by_code(&format!("{}\\", parent), "90"));
            }
        }

        if item.is_dir || is_link {
            let color = formatter.get_item_color(item);
            match color {
                Some(c) => line.push_str(&display::paint_by_code(file_name, c)),
                None => line.push_str(file_name),
            }
        } else {
            let ext = Path::new(file_name).extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            if ext.is_empty() {
                line.push_str(&display::paint_by_code(file_name, "37"));
            } else {
                let name_part = file_name.strip_suffix(&ext).unwrap_or(file_name);
                let ext_color = formatter.config.ext_color(&ext);
                line.push_str(&display::paint_by_code(name_part, "37"));
                match ext_color {
                    Some(ec) => line.push_str(&display::paint_by_code(&ext, ec)),
                    None => line.push_str(&ext),
                }
            }
        }

        if let Some(ref target) = item.link_target {
            line.push_str(&formatter.print_link_arrow(item.is_dir));
            line.push_str(&formatter.print_link_target(target, item.link_type == links::LinkType::Shortcut));
        }
        return line;
    }

    let is_link = matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction);
    let is_shortcut = item.link_type == links::LinkType::Shortcut;

    if item.is_dir || is_link {
        line.push_str(&formatter.print_type_marker("<dir>"));
    } else {
        line.push_str(&formatter.print_type_marker("<file>"));
    }

    let show_time = sort_key == "create" || sort_key == "update";
    if show_time {
        let secs = if sort_key == "create" {
            item.create_time_secs().unwrap_or(0)
        } else {
            item.modify_time_secs().unwrap_or(0)
        };
        line.push_str(&formatter.print_timestamp(secs));
    }

    line.push_str(&formatter.print_file_name(item, right_align, max_width));

    if show_size && !item.is_dir {
        line.push_str(&formatter.print_size(item, max_size_width));
    }

    if is_link || is_shortcut {
        if let Some(ref target) = item.link_target {
            line.push_str(&formatter.print_link_arrow(item.is_dir));
            line.push_str(&formatter.print_link_target(target, is_shortcut));
        }
    }

    if item.is_dir && !is_link {
        let name_lower = item.name.to_lowercase();
        let looks_like_python = name_lower == ".venv" || name_lower == "venv"
            || name_lower == "env" || name_lower == ".env" || name_lower.starts_with("python");
        let looks_like_java = name_lower.contains("jdk") || name_lower.contains("jre") || name_lower.contains("java");
        let looks_like_node = name_lower.contains("node");

        if looks_like_python {
            if let Some(ver) = item.get_python_env() {
                line.push_str(&format_text_colored(&ver));
            }
        } else if looks_like_java {
            if let Some(ver) = item.get_java_env() {
                line.push_str(&format_text_colored(&ver));
            }
        } else if looks_like_node {
            if let Some(ver) = item.get_node_info() {
                line.push_str(&format_text_colored(&ver));
            }
        }
    }

    if item.is_dir && !is_link {
        if let Some(info) = item.get_git_info() {
            line.push_str(&format_text_colored(&info));
        }
    }

    line
}

fn format_text_colored(text: &str) -> String {
    gray(&format!("\"{}\"", text))
}

// ── 排序 ────────────────────────────────────────────

fn sort_items(items: &mut Vec<ItemInfo>, sort: &str) {
    match sort {
        "name" => items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "suffix" => items.sort_by(|a, b| {
            let ext_a = Path::new(&a.name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let ext_b = Path::new(&b.name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            ext_a.cmp(&ext_b).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }),
        "size" => items.sort_by(|a, b| {
            if is_dirish(a) != is_dirish(b) {
                return if is_dirish(a) { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            b.size.cmp(&a.size)
        }),
        "create" => items.sort_by(|a, b| {
            let ta = a.create_time_secs().unwrap_or(0);
            let tb = b.create_time_secs().unwrap_or(0);
            ta.cmp(&tb)
        }),
        "update" => items.sort_by(|a, b| {
            let ta = a.modify_time_secs().unwrap_or(0);
            let tb = b.modify_time_secs().unwrap_or(0);
            ta.cmp(&tb)
        }),
        _ => items.sort_by(|a, b| {
            if is_dirish(a) != is_dirish(b) {
                return if is_dirish(a) { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
            }
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }),
    }
}

fn is_dirish(item: &ItemInfo) -> bool {
    item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction)
}

// ── 树形显示 ────────────────────────────────────────

fn print_tree(
    path: &Path,
    formatter: &Formatter,
    args: &ParsedArgs,
    prefix: &str,
    depth: i32,
) {
    if depth == 0 { return; }

    let all_items = scanner::scan_directory(path);
    let excludes: Vec<String> = args.values("exclude").iter().map(|s| s.to_string()).collect();
    let includes: Vec<String> = args.values("include").iter().map(|s| s.to_string()).collect();
    let no_color = args.flag("no-color");
    let only_dirs = args.flag("only-dirs");
    let only_files = args.flag("only-files");

    let items: Vec<&ItemInfo> = all_items.iter()
        .filter(|item| {
            if !excludes.is_empty() && excludes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase())) {
                return false;
            }
            if !includes.is_empty() && !item.is_dir {
                return includes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()));
            }
            true
        })
        .collect();

    let dirs: Vec<&ItemInfo> = items.iter().filter(|i| i.is_dir && i.link_type == links::LinkType::Dir).copied().collect();
    let files: Vec<&ItemInfo> = items.iter().filter(|i| !i.is_dir).copied().collect();

    let display_items: Vec<&ItemInfo> = match (only_dirs, only_files) {
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

        if no_color {
            println!("{}{}{}", prefix, connector, item.name);
        } else {
            let name = &item.name;
            if item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction) {
                let color = formatter.get_item_color(item);
                match color {
                    Some(c) => println!("{}{}{}", prefix, connector, display::paint_by_code(name, c)),
                    None => println!("{}{}{}", prefix, connector, name),
                }
            } else {
                let ext = Path::new(name).extension()
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
            print_tree(&item.path, formatter, args, &format!("{}{}", prefix, extension), new_depth);
        }
    }
}

// ── 递归列出 ────────────────────────────────────────

fn print_recursive(
    root: &Path,
    formatter: &Formatter,
    _no_color: bool,
    only_files: bool,
    only_dirs: bool,
    excludes: &[String],
    includes: &[String],
    right_align: bool,
    show_size: bool,
    sort_key: &str,
) {
    let mut dirs_to_visit: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut visited = std::collections::HashSet::new();

    while let Some(dir) = dirs_to_visit.pop() {
        if !visited.insert(dir.clone()) { continue; }

        let mut items = scanner::scan_directory(&dir);
        if items.is_empty() { continue; }

        if !excludes.is_empty() {
            items.retain(|item| {
                !excludes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()))
            });
        }
        if !includes.is_empty() {
            items.retain(|item| {
                if item.is_dir { return true; }
                includes.iter().any(|ext| item.name.to_lowercase().ends_with(&ext.to_lowercase()))
            });
        }
        if only_files {
            items.retain(|item| item.is_file);
        } else if only_dirs {
            items.retain(|item| {
                item.is_dir || matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction)
            });
        }

        sort_items(&mut items, sort_key);

        let display_path = dir.to_string_lossy().replace("\\\\?\\", "").replace("\\??\\", "");
        println!("{}:", display_path);

        let max_width = if right_align {
            items.iter().map(|item| item.name.display_width()).max().unwrap_or(0)
        } else { 0 };
        let max_size_width = if show_size {
            items.iter().map(|item| color::format_size(item.size).len()).max().unwrap_or(0)
        } else { 0 };

        // We can't use format_item_line here easily since it needs ParsedArgs with specific flags.
        // Instead, let's print directly, simplified.
        for item in &items {
            let mut line = String::new();
            let is_link = matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction);
            if item.is_dir || is_link {
                line.push_str(&formatter.print_type_marker("<dir>"));
            } else {
                line.push_str(&formatter.print_type_marker("<file>"));
            }
            line.push_str(&formatter.print_file_name(item, right_align, max_width));
            if show_size && !item.is_dir {
                line.push_str(&formatter.print_size(item, max_size_width));
            }
            println!("{}", line);
        }
        println!();

        let mut subdirs: Vec<PathBuf> = items.iter()
            .filter(|item| item.is_dir && !matches!(item.link_type, links::LinkType::Symlink | links::LinkType::Junction))
            .map(|item| item.path.clone())
            .collect();
        subdirs.sort();
        dirs_to_visit.extend(subdirs.into_iter().rev());
    }
}

// ── 路径清理 ────────────────────────────────────────

fn sanitize_path(raw: &str) -> String {
    let mut s = raw.to_string();
    s = s.trim().to_string();
    while (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len() - 1].to_string();
    }
    while s.ends_with('"') || s.ends_with('\'') { s.pop(); }
    while s.len() > 3 && s.ends_with('\\') { s.pop(); }
    s
}

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
        links::LinkType::Unknown => "未知类型",
    };
    println!("类型: {}", cyan(type_name));
    if let Some(ref target) = info.target {
        println!("目标: {}", green(target));
    }
}

fn parse_tree_opt(tree: &Option<String>, dir: &str) -> (i32, String) {
    match tree.as_ref() {
        Some(s) if s.is_empty() => (-1, dir.to_string()),
        Some(s) => {
            if let Ok(n) = s.parse::<i32>() {
                (n, dir.to_string())
            } else {
                (-1, s.to_string())
            }
        }
        _ => (-1, dir.to_string()),
    }
}

// ── 颜色快捷 ──

fn bright_cyan(text: &str) -> String  { color::Style::new(96).paint(text) }
fn bright_green(text: &str) -> String { color::Style::new(92).paint(text) }
