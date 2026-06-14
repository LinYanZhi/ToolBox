mod downloader;
mod installer;
mod paths;
mod pe_version;
mod registry;
mod software;
mod speedtest;

use std::fs;

use anyhow::{bail, Context};
use clap::{CommandFactory, Parser, Subcommand};
use clap::builder::styling::{AnsiColor, Color, Style, Styles};

use color::{DisplayWidth, format_size, pad_left as pad, truncate};

/// 统一帮助配色方案
const HELP_STYLES: Styles = Styles::styled()
    .header(*Styles::styled().get_header())
    .usage(Style::new().bold().fg_color(Some(Color::Ansi(AnsiColor::Yellow))))
    .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
    .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
    .error(*Styles::styled().get_error());

const HELP_TEMPLATE: &str = "\
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
const HELP_TEMPLATE_OPTIONS: &str = "\
{about}

\x1b[1;33m用法:\x1b[0m
  {usage}

\x1b[1;33m选项:\x1b[0m
{options}";

/// 含子命令的帮助模板
const HELP_TEMPLATE_SUBCMDS: &str = "\
{about}

\x1b[1;33m用法:\x1b[0m
  {usage}

\x1b[1;33m命令:\x1b[0m
{subcommands}
\x1b[1;33m选项:\x1b[0m
{options}";

/// AminOS - lightweight software package manager
#[derive(Parser)]
#[command(
    name = "as",
    version,
    about,
    color = clap::ColorChoice::Always,
    help_template = HELP_TEMPLATE,
    styles = HELP_STYLES,
    disable_help_subcommand = true,
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// 显示所有命令的示例用法
    #[arg(short = 'e', long = "example")]
    example: bool,
}

#[derive(Subcommand)]
enum Command {
    /// 安装指定软件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Install {
        /// 软件名称（可同时指定多个）
        #[arg(required = true)]
        names: Vec<String>,
        /// 指定版本号
        #[arg(short, long)]
        version: Option<String>,
        /// 使用图形界面安装（不静默）
        #[arg(long)]
        gui: bool,
        /// 强制重新下载
        #[arg(long)]
        renew: bool,
        /// 仅下载，不安装
        #[arg(long = "download-only")]
        download_only: bool,
    },
    /// 列出可用软件及安装状态
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    List {
        /// 按分类过滤
        #[arg(short, long)]
        filter: Option<String>,
        /// 仅显示已安装
        #[arg(long = "install", short = 'i')]
        install_only: bool,
        /// 仅显示未安装
        #[arg(long = "missing", short = 'm')]
        missing: bool,
        /// 搜索软件名、别名或描述
        #[arg(long = "search", short = 'S')]
        search: Option<String>,
        /// 仅显示已下载
        #[arg(short = 'D', long)]
        downloaded: bool,
        /// 仅显示下载中
        #[arg(long)]
        downloading: bool,
        /// 仅显示未下载
        #[arg(long = "no-download")]
        no_download: bool,
        /// 按分类分组显示
        #[arg(short = 'g', long)]
        group: bool,
        /// 显示所有分类概览
        #[arg(long)]
        categories: bool,
    },
    /// 查看软件详细信息
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Info {
        /// 软件名称
        #[arg(required = true)]
        name: String,
        /// 显示所有下载地址
        #[arg(short, long)]
        urls: bool,
    },
    /// 卸载指定软件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Uninstall {
        #[arg(required = true)]
        names: Vec<String>,
        /// 使用图形界面卸载
        #[arg(long)]
        gui: bool,
        /// 强制删除
        #[arg(short, long)]
        force: bool,
    },
    /// 查看已下载的缓存文件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Cache {
        /// 清除所有缓存文件
        #[arg(short, long)]
        clear: bool,
        /// 在资源管理器中打开缓存目录
        #[arg(short, long)]
        open: bool,
    },
    /// 升级所有已安装的软件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Upgrade {
        /// 可选：仅升级指定软件（不指定则全部升级）
        names: Vec<String>,
        /// 仅检查更新，不下也不装
        #[arg(short, long)]
        check: bool,
        /// 强制重新下载（即使版本相同）
        #[arg(long)]
        renew: bool,
    },
    /// 管理软件源定义
    #[command(help_template = HELP_TEMPLATE_SUBCMDS)]
    Source {
        #[command(subcommand)]
        action: SourceCmd,
    },
    /// 初始化 as 环境（创建 tools/bin 并注册到 PATH）
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Init,
    /// 更新 as 自身到最新版本
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    SelfUpdate,
    /// 管理自研工具（已安装的 as 工具集）
    #[command(help_template = HELP_TEMPLATE_SUBCMDS)]
    Tool {
        #[command(subcommand)]
        action: ToolCmd,
    },
    /// 管理下载引擎后端
    #[command(help_template = HELP_TEMPLATE_SUBCMDS)]
    Downloader {
        #[command(subcommand)]
        action: DownloaderCmd,
    },
}

#[derive(Subcommand)]
enum SourceCmd {
    /// 从远程仓库下载最新源定义
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Update,
    /// 显示当前源目录路径
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Path {
        /// 在资源管理器中打开
        #[arg(short, long)]
        open: bool,
    },
    /// 显示所有数据目录位置
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Dirs {
        /// 在资源管理器中打开数据目录
        #[arg(short, long)]
        open: bool,
    },
    /// 测速所有下载源
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Speedtest {
        /// 可选：仅测速指定软件
        name: Vec<String>,
        /// 以软件为单位统计（任一源可用即为通）
        #[arg(short = 'S', long = "software")]
        software: bool,
    },
}

#[derive(Subcommand)]
enum ToolCmd {
    /// 列出已安装的自研工具
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    List,
    /// 移除一个自研工具（同 as uninstall）
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Remove {
        /// 工具名称
        #[arg(required = true)]
        name: String,
    },
}

#[derive(Subcommand)]
enum DownloaderCmd {
    /// 列出所有下载后端及其启用状态
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    List,
    /// 启用或禁用一个后端
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Set {
        /// 后端名称（如 curl, RustRange, Aria2c）
        name: String,
        /// on 或 off
        state: String,
    },
    /// 显示或打开配置文件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Config {
        /// 在资源管理器中打开配置目录
        #[arg(short, long)]
        open: bool,
    },
}

fn main() {
    color::enable_ansi();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            print_clap_error(e);
            return;
        }
    };
    match cli.command {
        None => {
            if cli.example {
                run_example();
                return;
            }
            let _ = Cli::command().print_help();
        }
        Some(Command::Install { names, version, gui, renew, download_only }) => {
            let _ = run(|| run_install(names, version.unwrap_or_default(), gui, renew, download_only));
        }
        Some(Command::Uninstall { names, gui, force }) => {
            let _ = run(|| run_uninstall(names, gui, force));
        }
        Some(Command::List { filter, install_only, missing, search, downloaded, downloading, no_download, group, categories }) => {
            let _ = run(|| run_list(filter, install_only, missing, search, downloaded, downloading, no_download, group, categories));
        }
        Some(Command::Info { name, urls }) => {
            let _ = run(|| run_info(&name, urls));
        }
        Some(Command::Source { action }) => {
            let _ = run(|| run_source(action));
        }
        Some(Command::Cache { clear, open }) => {
            let _ = run(|| run_cache(clear, open));
        }
        Some(Command::Upgrade { names, check, renew }) => {
            let _ = run(|| run_upgrade(names, check, renew));
        }
        Some(Command::Init) => {
            let _ = run(run_init);
        }
        Some(Command::SelfUpdate) => {
            let _ = run(run_self_update);
        }
        Some(Command::Tool { action }) => {
            let _ = run(|| run_tool(action));
        }
        Some(Command::Downloader { action }) => {
            let _ = run(|| run_downloader(action));
        }
    }
}

/// 显示所有命令的详细示例用法
fn run_example() {
    println!();
    println!("  {}",
        color::bold_cyan("aminos 命令参考手册"));
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
    let max_usage_w = examples.iter()
        .flat_map(|(_, _, entries)| entries.iter())
        .map(|(usage, _)| (*usage).display_width())
        .max()
        .unwrap_or(44);

    for (cmd, desc, entries) in examples {
        println!("  {}  {}",
            color::bold_green(format!("{:<12}", cmd)),
            color::gray(desc));
        println!();
        for (usage, explanation) in *entries {
            println!("    {}  {}",
                color::cyan(pad(usage, max_usage_w)),
                explanation);
        }
        println!();
    }
}

fn print_clap_error(e: clap::Error) {
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

fn run<F: FnOnce() -> anyhow::Result<()>>(f: F) -> anyhow::Result<()> {
    if let Err(e) = f() {
        eprintln!("{} {}", color::red("错误:"), e);
    }
    Ok(())
}

// ── install ───────────────────────────────────────────────

fn run_install(names: Vec<String>, version: String, gui: bool, renew: bool, download_only: bool) -> anyhow::Result<()> {
    for name in &names {
        let n = name.to_lowercase();
        if let Err(e) = installer::install_software(&n, &version, gui, renew, download_only) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
        }
    }
    Ok(())
}

fn run_upgrade(names: Vec<String>, check_only: bool, renew: bool) -> anyhow::Result<()> {
    let installed_db = software::read_installed_db().unwrap_or_default();

    let targets: Vec<String> = if names.is_empty() {
        installed_db.keys().cloned().collect()
    } else {
        names.into_iter().map(|n| n.to_lowercase()).collect()
    };

    if targets.is_empty() {
        println!("没有已安装的软件需要升级。");
        return Ok(());
    }

    let mut updated = 0u32;
    let mut up_to_date = 0u32;
    let mut failed = 0u32;

    for name in &targets {
        let sd = match software::read_software_def(name) {
            Ok(sd) => sd,
            Err(e) => {
                eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
                failed += 1;
                continue;
            }
        };

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let source_ver = &sd.default_version;

        // 判断是否为便携版
        let is_portable = sd.versions.get(source_ver)
            .map(|vi| vi.installer_type == "portable")
            .unwrap_or(false);

        // 便携版：检查 apps/{name}-{version} 目录是否存在
        // 标准版：从注册表获取版本
        let portable_dir = paths::apps_dir().join(format!("{}-{}", name, source_ver));
        let current_ver: String = if is_portable {
            if portable_dir.is_dir() {
                installed_db.get(name)
                    .map(|rec| rec.version.clone())
                    .unwrap_or_else(|| source_ver.to_string())
            } else {
                // 目录已被手动删除
                String::new()
            }
        } else {
            // 标准安装：从注册表获取版本
            let registry_ver = sd.versions.get(source_ver)
                .and_then(|vi| vi.detection.as_ref())
                .and_then(|d| registry::detect_installed(d))
                .and_then(|r| r.get("DisplayVersion").cloned());
            installed_db.get(name)
                .map(|rec| rec.version.clone())
                .or(registry_ver)
                .unwrap_or_default()
        };

        if current_ver == *source_ver && !renew {
            println!("  {}", color::gray(format!("{} {} 已是最新", display, current_ver)));
            up_to_date += 1;
            continue;
        }

        if check_only {
            println!("  {} → {} 可更新",
                color::yellow(format!("{} {}", display, current_ver)),
                color::green(source_ver));
            updated += 1;
            continue;
        }

        println!("  ▶ {} {} → {} ...", display, current_ver, source_ver);
        match installer::install_software(name, "", false, renew, false) {
            Ok(()) => {
                updated += 1;
            }
            Err(e) => {
                eprintln!("  {}: {}", color::yellow(format!("升级 {} 失败", display)), e);
                failed += 1;
            }
        }
    }

    println!();
    if check_only {
        println!("{}",
            color::gray(format!("共检查 {} 个，{} 个可更新，{} 个最新，{} 个失败",
                targets.len(), updated, up_to_date, failed)));
    } else {
        println!("{}",
            color::gray(format!("共 {} 个，{} 个已升级，{} 个已最新，{} 个失败",
                targets.len(), updated, up_to_date, failed)));
    }
    Ok(())
}

// ── init ─────────────────────────────────────────────────

/// 初始化 as 环境：创建 tools/bin 目录并注册到 PATH。
fn run_init() -> anyhow::Result<()> {
    let bin_dir = paths::tools_bin_dir();
    fs::create_dir_all(&bin_dir)?;
    println!("✓ 已创建: {}", bin_dir.display());

    let bin_path = bin_dir.to_string_lossy().to_string();

    // 通过 PowerShell 读取注册表中的用户 PATH（不包含当前会话的临时修改）
    let ps_get_path = format!(
        "[Environment]::GetEnvironmentVariable('PATH', 'User')"
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_get_path])
        .output()
        .context("读取注册表 PATH 失败")?;
    let user_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let already_in_path = user_path.split(';')
        .any(|p| p.trim().to_lowercase() == bin_path.to_lowercase());

    if already_in_path {
        println!("✓ {} 已在用户 PATH 中", bin_dir.display());
        return Ok(());
    }

    // 将新路径追加到用户 PATH（不携带当前会话临时修改）
    let new_path = if user_path.is_empty() {
        bin_path.clone()
    } else {
        format!("{};{}", user_path, bin_path)
    };

    let ps_set_path = format!(
        "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
        new_path.replace('\'', "''")
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_set_path])
        .status()
        .context("注册 PATH 失败")?;

    if status.success() {
        println!("✓ 已将 {} 添加到用户 PATH", bin_dir.display());

        // 同时更新当前进程的 PATH，立即生效（无需重启终端）
        if let Ok(current_path) = std::env::var("PATH") {
            // SAFETY: 单线程环境，无其他线程并发读取 PATH
            unsafe { std::env::set_var("PATH", format!("{};{}", bin_path, current_path)); }
            println!("✓ 当前终端已生效，可直接使用工具");
        }
    } else {
        bail!("添加 PATH 失败");
    }

    println!("\n✓ as 环境初始化完成");
    println!("  现在可通过 {} 安装自研工具", color::cyan("as install <工具名>"));
    Ok(())
}

