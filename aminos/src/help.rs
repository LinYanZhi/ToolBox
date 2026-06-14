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

pub const HELP_TEMPLATE: &str = "\
\x1b[1;36maminos\x1b[0m — \x1b[32m轻量级 Windows 软件包管理器\x1b[0m

\x1b[1;33m用法:\x1b[0m
  \x1b[36mas\x1b[0m \x1b[32m<命令>\x1b[0m [参数]

\x1b[1;33m命令:\x1b[0m
{subcommands}
\x1b[1;33m选项:\x1b[0m
{options}

\x1b[1;33m示例:\x1b[0m
  \x1b[36mas list\x1b[0m
  \x1b[36mas install 7zip\x1b[0m
  \x1b[36mas uninstall 7zip\x1b[0m

\x1b[1;33m提示:\x1b[0m
  更多示例请运行 \x1b[36mas -e\x1b[0m
";

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
        ]),
        ("list", "列出可用软件及安装状态", &[
            ("as list", "列出所有软件"),
            ("as list -g", "按分类分组显示"),
            ("as list --categories", "查看分类概览"),
            ("as list -i", "仅显示已安装的软件"),
            ("as list -m", "仅显示未安装的软件"),
            ("as list -f 开发工具", "按分类过滤（如: 工具, 开发, 办公, 浏览器）"),
            ("as list -S 压缩", "搜索名称、别名或描述"),
            ("as list -S python", "搜索名称包含 python 的软件"),
            ("as list -D", "仅显示已下载缓存的软件"),
            ("as list --downloading", "仅显示正在下载的软件"),
            ("as list --no-download", "仅显示未下载的软件"),
        ]),
        ("info", "查看软件详细信息", &[
            ("as info 7zip", "查看 7-Zip 的基本信息"),
            ("as info 7zip --urls", "查看所有可用下载地址"),
        ]),
        ("uninstall", "卸载指定软件", &[
            ("as uninstall 7zip", "静默卸载 7-Zip"),
            ("as uninstall vscode python", "同时卸载多个软件"),
            ("as uninstall 7zip --gui", "使用图形界面卸载向导"),
            ("as uninstall 7zip --force", "强制删除（跳过卸载器）"),
        ]),
        ("cache", "查看已下载的缓存文件", &[
            ("as cache", "查看缓存文件列表和一致性"),
            ("as cache --clear", "清除所有缓存文件"),
            ("as cache --open", "在资源管理器中打开缓存目录"),
        ]),
        ("upgrade", "升级所有已安装的软件", &[
            ("as upgrade", "升级所有已安装的软件"),
            ("as upgrade 7zip", "仅升级指定软件"),
            ("as upgrade --check", "仅检查更新，不下载不安装"),
            ("as upgrade --renew", "强制重新下载（即使版本相同）"),
        ]),
        ("source", "管理软件源定义", &[
            ("as source update", "从远程仓库下载最新源定义"),
            ("as source path", "显示源定义目录路径"),
            ("as source path --open", "在资源管理器中打开源目录"),
            ("as source dirs", "显示所有数据目录位置"),
            ("as source speedtest", "对所有软件的所有源测速"),
            ("as source speedtest 7zip", "仅对指定软件的源测速"),
            ("as source speedtest -S", "以软件为单位统计可用性"),
        ]),
        ("init", "初始化 as 环境", &[
            ("as init", "创建 tools/bin 并注册到用户 PATH"),
        ]),
        ("self-update", "更新 as 自身", &[
            ("as self-update", "下载最新版 as 并热替换"),
        ]),
        ("tool", "管理自研工具", &[
            ("as tool list", "列出已安装的自研工具"),
            ("as tool remove ls", "移除自研工具 ls"),
        ]),
        ("downloader", "管理下载引擎后端", &[
            ("as downloader list", "列出所有下载后端及启用状态"),
            ("as downloader set curl on", "启用 curl 后端"),
            ("as downloader set curl off", "禁用 curl 后端"),
            ("as downloader config", "显示配置文件路径"),
            ("as downloader config --open", "在资源管理器中打开配置目录"),
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

    // Extract error detail: merge all lines after "error: " (multi-line support)
    let lines: Vec<&str> = info.lines().collect();
    let error_idx = lines.iter().position(|l| l.starts_with("error: "));
    let detail = error_idx
        .map(|i| {
            let first = lines[i].strip_prefix("error: ").unwrap_or("");
            let rest: Vec<&str> = lines[i + 1..]
                .iter()
                .take_while(|l| !l.is_empty() && !l.starts_with("Usage:") && !l.starts_with("For more"))
                .map(|s| s.trim())
                .collect();
            let mut parts = vec![first.to_string()];
            parts.extend(rest.iter().map(|s| s.to_string()));
            parts.join(" ")
        })
        .unwrap_or_default();

    // Extract the first quoted string from the detail (e.g. '-b', '--flag', '<ARG>')
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
            } else {
                "缺少必需参数".to_string()
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

    let usage_line = info
        .lines()
        .find(|l| l.starts_with("Usage:"))
        .and_then(|u| u.strip_prefix("Usage:"))
        .unwrap_or("");

    eprintln!("{} {}", color::red("错误:"), msg);
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
