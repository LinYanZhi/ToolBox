use clap::builder::styling::{AnsiColor, Color, Style, Styles};

use color::pad_left as pad;
use color::DisplayWidth;

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
    println!();
    println!("  {} — {}", color::bold_cyan("as config"), color::green("管理 as 环境和配置"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as config"), color::green("<子命令>"), color::gray("[参数]"));
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

    let examples: &[(&str, &str)] = &[
        ("as config path",          "查看所有数据目录位置"),
        ("as config path -o",       "在资源管理器中打开配置目录"),
        ("as config cache",         "查看缓存文件"),
        ("as config cache --clear", "清除所有缓存"),
        ("as config source update", "更新软件源和工具源"),
        ("as config speedtest",     "测速下载源"),
        ("as config downloader list", "列出下载后端"),
    ];

    let max_w = examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(32);

    for (cmd, desc) in examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
}

/// 打印 as config source 子命令帮助
pub fn print_source_help() {
    println!();
    println!("  {} — {}", color::bold_cyan("as config source"), color::green("管理软件源和工具源定义"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as config source"), color::green("<子命令>"), color::gray("[参数]"));
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
    println!("  {} — {}", color::bold_cyan("as config downloader"), color::green("管理下载引擎后端"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as config downloader"), color::green("<子命令>"), color::gray("[参数]"));
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
    println!("  {} — {}", color::bold_cyan("as self"), color::green("管理 as 自身"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as self"), color::green("<子命令>"), color::gray("[参数]"));
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

    let self_examples: &[(&str, &str)] = &[
        ("as self init",   "初始化环境"),
        ("as self update", "更新自身"),
    ];

    let max_w = self_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(20);

    for (cmd, desc) in self_examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
}

/// 打印 as tool 子命令帮助
pub fn print_tool_help() {
    println!();
    println!("  {} — {}", color::bold_cyan("as tool"), color::green("管理自研工具"));
    println!();
    println!("  {}", color::bold_yellow("用法:"));
    println!("    {} {} {}", color::cyan("as tool"), color::green("<子命令>"), color::gray("[参数]"));
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

    let tool_examples: &[(&str, &str)] = &[
        ("as tool install ls",        "安装 ls 工具"),
        ("as tool install ls uv",      "同时安装多个工具"),
        ("as tool list",              "列出自研工具"),
        ("as tool remove ls",         "移除 ls 工具"),
        ("as tool upgrade",           "升级所有自研工具"),
    ];

    let max_w = tool_examples.iter().map(|(e, _)| e.display_width()).max().unwrap_or(28);

    for (cmd, desc) in tool_examples {
        println!("    {}  {}",
            pad(&color::cyan(cmd), max_w),
            desc);
    }
    println!();
    println!("  {}", color::gray("第三方软件请使用 as install 命令"));
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

    let examples: &[(&str, &str, &[(&str, &str)])] = &[
        ("install", "安装指定软件", &[
            ("as install 7zip", "安装 7-Zip（最新版）"),
            ("as install vscode python git", "同时安装多个软件"),
            ("as install 7zip -v 1.0.0", "安装指定版本"),
            ("as install 7zip --gui", "使用图形界面向导安装"),
            ("as install 7zip --renew", "强制重新下载并安装"),
            ("as install 7zip --download-only", "仅下载，不安装"),
            ("as install 7zip --type portable", "指定安装类型为便携版"),
        ]),
        ("list", "列出可用软件及安装状态", &[
            ("as list", "列出所有软件"),
            ("as list -g", "按分类分组显示"),
            ("as list --categories", "查看分类概览"),
            ("as list -i", "仅显示已安装的软件"),
            ("as list -m", "仅显示未安装的软件"),
            ("as list -f 开发工具", "按分类过滤"),
            ("as list -s 压缩", "搜索名称、别名或描述"),
            ("as list -d", "仅显示已下载缓存的软件"),
            ("as list --info 7zip", "查看 7-Zip 的详细信息"),
            ("as list --info 7zip --urls", "查看 7-Zip 所有下载地址"),
        ]),
        ("uninstall", "卸载指定软件", &[
            ("as uninstall 7zip", "静默卸载 7-Zip"),
            ("as uninstall vscode python", "同时卸载多个软件"),
            ("as uninstall 7zip --gui", "使用图形界面卸载向导"),
            ("as uninstall 7zip --force", "强制删除（跳过卸载器）"),
        ]),
        ("upgrade", "升级已安装的软件", &[
            ("as upgrade", "升级所有已安装的软件"),
            ("as upgrade 7zip", "仅升级指定软件"),
            ("as upgrade --check", "仅检查更新，不下载不安装"),
            ("as upgrade --renew", "强制重新下载（即使版本相同）"),
        ]),
        ("config cache", "管理下载缓存", &[
            ("as config cache", "查看缓存文件列表和一致性"),
            ("as config cache --clear", "清除所有缓存文件"),
            ("as config cache --open", "在资源管理器中打开缓存目录"),
        ]),
        ("config path", "显示/打开配置目录", &[
            ("as config path", "显示所有数据目录位置"),
            ("as config path -o", "在资源管理器中打开配置目录"),
        ]),
        ("config source", "管理源定义（软件 + 工具）", &[
            ("as config source update", "从远程仓库下载最新源定义"),
            ("as config source path", "显示源定义目录路径"),
            ("as config source path -o", "在资源管理器中打开源目录"),
        ]),
        ("config speedtest", "测速所有下载源", &[
            ("as config speedtest", "对所有软件的所有源测速"),
            ("as config speedtest 7zip", "仅对指定软件的源测速"),
            ("as config speedtest -S", "以软件为单位统计可用性"),
        ]),
        ("config downloader", "管理下载引擎后端", &[
            ("as config downloader list", "列出所有下载后端及启用状态"),
            ("as config downloader set curl on", "启用 curl 后端"),
            ("as config downloader set curl off", "禁用 curl 后端"),
            ("as config downloader config", "显示配置文件路径"),
            ("as config downloader config -o", "在资源管理器中打开配置目录"),
        ]),
        ("self init", "初始化 as 环境", &[
            ("as self init", "创建 tools/bin 并注册到用户 PATH"),
        ]),
        ("self update", "更新 as 自身", &[
            ("as self update", "下载最新版 as 并热替换"),
        ]),
        ("tool", "管理自研工具", &[
            ("as tool list", "列出已安装的自研工具"),
            ("as tool remove ls", "移除自研工具 ls"),
        ]),
    ];

    // 计算示例命令文本的最大显示宽度用于对齐
    let max_usage_w = examples
        .iter()
        .flat_map(|(_, _, entries)| entries.iter())
        .map(|(usage, _)| (*usage).display_width())
        .max()
        .unwrap_or(44);

    for (cmd, desc, entries) in examples {
        println!(
            "  {}  {}",
            color::bold_green(format!("{:<12}", cmd)),
            color::gray(desc)
        );
        println!();
        for (usage, explanation) in *entries {
            println!(
                "    {}  {}",
                color::cyan(pad(usage, max_usage_w)),
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
