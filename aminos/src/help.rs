use clap::builder::styling::{AnsiColor, Color, Style, Styles};

use color::pad_left as pad;
use color::DisplayWidth;
use crate::cmd_names;

/// 统一帮助配色方案
pub const HELP_STYLES: Styles = Styles::styled()
    .header(*Styles::styled().get_header())
    .usage(Style::new().bold().fg_color(Some(Color::Ansi(AnsiColor::Yellow))))
    .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
    .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
    .error(*Styles::styled().get_error());

/// 命令帮助条目：定义命令名称、中文描述、名称样式
pub struct CmdHelp {
    pub name: &'static str,
    pub desc: &'static str,
    pub style: color::Style,
}

/// 根级命令列表（每个命令可独立配色）
pub const ROOT_COMMANDS: &[CmdHelp] = &[
    CmdHelp { name: "install",   desc: "安装指定软件",                           style: color::BOLD_GREEN },
    CmdHelp { name: "list",      desc: "列出可用软件及安装状态",                 style: color::BOLD_CYAN },
    CmdHelp { name: "uninstall", desc: "卸载指定软件",                           style: color::BOLD_RED },
    CmdHelp { name: "upgrade",   desc: "升级已安装的软件",                       style: color::BOLD_MAGENTA },
    CmdHelp { name: "config",   desc: "管理 as 环境和配置（路径/缓存/源/下载器）",   style: color::BRIGHT_CYAN },
    CmdHelp { name: "self",      desc: "管理 as 自身（初始化、更新）",           style: color::BRIGHT_GREEN },
    CmdHelp { name: "tool",      desc: "管理自研工具（已安装的 as 工具集）",     style: color::BRIGHT_YELLOW },
];

