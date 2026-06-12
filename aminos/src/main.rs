mod downloader;
mod installer;
mod paths;
mod pe_version;
mod registry;
mod software;
mod speedtest;

use clap::{CommandFactory, Parser, Subcommand};

use crate::downloader::{display_width, format_size, pad, truncate_display};

const HELP_TEMPLATE: &str = "\
\x1b[1;36maminos\x1b[0m — \x1b[32m轻量级 Windows 软件包管理器\x1b[0m

\x1b[1;33m用法:\x1b[0m
  \x1b[36mas\x1b[0m \x1b[32m<命令>\x1b[0m [参数]

\x1b[1;33m命令:\x1b[0m
{subcommands}
\x1b[1;33m选项:\x1b[0m
{options}

\x1b[1;33m示例:\x1b[0m
  \x1b[36mas source update\x1b[0m        下载软件源
  \x1b[36mas list\x1b[0m                 列出所有可安装的软件
  \x1b[36mas install 7zip\x1b[0m         安装指定软件
  \x1b[36mas upgrade\x1b[0m              升级所有已安装的软件
  \x1b[36mas upgrade --check\x1b[0m      仅检查更新
  \x1b[36mas info 7zip\x1b[0m            查看软件详情
  \x1b[36mas info 7zip --urls\x1b[0m     查看所有下载地址
  \x1b[36mas source dirs\x1b[0m          查看所有数据目录

\x1b[1;33m提示:\x1b[0m
  更多帮助请运行 \x1b[36mas <命令> --help\x1b[0m
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
    disable_help_subcommand = true,
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
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
        /// 搜索软件名
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
    /// 测速所有下载源
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Speedtest {
        /// 可选：仅测速指定软件
        name: Vec<String>,
        /// 以软件为单位统计（任一源可用即为通）
        #[arg(short = 'S', long = "software")]
        software: bool,
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
}

#[derive(Subcommand)]
enum SourceCmd {
    /// 从远程仓库下载最新源定义
    Update,
    /// 显示当前源目录路径
    Path {
        /// 在资源管理器中打开
        #[arg(short, long)]
        open: bool,
    },
    /// 显示所有数据目录位置
    Dirs {
        /// 在资源管理器中打开数据目录
        #[arg(short, long)]
        open: bool,
    },
}

fn enable_ansi() {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> isize;
            fn GetConsoleMode(hConsoleHandle: isize, lpMode: *mut u32) -> i32;
            fn SetConsoleMode(hConsoleHandle: isize, dwMode: u32) -> i32;
        }
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

        unsafe {
            for &handle_id in &[0xFFFFFFF5u32, 0xFFFFFFF4u32] {
                let h = GetStdHandle(handle_id);
                if h <= 0 {
                    continue;
                }
                let mut mode: u32 = 0;
                if GetConsoleMode(h, &mut mode) != 0 {
                    let _ = SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
        }
    }
}

fn main() {
    enable_ansi();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            print_clap_error(e);
            return;
        }
    };
    match cli.command {
        None => {
            let _ = Cli::command().print_help();
        }
        Some(Command::Install { names, version, gui, renew, download_only }) => {
            let _ = run(|| run_install(names, version.unwrap_or_default(), gui, renew, download_only));
        }
        Some(Command::Uninstall { names, gui, force }) => {
            let _ = run(|| run_uninstall(names, gui, force));
        }
        Some(Command::List { filter, install_only, missing, search, downloaded, downloading, no_download }) => {
            let _ = run(|| run_list(filter, install_only, missing, search, downloaded, downloading, no_download));
        }
        Some(Command::Info { name, urls }) => {
            let _ = run(|| run_info(&name, urls));
        }
        Some(Command::Speedtest { name, software }) => {
            let _ = run(|| speedtest::speedtest(&name, software));
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
    }
}

fn print_clap_error(e: clap::Error) {
    use clap::error::ErrorKind;
    let info = e.to_string();

    // Extract error detail from clap's raw output (the line after "error: ")
    let detail = info
        .lines()
        .find(|l| !l.starts_with("Usage:") && !l.starts_with("For more"))
        .and_then(|l| l.strip_prefix("error: "))
        .unwrap_or("")
        .to_string();

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
            // clap: "the following required arguments were not provided: <NAMES>..."
            let missing = detail
                .strip_prefix("the following required arguments were not provided:")
                .unwrap_or("")
                .trim();
            if !missing.is_empty() {
                format!("缺少必需参数: {}", missing)
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

    let usage_line = info
        .lines()
        .find(|l| l.starts_with("Usage:"))
        .and_then(|u| u.strip_prefix("Usage:"))
        .unwrap_or("");

    eprintln!("\x1b[31m错误:\x1b[0m {}", msg);
    if !usage_line.is_empty() {
        eprintln!("\x1b[1;33m用法:\x1b[0m{}", usage_line);
    }
    eprintln!("\x1b[90m更多帮助请运行 --help\x1b[0m");
}

fn run<F: FnOnce() -> anyhow::Result<()>>(f: F) -> anyhow::Result<()> {
    if let Err(e) = f() {
        eprintln!("\x1b[31m错误:\x1b[0m {}", e);
    }
    Ok(())
}

// ── install ───────────────────────────────────────────────

fn run_install(names: Vec<String>, version: String, gui: bool, renew: bool, download_only: bool) -> anyhow::Result<()> {
    for name in &names {
        let n = name.to_lowercase();
        if let Err(e) = installer::install_software(&n, &version, gui, renew, download_only) {
            eprintln!("  \x1b[33m跳过 {}\x1b[0m: {}", name, e);
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
                eprintln!("  \x1b[33m跳过 {}\x1b[0m: {}", name, e);
                failed += 1;
                continue;
            }
        };

        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let source_ver = &sd.default_version;

        // 检查已安装的版本
        let current_ver = installed_db.get(name)
            .map(|rec| rec.version.as_str())
            .or_else(|| {
                // 回退到注册表检测
                sd.versions.get(source_ver)
                    .and_then(|vi| vi.detection.as_ref())
                    .and_then(|d| registry::detect_installed(d))
                    .and_then(|r| r.get("DisplayVersion").cloned())
                    .map(|s| {
                        // 仅在 check_only 模式下，注册表版本不做记录
                        let leaked: &'static str = Box::leak(s.into_boxed_str());
                        leaked
                    })
            })
            .unwrap_or("");

        if current_ver == source_ver && !renew {
            println!("  \x1b[90m{} {} 已是最新\x1b[0m", display, current_ver);
            up_to_date += 1;
            continue;
        }

        if check_only {
            println!("  \x1b[33m{} {}\x1b[0m → \x1b[32m{}\x1b[0m 可更新",
                display, current_ver, source_ver);
            updated += 1;
            continue;
        }

        println!("  ▶ {} {} → {} ...", display, current_ver, source_ver);
        match installer::install_software(name, "", false, renew, false) {
            Ok(()) => {
                updated += 1;
            }
            Err(e) => {
                eprintln!("  \x1b[33m升级 {} 失败\x1b[0m: {}", display, e);
                failed += 1;
            }
        }
    }

    println!();
    if check_only {
        println!("\x1b[90m共检查 {} 个，{} 个可更新，{} 个最新，{} 个失败\x1b[0m",
            targets.len(), updated, up_to_date, failed);
    } else {
        println!("\x1b[90m共 {} 个，{} 个已升级，{} 个已最新，{} 个失败\x1b[0m",
            targets.len(), updated, up_to_date, failed);
    }
    Ok(())
}

fn run_uninstall(names: Vec<String>, gui: bool, force: bool) -> anyhow::Result<()> {
    for name in &names {
        let n = name.to_lowercase();
        if let Err(e) = installer::uninstall_software(&n, gui, force) {
            eprintln!("  \x1b[33m跳过 {}\x1b[0m: {}", name, e);
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

            let (status, color) = if is_downloading {
                ("下载中", "\x1b[33m") // 黄色
            } else {
                ("已下载", "\x1b[36m") // 青色
            };

            // 优先保留"已下载"状态（覆盖"下载中"）
            let entry = result.entry(name_part).or_insert((status, color));
            if !is_downloading {
                *entry = (status, color);
            }
        }
    }

    result
}

fn run_list(filter: Option<String>, install_only: bool, missing: bool, search: Option<String>, downloaded: bool, downloading: bool, no_download: bool) -> anyhow::Result<()> {
    // Auto-init: if source dir is empty, suggest `as source update`
    let source = paths::source_dir();
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("\x1b[33m  未找到源定义。首次使用请运行:\x1b[0m");
        println!("  as source update\n");
        return Ok(());
    }

    let reg_installed = registry::scan_all_installed();
    let installed_db = software::read_installed_db().unwrap_or_default();
    let defs = software::list_software_defs()?;
    let dl_cache = scan_download_cache();

    // Rows: (名称, 版本, 安装状态, 安装颜色, 下载状态, 下载颜色, 源标签, 源颜色)
    let mut rows: Vec<(String, String, &str, &str, &str, &str, &str, &str)> = Vec::new();
    let mut seen_registry: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Registry entries — 版本：installed_db (PE) > Registry
    for reg in &reg_installed {
        let rn = reg.get("display_name").cloned().unwrap_or_default();
        if rn.is_empty() || !seen_registry.insert(rn.clone()) {
            continue;
        }
        let has_source = defs.iter().any(|sd| name_matches(&rn, sd));
        let src_label = if has_source { "有" } else { "无" };
        let src_color = if has_source { "\x1b[32m" } else { "\x1b[90m" };
        // 通过软件名查找下载状态
        let (dl_status, dl_color) = if has_source {
            if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
                dl_cache.get(&sd.name).copied().unwrap_or(("未下载", "\x1b[90m"))
            } else {
                ("未下载", "\x1b[90m")
            }
        } else {
            ("未下载", "\x1b[90m")
        };
        // 版本 reconciliation：installed_db (PE) > Registry DisplayVersion
        let ver = if has_source {
            if let Some(sd) = defs.iter().find(|sd| name_matches(&rn, sd)) {
                installed_db.get(&sd.name)
                    .map(|rec| rec.version.clone())
                    .unwrap_or_else(|| reg.get("version").cloned().unwrap_or_default())
            } else {
                reg.get("version").cloned().unwrap_or_default()
            }
        } else {
            reg.get("version").cloned().unwrap_or_default()
        };
        rows.push((rn, ver,
            "已安装", "\x1b[32m", dl_status, dl_color, src_label, src_color));
    }

    // 2. Source definitions not in registry
    for sd in &defs {
        let name = &sd.name;
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        let already = reg_installed.iter().any(|r| {
            name_matches(&r.get("display_name").cloned().unwrap_or_default(), sd)
        });
        if already {
            continue;
        }
        let (dl_status, dl_color) = dl_cache.get(name)
            .copied()
            .unwrap_or(("未下载", "\x1b[90m"));
        if let Some(rec) = installed_db.get(name) {
            rows.push((display.to_string(), rec.version.clone(),
                "已安装", "\x1b[32m", dl_status, dl_color, "有", "\x1b[32m"));
            continue;
        }
        rows.push((display.to_string(), sd.default_version.clone(),
            "未安装", "\x1b[90m", dl_status, dl_color, "有", "\x1b[32m"));
    }

    // 3. Filter by install/download/search
    if install_only {
        rows.retain(|r| r.2 == "已安装");
    }
    if missing {
        rows.retain(|r| r.2 == "未安装");
    }
    if downloaded {
        rows.retain(|r| r.4 == "已下载");
    }
    if downloading {
        rows.retain(|r| r.4 == "下载中");
    }
    if no_download {
        rows.retain(|r| r.4 == "未下载");
    }
    if let Some(ref kw) = search {
        let kw_lower = kw.to_lowercase();
        rows.retain(|r| r.0.to_lowercase().contains(&kw_lower));
    }
    if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        rows.retain(|r| r.0.to_lowercase().contains(&f_lower));
    }

    // Sort by name case-insensitive
    rows.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    if rows.is_empty() {
        println!("没有匹配的软件。");
        return Ok(());
    }

    // Calculate column widths
    let max_name = rows.iter().map(|r| display_width(&r.0)).max().unwrap_or(4).max(4).min(40);
    let max_ver = rows.iter().map(|r| display_width(&r.1)).max().unwrap_or(4).max(4);

    println!();
    let header = format!("{}{}{}{}{}",
        pad("名称", max_name + 2),
        pad("版本", max_ver + 2),
        pad("下载", 8 + 1), // 空格
        pad("状态", 8 + 1), // 空格
        pad("源", 4));
    println!("{}", header);
    println!("{}", "-".repeat(display_width(&header)));

    for (name, ver, _status, status_color, dl_status, dl_color, src_label, src_color) in &rows {
        let name_d = truncate_display(name, max_name);
        let ver_d = truncate_display(ver, max_ver + 1);
        println!(
            "{}{}{}{}{}{}{}{}{}{}\x1b[0m",
            pad(&name_d, max_name + 2),
            pad(&ver_d, max_ver + 2),
            dl_color,
            pad(dl_status, 8),
            "\x1b[0m ",
            status_color,
            pad(_status, 8),
            "\x1b[0m ",
            src_color,
            pad(src_label, 4),
        );
    }

    println!("\n\x1b[90m共 {} 项\x1b[0m", rows.len());
    Ok(())
}

// ── info (matches Python InfoCommand) ─────────────────────

fn run_info(name: &str, show_urls: bool) -> anyhow::Result<()> {
    let name_lower = name.to_lowercase();
    let sd = software::read_software_def(&name_lower)?;
    let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };

    // --urls 模式：仅列出所有下载地址
    if show_urls {
        println!("\x1b[32m{}\x1b[0m", display);
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
            let expanded = crate::downloader::expand_github_urls(&vi.urls);
            println!("  \x1b[36m{}\x1b[0m", vk);
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

    println!("\x1b[32m{}\x1b[0m", display);
    if !desc.is_empty() {
        println!("  {}", desc);
    }
    println!();

    // Identifier & aliases
    let primary_id = &sd.name;
    let aliases = &sd.aliases;
    let mut id_line = format!("  \x1b[90m标识符:\x1b[0m   {}", primary_id);
    if !aliases.is_empty() {
        id_line.push_str(&format!("  ({})", aliases.join(", ")));
    }
    println!("{}", id_line);
    println!("  \x1b[90m分类:\x1b[0m     {}", category);
    println!("  \x1b[90m官网:\x1b[0m     {}", homepage);

    // Installation detection
    let installed_db = software::read_installed_db().unwrap_or_default();
    if let Some(rec) = installed_db.get(&sd.name) {
        println!("\n  \x1b[32m已安装\x1b[0m (版本 {})", rec.version);
        if !rec.install_path.is_empty() {
            println!("  路径: {}", rec.install_path);
        }
    } else {
        let mut found = false;
        for reg in registry::scan_all_installed() {
            if name_matches(&reg.get("display_name").cloned().unwrap_or_default(), &sd) {
                println!("\n  \x1b[32m已安装\x1b[0m");
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
            println!("\n  \x1b[90m未安装\x1b[0m");
        }
    }

    // Version list
    println!("\n  \x1b[90m可用版本:\x1b[0m");
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
        println!("    \x1b[32m{}{}\x1b[0m", vk, marker);
        println!("      \x1b[90m类型:\x1b[0m {}", installer_type);
        println!("      \x1b[90m下载:\x1b[0m {}", first_url);
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
    println!("\x1b[1;36maminos 数据目录一览\x1b[0m\n");

    println!("  \x1b[1;33m可执行文件\x1b[0m");
    println!("    {}", exe.display());

    println!();
    println!("  \x1b[1;33m软件源定义\x1b[0m  (json)");
    println!("    {}", paths::source_dir().display());

    println!();
    println!("  \x1b[1;33m安装包缓存\x1b[0m  (下载的 exe/msi/zip)");
    println!("    {}", paths::downloads_dir().display());

    println!();
    println!("  \x1b[1;33m安装记录\x1b[0m  (installed.json)");
    println!("    {}", paths::installed_json().display());

    println!();
    println!("  \x1b[1;33m快捷方式\x1b[0m  (as 安装的软件链接)");
    println!("    {}", paths::apps_dir().display());

    println!();
    println!("  \x1b[1;33m数据根目录\x1b[0m");
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
            println!("\x1b[32m已清除 {} 个缓存文件 ({} 空间)\x1b[0m",
                count, format_size(total_size as f64));
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
                            format!("\x1b[33m ⚠\x1b[0m")
                        }
                        Some(_) => {
                            format!("\x1b[32m ✓\x1b[0m")
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
    let max_name = entries.iter().map(|(n, _, _, _)| display_width(n)).max().unwrap_or(4).min(50);
    let max_ver = entries.iter().map(|(_, _, v, _)| display_width(v)).max().unwrap_or(4).max(4);

    println!("\n\x1b[1;33m下载缓存\x1b[0m  \x1b[90m{}\x1b[0m\n", downloads.display());
    println!("  {}{}{}",
        pad("文件", max_name + 2),
        pad("版本", max_ver + 2),
        pad("大小", 12));

    for (name, size, ver, consistency) in &entries {
        println!("  {}{}{}{}",
            pad(&truncate_display(name, max_name), max_name + 2),
            pad(&truncate_display(ver, max_ver), max_ver + 2),
            pad(&format_size(*size as f64), 12),
            consistency,
        );
    }

    // 图例
    if entries.iter().any(|(_, _, _, c)| !c.is_empty()) {
        println!();
        println!("  \x1b[32m✓\x1b[0m 版本与源定义一致  \x1b[33m⚠\x1b[0m 与源定义不一致");
    }

    println!("\n\x1b[90m共 {} 个文件，{} 空间\x1b[0m", entries.len(), format_size(total_size as f64));
    println!("\x1b[90m  as cache --clear  清除缓存\x1b[0m");
    println!("\x1b[90m  as cache --open   在资源管理器中打开\x1b[0m");
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