// ── self-update ───────────────────────────────────────────

/// 更新 as 自身到最新版本。
fn run_self_update() -> anyhow::Result<()> {
    println!("正在检查 as 更新...");

    // 读取 as 自身的源定义
    let sd = software::read_software_def("as")?;
    let ver = &sd.default_version;
    let vi = sd.versions.get(ver)
        .ok_or_else(|| anyhow::anyhow!("as 版本 {} 未定义", ver))?;

    println!("  发现最新版本: {}", color::cyan(ver));

    // 下载新版 as.zip
    let dl = paths::downloads_dir();
    fs::create_dir_all(&dl)?;

    let zip_name = format!("as-{}.zip", ver);
    let zip_path = dl.join(&zip_name);
    net::download::download_with_url_fallback("as", &vi.urls, &zip_path, &net::DownloadConfig::default())?;

    // 解压到临时目录
    let temp_dir = dl.join(format!("as-update-{}", ver));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    installer::extract_zip_to(&zip_path, &temp_dir)?;

    // 找到 as.exe
    let new_exe = temp_dir.join("as.exe");
    if !new_exe.is_file() {
        bail!("更新包中未找到 as.exe");
    }

    // 获取当前 as.exe 路径
    let current_exe = std::env::current_exe()
        .context("无法获取当前程序路径")?;

    println!("  正在更新: {}", current_exe.display());

    // 通过 PowerShell 脚本热替换（Windows 下不能直接覆盖正在运行的程序）
    let ps_script = format!(
        r#"
Start-Sleep -Seconds 1
Copy-Item -Path '{}' -Destination '{}' -Force
Remove-Item -Path '{}' -Recurse -Force
Write-Host '✓ as 已更新到 {}'
"#,
        new_exe.display(),
        current_exe.display(),
        temp_dir.display(),
        ver,
    );

    let ps_path = dl.join("update-as.ps1");
    fs::write(&ps_path, ps_script)?;

    // 启动 PowerShell 脚本，然后退出当前进程
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", &ps_path.to_string_lossy()])
        .spawn()?;

    println!("  ✓ 更新脚本已启动，as 将在重启后完成更新");
    println!("  当前终端可继续使用");

    Ok(())
}