/// 打印根命令帮助
pub fn print_root_help() {
    let max_name_w = ROOT_COMMANDS
        .iter()
        .map(|c| c.name.display_width())
        .max()
        .unwrap_or(10);

    println!();
    println!("  {} — {}", color::bold_cyan("aminos"), color::green("轻量级 Windows 软件包管理器"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as"), color::green("<命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("命令:"));
    for cmd in ROOT_COMMANDS {
        let name_colored = cmd.style.paint(pad(cmd.name, max_name_w));
        println!("    {}    {}", name_colored, cmd.desc);
    }
    println!();
    println!("  {}", color::bold_yellow("选项:"));

    let opts: &[(&str, &str)] = &[
        ("-e, --example",  "显示所有命令的示例用法"),
        ("-h, --help",     "显示帮助信息"),
        ("-V, --version",  "显示版本信息"),
    ];

    let max_opt_w = opts.iter().map(|(o, _)| o.display_width()).max().unwrap_or(18);

    for (opt, desc) in opts {
        println!("    {}  {}",
            pad(&color::cyan(opt), max_opt_w),
            desc);
    }
    println!();
    println!("  {}", color::bold_yellow("提示:"));
    println!("    {} 了解更多请使用 {}as <命令> --help{}", color::gray("•"), color::cyan(""), color::gray(""));
    println!("    {} 查看示例请使用 {}as -e{}", color::gray("•"), color::cyan(""), color::gray(""));
}



/// 打印 as config 子命令帮助
pub fn print_config_help() {
    println!("  {} — {}", color::bold_cyan(cmd_names::CONFIG), color::green("管理 as 环境和配置"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan(cmd_names::CONFIG), color::green("<子命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("子命令:"));

    let subcmds: &[(&str, &str)] = &[
        ("path",       "显示/打开配置目录"),
        ("cache",      "管理下载缓存"),
        ("source",     "管理软件源定义"),
        ("speedtest",  "测速所有下载源"),
        ("downloader", "管理下载引擎后端"),
    ];

    let max_w = subcmds.iter().map(|(n, _)| n.display_width()).max().unwrap_or(12);

    for (name, desc) in subcmds {
        println!("    {}  {}",
            pad(&color::cyan(name), max_w),
            desc);
    }
    println!();
     println!("  {}", color::bold_yellow("示例:"));

    let examples = vec![
        (cmd_names::CONFIG_PATH.to_string(),          "查看所有数据目录位置"),
        (format!("{} -o", cmd_names::CONFIG_PATH),    "在资源管理器中打开配置目录"),
        (cmd_names::CONFIG_CACHE.to_string(),          "查看缓存文件"),
        (cmd_names::CONFIG_CACHE_CLEAR.to_string(),    "清除所有缓存"),
        (cmd_names::CONFIG_SOURCE_UPDATE.to_string(),  "更新软件源和工具源"),
        (cmd_names::CONFIG_SPEEDTEST.to_string(),      "测速下载源"),
        (cmd_names::CONFIG_DOWNLOADER_LIST.to_string(),"列出下载后端"),
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(44);

    for (cmd, desc) in &examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
}

/// 打印 as config source 子命令帮助
pub fn print_source_help() {
    println!();
    println!("  {} — {}", color::bold_cyan(cmd_names::CONFIG_SOURCE), color::green("管理软件源和工具源定义"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan(cmd_names::CONFIG_SOURCE), color::green("<子命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("子命令:"));

    let subcmds: &[(&str, &str)] = &[
        ("update",           "从远程仓库下载最新源定义"),
        ("path",             "显示源定义目录路径"),
        ("path -o",          "在资源管理器中打开源目录"),
    ];

    let max_w = subcmds.iter().map(|(n, _)| n.display_width()).max().unwrap_or(10);

    for (name, desc) in subcmds {
        println!("    {}  {}",
            pad(&color::cyan(name), max_w),
            desc);
    }
}

/// 打印 as config downloader 子命令帮助
pub fn print_downloader_help() {
    println!();
    println!("  {} — {}", color::bold_cyan(cmd_names::CONFIG_DOWNLOADER), color::green("管理下载引擎后端"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan(cmd_names::CONFIG_DOWNLOADER), color::green("<子命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("子命令:"));

    let subcmds: &[(&str, &str)] = &[
        ("list",               "列出所有下载后端及启用状态"),
        ("set <名称> on|off",   "启用或禁用一个后端"),
        ("config",             "显示配置文件路径"),
        ("config -o",          "在资源管理器中打开配置目录"),
    ];

    let max_w = subcmds.iter().map(|(n, _)| n.display_width()).max().unwrap_or(24);

    for (name, desc) in subcmds {
        println!("    {}  {}",
            pad(&color::cyan(name), max_w),
            desc);
    }
}

/// 打印 as self 子命令帮助
pub fn print_self_help() {
    println!();
    println!("  {} — {}", color::bold_cyan(cmd_names::SELF), color::green("管理 as 自身"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan(cmd_names::SELF), color::green("<子命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("子命令:"));

    let self_subcmds: &[(&str, &str)] = &[
        ("init",   "初始化 as 环境（创建 tools/bin 并注册到 PATH）"),
        ("update", "更新 as 自身到最新版本"),
    ];

    let max_w = self_subcmds.iter().map(|(n, _)| n.display_width()).max().unwrap_or(8);

    for (name, desc) in self_subcmds {
        println!("    {}  {}",
            pad(&color::cyan(name), max_w),
            desc);
    }
    println!();
    println!("  {}", color::bold_yellow("示例:"));

    let self_examples = vec![
        (cmd_names::SELF_INIT.to_string(),   "初始化环境"),
        (cmd_names::SELF_UPDATE.to_string(), "更新自身"),
    ];

    let max_w = self_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    for (cmd, desc) in &self_examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
}

/// 打印 as tool 子命令帮助
pub fn print_tool_help() {
    println!();
    println!("  {} — {}", color::bold_cyan(cmd_names::TOOL), color::green("管理自研工具"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan(cmd_names::TOOL), color::green("<子命令>"), color::gray("[参数]"));
    println!();
    println!("  {}", color::bold_yellow("子命令:"));

    let tool_subcmds: &[(&str, &str)] = &[
        ("install", "安装/更新自研工具（从 source/tools/ 读取）"),
        ("upgrade", "升级所有已安装的自研工具"),
        ("list",    "列出所有可用自研工具及安装状态"),
        ("remove",  "移除一个自研工具"),
    ];

    let max_w = tool_subcmds.iter().map(|(n, _)| n.display_width()).max().unwrap_or(10);

    for (name, desc) in tool_subcmds {
        println!("    {}  {}",
            pad(&color::cyan(name), max_w),
            desc);
    }
    println!();
    println!("  {}", color::bold_yellow("示例:"));

    let tool_examples = vec![
        (format!("{} ls", cmd_names::TOOL_INSTALL), "安装 ls 工具"),
        (format!("{} ls uv", cmd_names::TOOL_INSTALL), "同时安装多个工具"),
        (cmd_names::TOOL_LIST.to_string(), "列出自研工具"),
        (format!("{} ls", cmd_names::TOOL_REMOVE), "移除 ls 工具"),
        (cmd_names::TOOL_UPGRADE.to_string(), "升级所有自研工具"),
    ];

    let max_w = tool_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(32);

    for (cmd, desc) in &tool_examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
    println!();
    println!("  {}", color::gray(format!("第三方软件请使用 {} 命令", cmd_names::INSTALL)));
}

/// 仅含选项的子命令帮助模板
pub const HELP_TEMPLATE_OPTIONS: &str = "\
{about}

\x1b[1;33m用法:\x1b[0m
  {usage}

\x1b[1;33m选项:\x1b[0m
{options}";

/// 含子命令的帮助模板
pub const HELP_TEMPLATE_SUBCMDS: &str = "\
{about}

\x1b[1;33m用法:\x1b[0m
  {usage}

\x1b[1;33m命令:\x1b[0m
{subcommands}
\x1b[1;33m选项:\x1b[0m
{options}";

/// 显示所有命令的详细示例用法
pub fn run_example() {
    println!();
    println!("  {}", color::bold_cyan("aminos 命令参考手册"));
    println!();

    // 每组示例：(分组名, 分组描述, 示例列表)
    struct Group<'a> { cmd: String, desc: &'a str, entries: Vec<(String, &'a str)> }

    let examples = vec![
        Group {
            cmd: "install".into(), desc: "安装指定软件",
            entries: vec![
                (format!("{} 7zip", cmd_names::INSTALL), "安装 7-Zip（最新版）"),
                (format!("{} vscode python git", cmd_names::INSTALL), "同时安装多个软件"),
                (format!("{} 7zip -v 1.0.0", cmd_names::INSTALL), "安装指定版本"),
                (format!("{} 7zip --gui", cmd_names::INSTALL), "使用图形界面向导安装"),
                (format!("{} 7zip --renew", cmd_names::INSTALL), "强制重新下载并安装"),
                (format!("{} 7zip --download-only", cmd_names::INSTALL), "仅下载，不安装"),
                (format!("{} 7zip --type portable", cmd_names::INSTALL), "指定安装类型为便携版"),
            ],
        },
        Group {
            cmd: "list".into(), desc: "列出可用软件及安装状态",
            entries: vec![
                (cmd_names::LIST.to_string(), "列出所有软件"),
                (format!("{} -g", cmd_names::LIST), "按分类分组显示"),
                (format!("{} --categories", cmd_names::LIST), "查看分类概览"),
                (format!("{} -i", cmd_names::LIST), "仅显示已安装的软件"),
                (format!("{} -m", cmd_names::LIST), "仅显示未安装的软件"),
                (format!("{} -f 开发工具", cmd_names::LIST), "按分类过滤"),
                (format!("{} -s 压缩", cmd_names::LIST), "搜索名称、别名或描述"),
                (format!("{} -d", cmd_names::LIST), "仅显示已下载缓存的软件"),
                (format!("{} --info 7zip", cmd_names::LIST), "查看 7-Zip 的详细信息"),
                (format!("{} --info 7zip --urls", cmd_names::LIST), "查看 7-Zip 所有下载地址"),
            ],
        },
        Group {
            cmd: "uninstall".into(), desc: "卸载指定软件",
            entries: vec![
                (format!("{} 7zip", cmd_names::UNINSTALL), "静默卸载 7-Zip"),
                (format!("{} vscode python", cmd_names::UNINSTALL), "同时卸载多个软件"),
                (format!("{} 7zip --gui", cmd_names::UNINSTALL), "使用图形界面卸载向导"),
                (format!("{} 7zip --force", cmd_names::UNINSTALL), "强制删除（跳过卸载器）"),
            ],
        },
        Group {
            cmd: "upgrade".into(), desc: "升级已安装的软件",
            entries: vec![
                (cmd_names::UPGRADE.to_string(), "升级所有已安装的软件"),
                (format!("{} 7zip", cmd_names::UPGRADE), "仅升级指定软件"),
                (format!("{} --check", cmd_names::UPGRADE), "仅检查更新，不下载不安装"),
                (format!("{} --renew", cmd_names::UPGRADE), "强制重新下载（即使版本相同）"),
            ],
        },
        Group {
            cmd: "config cache".into(), desc: "管理下载缓存",
            entries: vec![
                (cmd_names::CONFIG_CACHE.to_string(), "查看缓存文件列表和一致性"),
                (cmd_names::CONFIG_CACHE_CLEAR.to_string(), "清除所有缓存文件"),
                (cmd_names::CONFIG_CACHE_OPEN.to_string(), "在资源管理器中打开缓存目录"),
            ],
        },
        Group {
            cmd: "config path".into(), desc: "显示/打开配置目录",
            entries: vec![
                (cmd_names::CONFIG_PATH.to_string(), "显示所有数据目录位置"),
                (format!("{} -o", cmd_names::CONFIG_PATH), "在资源管理器中打开配置目录"),
            ],
        },
        Group {
            cmd: "config source".into(), desc: "管理源定义（软件 + 工具）",
            entries: vec![
                (cmd_names::CONFIG_SOURCE_UPDATE.to_string(), "从远程仓库下载最新源定义"),
                (cmd_names::CONFIG_SOURCE_PATH.to_string(), "显示源定义目录路径"),
                (format!("{} -o", cmd_names::CONFIG_SOURCE_PATH), "在资源管理器中打开源目录"),
            ],
        },
        Group {
            cmd: "config speedtest".into(), desc: "测速所有下载源",
            entries: vec![
                (cmd_names::CONFIG_SPEEDTEST.to_string(), "对所有软件的所有源测速"),
                (format!("{} 7zip", cmd_names::CONFIG_SPEEDTEST), "仅对指定软件的源测速"),
                (format!("{} -S", cmd_names::CONFIG_SPEEDTEST), "以软件为单位统计可用性"),
            ],
        },
        Group {
            cmd: "config downloader".into(), desc: "管理下载引擎后端",
            entries: vec![
                (cmd_names::CONFIG_DOWNLOADER_LIST.to_string(), "列出所有下载后端及启用状态"),
                (format!("{} set curl on", cmd_names::CONFIG_DOWNLOADER), "启用 curl 后端"),
                (format!("{} set curl off", cmd_names::CONFIG_DOWNLOADER), "禁用 curl 后端"),
                (cmd_names::CONFIG_DOWNLOADER_CONFIG.to_string(), "显示配置文件路径"),
                (cmd_names::CONFIG_DOWNLOADER_CONFIG_OPEN.to_string(), "在资源管理器中打开配置目录"),
            ],
        },
        Group {
            cmd: "self init".into(), desc: "初始化 as 环境",
            entries: vec![
                (cmd_names::SELF_INIT.to_string(), "创建 tools/bin 并注册到用户 PATH"),
            ],
        },
        Group {
            cmd: "self update".into(), desc: "更新 as 自身",
            entries: vec![
                (cmd_names::SELF_UPDATE.to_string(), "下载最新版 as 并热替换"),
            ],
        },
        Group {
            cmd: "tool".into(), desc: "管理自研工具",
            entries: vec![
                (cmd_names::TOOL_LIST.to_string(), "列出已安装的自研工具"),
                (format!("{} ls", cmd_names::TOOL_REMOVE), "移除自研工具 ls"),
            ],
        },
    ];

    // 计算示例命令文本的最大显示宽度用于对齐
    let max_usage_w = examples
        .iter()
        .flat_map(|g| g.entries.iter())
        .map(|(usage, _)| usage.display_width())
        .max()
        .unwrap_or(44);

    for group in &examples {
        println!(
            "  {}  {}",
            color::bold_green(format!("{:<12}", group.cmd)),
            color::gray(group.desc)
        );
        println!();
        for (usage, explanation) in &group.entries {
            println!(
                "    {}  {}",
                color::cyan(pad(&usage, max_usage_w)),
                explanation
            );
        }
        println!();
    }
}

/// 美化输出 clap 错误信息（中文友好版）
pub fn print_clap_error(e: clap::Error) {
    use clap::error::ErrorKind;
    let info = e.to_string();

    // 剥离 ANSI 后再提取建议（clap 输出的 tip 带颜色码）
    let plain_info = color::strip_ansi(&info);
    let tip = plain_info.lines()
        .find(|l| l.contains("tip:") || l.contains("did you mean"))
        .map(|l| l.trim())
        .unwrap_or("");

    // 提取用法行
    let usage_line = plain_info
        .lines()
        .find(|l| l.starts_with("Usage:"))
        .and_then(|u| u.strip_prefix("Usage:"))
        .unwrap_or("");

    // 提取错误详情（使用纯文本，避免 ANSI 干扰）
    let lines: Vec<&str> = plain_info.lines().collect();
    let error_idx = lines.iter().position(|l| l.starts_with("error: "));
    let detail = error_idx
        .map(|i| {
            let first = lines[i].strip_prefix("error: ").unwrap_or("");
            let rest: Vec<&str> = lines[i + 1..]
                .iter()
                .take_while(|l| !l.is_empty() && !l.starts_with("Usage:") && !l.starts_with("For more") && !l.contains("tip:"))
                .map(|s| s.trim())
                .collect();
            let mut parts = vec![first.to_string()];
            parts.extend(rest.iter().map(|s| s.to_string()));
            parts.join(" ")
        })
        .unwrap_or_default();

    // 提取第一个引号内容
    let quoted = detail
        .find('\'')
        .and_then(|s| detail[s + 1..].find('\'').map(|e| &detail[s + 1..s + 1 + e]))
        .unwrap_or("")
        .to_string();

    let msg = match e.kind() {
        ErrorKind::InvalidSubcommand => {
            if !quoted.is_empty() {
                format!("无法识别的子命令 \"{}\"", quoted)
            } else {
                detail
            }
        }
        ErrorKind::UnknownArgument => {
            if !quoted.is_empty() {
                format!("无法识别的参数 {}", quoted)
            } else {
                detail
            }
        }
        ErrorKind::MissingRequiredArgument => {
            if let Some(missing) = detail
                .strip_prefix("the following required arguments were not provided:")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                format!("缺少必需参数: {}", missing)
            } else if detail.contains("requires a subcommand") {
                "缺少子命令，请查看上方用法".to_string()
            } else if detail.is_empty() {
                "缺少必需参数".to_string()
            } else {
                detail
            }
        }
        ErrorKind::InvalidValue => {
            if !quoted.is_empty() {
                format!("参数 {} 的值无效", quoted)
            } else {
                detail
            }
        }
        ErrorKind::ValueValidation => format!("参数校验失败: {}", detail),
        ErrorKind::TooManyValues => {
            if !quoted.is_empty() {
                format!("参数 {} 的值过多", quoted)
            } else {
                detail
            }
        }
        ErrorKind::TooFewValues => {
            if !quoted.is_empty() {
                format!("参数 {} 的值不足", quoted)
            } else {
                detail
            }
        }
        ErrorKind::ArgumentConflict => format!("参数冲突: {}", detail),
        ErrorKind::DisplayHelp => {
            let _ = e.print();
            return;
        }
        ErrorKind::DisplayVersion => {
            let _ = e.print();
            return;
        }
        _ => detail,
    };

    eprintln!("{} {}", color::red("错误:"), msg);
    if !tip.is_empty() {
        eprintln!("  {}", color::cyan(tip));
    }
    if !usage_line.is_empty() {
        eprintln!("{} {}", color::bold_yellow("用法:"), usage_line);
    }
    eprintln!("{}", color::gray("更多帮助请运行 --help"));
}

/// 执行闭包，统一捕获并输出错误
pub fn run<F: FnOnce() -> anyhow::Result<()>>(f: F) -> anyhow::Result<()> {
    if let Err(e) = f() {
        eprintln!("{} {}", color::red("错误:"), e);
    }
    Ok(())
}