// ── tool ──────────────────────────────────────────────────

fn run_tool(action: ToolCmd) -> anyhow::Result<()> {
    match action {
        ToolCmd::List => {
            let defs = software::list_software_defs().unwrap_or_default();
            let db = software::read_installed_db().unwrap_or_default();

            // 筛选所有自研工具（kind=self）
            let self_tools: Vec<&software::SoftwareDef> = defs.iter()
                .filter(|d| d.kind == "self")
                .collect();

            if self_tools.is_empty() {
                println!("没有可用的自研工具。");
                return Ok(());
            }

            println!("{}", color::bold_cyan("可用自研工具:"));
            println!();

            let max_name_w = self_tools.iter()
                .map(|d| d.display_name.as_str())
                .max_by_key(|n| {
                    use color::DisplayWidth;
                    n.display_width()
                })
                .map(|n| {
                    use color::DisplayWidth;
                    n.display_width()
                })
                .unwrap_or(10)
                .max(10);

            for def in &self_tools {
                let name = if def.display_name.is_empty() { &def.name } else { &def.display_name };
                let ver = &def.default_version;
                let installed = db.contains_key(&def.name);
                let status = if installed {
                    color::green(format!("{}  {}", color::bold_green("已安装"), color::cyan(ver)))
                } else {
                    color::gray("未安装".to_string())
                };
                println!("  {}  {}  {}", color::cyan(pad(name, max_name_w)), status, color::gray(&def.description));
            }
        }
        ToolCmd::Remove { name } => {
            installer::uninstall_software(&name, false, false)?;
        }
    }
    Ok(())
}

fn run_downloader(action: DownloaderCmd) -> anyhow::Result<()> {
    match action {
        DownloaderCmd::List => {
            downloader::run_downloader_list()?;
        }
        DownloaderCmd::Set { name, state } => {
            let enable = match state.as_str() {
                "on" | "启用" | "enable" | "1" => true,
                "off" | "禁用" | "disable" | "0" => false,
                _ => {
                    eprintln!("    {} 无效状态: {}（使用 on/off）", color::red("错误"), state);
                    std::process::exit(1);
                }
            };
            downloader::run_downloader_set(&name, enable)?;
        }
        DownloaderCmd::Config { open } => {
            downloader::run_downloader_config(open)?;
        }
    }
    Ok(())
}

fn run_uninstall(names: Vec<String>, gui: bool, force: bool) -> anyhow::Result<()> {
    for name in &names {
        let n = name.to_lowercase();
        if let Err(e) = installer::uninstall_software(&n, gui, force) {
            eprintln!("  {} {}: {}", color::yellow("跳过"), name, e);
        }
    }
    Ok(())
}

// ── list (matches Python ListCommand) ─────────────────────

/// 扫描下载缓存目录，返回 {软件名 → (状态, 颜色)} 的映射
fn scan_download_cache() -> std::collections::HashMap<String, (&'static str, &'static str)> {
    let mut result = std::collections::HashMap::new();
    let downloads = paths::downloads_dir();
    if !downloads.is_dir() {
        return result;
    }

    // 加载源定义，用于精确的软件名匹配
    let defs = software::list_software_defs().unwrap_or_default();

    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }

            let fname = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // 提取软件名
            let (raw_name, is_downloading) = if let Some(stripped) = fname.strip_suffix(".downloading") {
                (stripped.to_string(), true)
            } else {
                (fname.clone(), false)
            };

            // 从文件名匹配软件名：检查每个源名是否作为文件名前缀
            let name_part = defs.iter()
                .find(|sd| {
                    let prefix = format!("{}-", sd.name);
                    raw_name.starts_with(&prefix)
                })
                .map(|sd| sd.name.clone())
                .or_else(|| {
                    // 回退：取第一个 hyphen 前的内容
                    raw_name.find('-')
                        .map(|pos| raw_name[..pos].to_string())
                })
                .unwrap_or_default();

            if name_part.is_empty() || name_part == raw_name {
                continue;
            }

            let (status, color_code) = if is_downloading {
                ("下载中", color::ansi::YELLOW) // 黄色
            } else {
                ("已下载", color::ansi::CYAN) // 青色
            };

            // 优先保留"已下载"状态（覆盖"下载中"）
            let entry = result.entry(name_part).or_insert((status, color_code));
            if !is_downloading {
                *entry = (status, color_code);
            }
        }
    }

    result
}

fn run_list(
    filter: Option<String>, install_only: bool, missing: bool,
    search: Option<String>, downloaded: bool, downloading: bool, no_download: bool,
    group: bool, categories: bool,
) -> anyhow::Result<()> {
    // Auto-init: if source dir is empty, suggest `as source update`
    let source = paths::source_dir();
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("{}", color::yellow("  未找到源定义。首次使用请运行:"));
        println!("  as source update\n");
        return Ok(());
    }

    let reg_installed = registry::scan_all_installed();
    let installed_db = software::read_installed_db().unwrap_or_default();
    let defs = software::list_software_defs()?;
    let dl_cache = scan_download_cache();

    // ── 类别概览模式 ──
    if categories {
        let mut cat_count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for sd in &defs {
            let c = if sd.category.is_empty() { "未分类" } else { &sd.category };
            *cat_count.entry(c).or_insert(0) += 1;
        }
        let mut sorted: Vec<(&&str, &usize)> = cat_count.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));

        let max_cat_w = sorted.iter().map(|(c, _)| c.display_width()).max().unwrap_or(4).max(4).min(20);
        let max_num = sorted.iter().map(|(_, n)| n.to_string().len()).max().unwrap_or(2);
        let bar_max: usize = 30;
        let total: usize = defs.len();

        println!("\n{}", color::bold_yellow("类别概览"));
        println!("{}", color::gray("─".repeat(50)));
        for &(cat, count) in &sorted {
            let bar_w = (*count as f64 / total as f64 * bar_max as f64).round() as usize;
            let bar = "█".repeat(bar_w.max(1));
            println!("  {}  {:>w$}  {}",
                pad(cat, max_cat_w + 2),
                count,
                color::cyan(bar),
                w = max_num,
            );
        }
        println!("{}", color::gray("─".repeat(50)));
        println!("  共计 {} 个软件", total);
        return Ok(());
    }

    // ── 构建行数据 ──
    fn installer_marker(t: &str) -> &'static str {
        match t.to_lowercase().as_str() {
            "nsis" | "inno" | "exe" | "installer" => "EXE",
            "msi" | "appx" => "MSI",
            "portable" | "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" => "便携",
            _ => "EXE",
        }
    }

    // Rows: (名称, 版本, 安装状态, 安装颜色, 下载状态, 下载颜色,
    //        源标签, 源颜色, 分类, 安装标记)
    let mut rows: Vec<(String, String, &str, &str, &str, &str, &str, &str, String, &str)> = Vec::new();
    let mut seen_registry: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Registry entries
    for reg in &reg_installed {
        let rn = reg.get("display_name").cloned().unwrap_or_default();
        if rn.is_empty() || !seen_registry.insert(rn.clone()) {
            continue;
        }
        let (category, installer) = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            let cat = if sd.category.is_empty() { "未分类".to_string() } else { sd.category.clone() };
            let ins = sd.versions.get(&sd.default_version)
                .map(|vi| installer_marker(&vi.installer_type))
                .unwrap_or("EXE");
            (cat, ins)
        } else {
            ("其他".to_string(), "")
        };
        let has_source = defs.iter().any(|sd| name_matches(&rn, sd));
        let src_label = if has_source { "有" } else { "无" };
        let src_color = if has_source { color::ansi::GREEN } else { color::ansi::GRAY };
        let (dl_status, dl_color) = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            dl_cache.get(&sd.name).copied().unwrap_or(("未下载", color::ansi::GRAY))
        } else {
            ("未下载", color::ansi::GRAY)
        };
        let ver = if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
            installed_db.get(&sd.name)
                .map(|rec| rec.version.clone())
                .unwrap_or_else(|| reg.get("version").cloned().unwrap_or_default())
        } else {
            reg.get("version").cloned().unwrap_or_default()
        };
        rows.push((rn, ver, "已安装", color::ansi::GREEN, dl_status, dl_color, src_label, src_color, category, installer));
    }

    // 2. Source definitions not in registry
    for sd in &defs {
        let name = &sd.name;
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let category = if sd.category.is_empty() { "未分类".to_string() } else { sd.category.clone() };
        let installer = sd.versions.get(&sd.default_version)
            .map(|vi| installer_marker(&vi.installer_type))
            .unwrap_or("EXE");
        let already = reg_installed.iter().any(|r| {
            name_matches(&r.get("display_name").cloned().unwrap_or_default(), sd)
        });
        if already { continue; }
        let (dl_status, dl_color) = dl_cache.get(name)
            .copied()
            .unwrap_or(("未下载", color::ansi::GRAY));
        if let Some(rec) = installed_db.get(name) {
            rows.push((display.to_string(), rec.version.clone(),
                "已安装", color::ansi::GREEN, dl_status, dl_color, "有", color::ansi::GREEN, category, installer));
            continue;
        }
        rows.push((display.to_string(), sd.default_version.clone(),
            "未安装", color::ansi::GRAY, dl_status, dl_color, "有", color::ansi::GREEN, category, installer));
    }

    // ── 筛选 ──
    if install_only { rows.retain(|r| r.2 == "已安装"); }
    if missing      { rows.retain(|r| r.2 == "未安装"); }
    if downloaded   { rows.retain(|r| r.4 == "已下载"); }
    if downloading  { rows.retain(|r| r.4 == "下载中"); }
    if no_download  { rows.retain(|r| r.4 == "未下载"); }

    // 搜索增强：同时匹配名称、别名、描述
    if let Some(ref kw) = search {
        let kw_lower = kw.to_lowercase();
        rows.retain(|r| {
            if r.0.to_lowercase().contains(&kw_lower) {
                return true;
            }
            for sd in &defs {
                if sd.display_name == r.0 || sd.name == r.0.to_lowercase() {
                    if sd.aliases.iter().any(|a| a.to_lowercase().contains(&kw_lower)) {
                        return true;
                    }
                    if sd.description.to_lowercase().contains(&kw_lower) {
                        return true;
                    }
                }
            }
            false
        });
    }

    // 分类过滤：修复为按 category 字段过滤
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.8.to_lowercase().contains(&f_lower));
    }

    if rows.is_empty() {
        println!("没有匹配的软件。");
        return Ok(());
    }

    // Sort by name case-insensitive
    rows.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let max_name = rows.iter().map(|r| r.0.display_width()).max().unwrap_or(4).max(4).min(40);
    let max_ver = rows.iter().map(|r| r.1.display_width()).max().unwrap_or(4).max(4);

    // ── 分组显示 ──
    if group {
        let mut by_cat: std::collections::BTreeMap<String, Vec<&(String, String, &str, &str, &str, &str, &str, &str, String, &str)>> = std::collections::BTreeMap::new();
        for r in &rows {
            by_cat.entry(r.8.clone()).or_default().push(r);
        }
        for (cat, entries) in &by_cat {
            println!("\n{}  {}", color::bold_yellow(format!("{}", cat)), color::gray(format!("({})", entries.len())));
            // 子表头
            println!("  {}{}{}{}",
                pad("名称", max_name + 2),
                pad("版本", max_ver + 2),
                pad("安装", 8),
                pad("方式", 6));
            for (name, ver, _status, status_color, _dl_status, _dl_color, _src_label, _src_color, _cat, installer) in entries {
                let name_d = truncate(name, max_name);
                let ver_d = truncate(ver, max_ver + 1);
                let ins_color = match *installer {
                    "便携" => color::ansi::CYAN,
                    _ => color::ansi::RESET,
                };
                println!("  {}{}{}{}{}{}{}{}",
                    pad(&name_d, max_name + 2),
                    pad(&ver_d, max_ver + 2),
                    status_color,
                    pad(_status, 8),
                    color::ansi::RESET,
                    ins_color,
                    pad(installer, 6),
                    color::ansi::RESET,
                );
            }
        }
        println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
        return Ok(());
    }

    // ── 平铺显示（默认） ──
    println!();
    let header = format!("{}{}{}{}{}{}",
        pad("名称", max_name + 2),
        pad("版本", max_ver + 2),
        pad("下载", 8 + 1),
        pad("状态", 8 + 1),
        pad("源", 4),
        pad("方式", 6));
    println!("{}", header);
    println!("{}", "-".repeat(header.display_width()));

    for (name, ver, _status, status_color, dl_status, dl_color, src_label, src_color, _cat, installer) in &rows {
        let name_d = truncate(name, max_name);
        let ver_d = truncate(ver, max_ver + 1);
        let ins_color = match *installer {
            "便携" => color::ansi::CYAN,
            _ => color::ansi::RESET,
        };
        println!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            pad(&name_d, max_name + 2),
            pad(&ver_d, max_ver + 2),
            dl_color,
            pad(dl_status, 8),
            color::ansi::RESET,
            " ",
            status_color,
            pad(_status, 8),
            color::ansi::RESET,
            " ",
            src_color,
            pad(src_label, 4),
            ins_color,
            pad(installer, 6),
            color::ansi::RESET,
        );
    }

    println!("\n{}", color::gray(format!("共 {} 项", rows.len())));
    Ok(())
}

// ── info (matches Python InfoCommand) ─────────────────────

fn run_info(name: &str, show_urls: bool) -> anyhow::Result<()> {
    let name_lower = name.to_lowercase();
    let sd = software::read_software_def(&name_lower)?;
    let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };

    // --urls 模式：仅列出所有下载地址
    if show_urls {
        println!("{}", color::green(format!("{}", display)));
        let mut sorted_versions: Vec<&String> = sd.versions.keys().collect();
        sorted_versions.sort_by(|a, b| {
            let a_segs: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
            let b_segs: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
            for i in 0..a_segs.len().max(b_segs.len()) {
                let av = a_segs.get(i).copied().unwrap_or(0);
                let bv = b_segs.get(i).copied().unwrap_or(0);
                match bv.cmp(&av) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
            b.cmp(a)
        });
        for vk in &sorted_versions {
            let vi = &sd.versions[*vk];
            let expanded = net::download::expand_github_urls(&vi.urls);
            println!("  {}", color::cyan(format!("{}", vk)));
            for url in &expanded {
                println!("    {}", url);
            }
        }
        return Ok(());
     }

     let desc = &sd.description;
    let category = &sd.category;
    let homepage = &sd.homepage;
    let default_ver = &sd.default_version;

    println!("{}", color::green(format!("{}", display)));
    if !desc.is_empty() {
        println!("  {}", desc);
    }
    println!();

    // Identifier & aliases
    let primary_id = &sd.name;
    let aliases = &sd.aliases;
    let mut id_line = format!("  {}   {}", color::gray("标识符:"), primary_id);
    if !aliases.is_empty() {
        id_line.push_str(&format!("  ({})", aliases.join(", ")));
    }
    println!("{}", id_line);
    println!("  {}     {}", color::gray("分类:"), category);
    println!("  {}     {}", color::gray("官网:"), homepage);

    // Installation detection
    let installed_db = software::read_installed_db().unwrap_or_default();
    if let Some(rec) = installed_db.get(&sd.name) {
        println!("\n  {} (版本 {})", color::green("已安装"), rec.version);
        if !rec.install_path.is_empty() {
            println!("  路径: {}", rec.install_path);
        }
    } else {
        let mut found = false;
        for reg in registry::scan_all_installed() {
            if name_matches(&reg.get("display_name").cloned().unwrap_or_default(), &sd) {
                println!("\n  {}", color::green("已安装"));
                println!("  显示名称: {}", reg.get("display_name").unwrap_or(&"".to_string()));
                println!("  版本: {}", reg.get("version").unwrap_or(&"".to_string()));
                if let Some(p) = reg.get("install_path") {
                    if !p.is_empty() {
                        println!("  路径: {}", p.trim_matches('"'));
                    }
                }
                if let Some(p) = reg.get("publisher") {
                    if !p.is_empty() {
                        println!("  发行商: {}", p);
                    }
                }
                found = true;
                break;
            }
        }
        if !found {
            println!("\n  {}", color::gray("未安装"));
        }
    }

    // Version list
    println!("\n  {}", color::gray("可用版本:"));
    let mut sorted_versions: Vec<&String> = sd.versions.keys().collect();
    sorted_versions.sort_by(|a, b| {
        let a_segs: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
        let b_segs: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
        for i in 0..a_segs.len().max(b_segs.len()) {
            let av = a_segs.get(i).copied().unwrap_or(0);
            let bv = b_segs.get(i).copied().unwrap_or(0);
            match bv.cmp(&av) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        b.cmp(a)
    });
    for vk in &sorted_versions {
        let vi = &sd.versions[*vk];
        let marker = if vk.as_str() == default_ver { " ← 默认" } else { "" };
        let urls = &vi.urls;
        let first_url = urls.first().map(|s| s.as_str()).unwrap_or("无下载地址");
        let installer_type = if vi.installer_type.is_empty() { "(auto)" } else { &vi.installer_type };
        println!("    {}", color::green(format!("{}{}", vk, marker)));
        println!("      {} {}", color::gray("类型:"), installer_type);
        println!("      {} {}", color::gray("下载:"), first_url);
        // Show additional URLs
        for url in urls.iter().skip(1) {
            println!("           {}", url);
        }
    }
    println!();

    Ok(())
}

// ── source ────────────────────────────────────────────────

fn run_source(action: SourceCmd) -> anyhow::Result<()> {
    match action {
        SourceCmd::Update => {
            software::update_sources()?;
        }
        SourceCmd::Path { open } => {
            let dir = paths::source_dir();
            if open {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                println!("已在资源管理器中打开: {}", dir.display());
            } else {
                println!("{}", dir.display());
            }
        }
        SourceCmd::Dirs { open } => {
            return run_dirs(open);
        }
        SourceCmd::Speedtest { name, software } => {
            return speedtest::speedtest(&name, software);
        }
    }
    Ok(())
}

// ── dirs ──────────────────────────────────────────────────

fn run_dirs(open_explorer: bool) -> anyhow::Result<()> {
    let root = std::env::var("LOCALAPPDATA")
        .map(|p| std::path::PathBuf::from(p).join("aminos"))
        .unwrap_or_else(|_| paths::source_dir().parent().map(|p| p.to_path_buf()).unwrap_or_default());

    if open_explorer {
        let _ = std::process::Command::new("explorer").arg(&root).spawn();
        println!("已在资源管理器中打开: {}", root.display());
        return Ok(());
    }

    let exe = std::env::current_exe().unwrap_or_default();
    println!("\n{}\n", color::bold_cyan("aminos 数据目录一览"));

    println!("  {}", color::bold_yellow("可执行文件"));
    println!("    {}", exe.display());

    println!();
    println!("  {}  (json)", color::bold_yellow("软件源定义"));
    println!("    {}", paths::source_dir().display());

    println!();
    println!("  {}  (下载的 exe/msi/zip)", color::bold_yellow("安装包缓存"));
    println!("    {}", paths::downloads_dir().display());

    println!();
    println!("  {}  (installed.json)", color::bold_yellow("安装记录"));
    println!("    {}", paths::installed_json().display());

    println!();
    println!("  {}  (as 安装的软件链接)", color::bold_yellow("快捷方式"));
    println!("    {}", paths::apps_dir().display());

    println!();
    println!("  {}", color::bold_yellow("数据根目录"));
    println!("    {}", root.display());

    Ok(())
}

// ── cache ────────────────────────────────────────────────

fn run_cache(clear: bool, open: bool) -> anyhow::Result<()> {
    let downloads = paths::downloads_dir();

    if open {
        if downloads.exists() {
            let _ = std::process::Command::new("explorer").arg(&downloads).spawn();
            println!("已在资源管理器中打开: {}", downloads.display());
        } else {
            println!("缓存目录不存在，暂无已下载的文件。");
        }
        return Ok(());
    }

    if clear {
        if downloads.is_dir() {
            let mut count = 0u32;
            let mut total_size = 0u64;
            if let Ok(entries) = std::fs::read_dir(&downloads) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        total_size += path.metadata().map(|m| m.len()).unwrap_or(0);
                        count += 1;
                    }
                }
            }
            // Remove all files
            if let Ok(entries) = std::fs::read_dir(&downloads) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
            println!("{}", color::green(format!("已清除 {} 个缓存文件 ({} 空间)", count, format_size(total_size))));
        } else {
            println!("缓存目录不存在，无需清除。");
        }
        return Ok(());
    }

    // List cached files
    if !downloads.is_dir() || downloads.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("暂无已下载的缓存文件。\n  目录: {}", downloads.display());
        return Ok(());
    }

    let source_defs = software::list_software_defs().unwrap_or_default();

    // entries: (文件名, 大小, PE版本, 一致性标记)
    let mut entries: Vec<(String, u64, String, String)> = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(&downloads) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                let pe_ver = pe_version::get_pe_version(&path).unwrap_or_else(|| "-".to_string());

                // 尝试匹配源定义，做一致性检查
                let consistency = if pe_ver == "-" || pe_ver.is_empty() {
                    String::new()
                } else {
                    // 从文件名推测软件名：依次检查每个源名是否匹配文件名前缀
                    let matched_sd = source_defs.iter().find(|sd| {
                        let prefix = format!("{}-", sd.name);
                        name.starts_with(&prefix)
                    });
                    match matched_sd {
                        Some(sd) if sd.default_version != pe_ver => {
                            color::yellow(" ⚠")
                        }
                        Some(_) => {
                            color::green(" ✓")
                        }
                        None => String::new(),
                    }
                };

                entries.push((name, size, pe_ver, consistency));
            }
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let total_size: u64 = entries.iter().map(|(_, s, _, _)| s).sum();
    let max_name = entries.iter().map(|(n, _, _, _)| n.display_width()).max().unwrap_or(4).min(50);
    let max_ver = entries.iter().map(|(_, _, v, _)| v.display_width()).max().unwrap_or(4).max(4);

    println!("\n{}  {}\n", color::bold_yellow("下载缓存"), color::gray(format!("{}", downloads.display())));
    println!("  {}{}{}",
        pad("文件", max_name + 2),
        pad("版本", max_ver + 2),
        pad("大小", 12));

    for (name, size, ver, consistency) in &entries {
        println!("  {}{}{}{}",
            pad(&truncate(name, max_name), max_name + 2),
            pad(&truncate(ver, max_ver), max_ver + 2),
            pad(&format_size(*size), 12),
            consistency,
        );
    }

    // 图例
    if entries.iter().any(|(_, _, _, c)| !c.is_empty()) {
        println!();
        println!("  {} 版本与源定义一致  {} 与源定义不一致", color::green("✓"), color::yellow("⚠"));
    }

    println!("\n{}", color::gray(format!("共 {} 个文件，{} 空间", entries.len(), format_size(total_size))));
    println!("{}", color::gray("  as cache --clear  清除缓存"));
    println!("{}", color::gray("  as cache --open   在资源管理器中打开"));
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────

fn name_matches(reg_name: &str, sd: &software::SoftwareDef) -> bool {
    let rn_lower = reg_name.to_lowercase();
    let dn = sd.display_name.to_lowercase();
    if !dn.is_empty() && dn == rn_lower {
        return true;
    }
    if !dn.is_empty() && word_match(&dn, reg_name) {
        return true;
    }
    // Aliases: exact case-insensitive match only (no substring)
    for alias in &sd.aliases {
        if alias.to_lowercase() == rn_lower {
            return true;
        }
    }
    word_match(&sd.name.to_lowercase(), reg_name)
}

fn word_match(keyword: &str, text: &str) -> bool {
    let lower_text = text.to_lowercase();
    let lower_kw = keyword.to_lowercase();
    lower_text.contains(&lower_kw)
}
