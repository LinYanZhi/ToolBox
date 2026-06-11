mod downloader;
mod installer;
mod paths;
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
  \x1b[36mas source update\x1b[0m  首次使用，下载软件源
  \x1b[36mas list\x1b[0m           列出所有可安装的软件
  \x1b[36mas install 7zip \x1b[0m  安装指定软件
  \x1b[36mas speedtest\x1b[0m      测速所有下载源
  \x1b[36mas dirs\x1b[0m           查看数据目录位置
  \x1b[36mas info 7zip\x1b[0m      查看软件详情

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
        #[arg(long = "download-only", short = 'D')]
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
    },
    /// 查看软件详细信息
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Info {
        /// 软件名称
        #[arg(required = true)]
        name: String,
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
    /// 列出软件下载链接
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Urls {
        /// 软件名称（不指定则列出全部）
        name: Vec<String>,
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
    /// 管理软件源定义
    #[command(help_template = HELP_TEMPLATE_SUBCMDS)]
    Source {
        #[command(subcommand)]
        action: SourceCmd,
    },
    /// 显示所有数据目录位置
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Dirs {
        /// 在资源管理器中打开数据目录
        #[arg(short, long)]
        open: bool,
    },
}

#[derive(Subcommand)]
enum SourceCmd {
    /// 从远程仓库下载最新源定义
    Update,
    /// 显示当前源目录路径
    Path,
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
        Some(Command::List { filter, install_only, missing, search }) => {
            let _ = run(|| run_list(filter, install_only, missing, search));
        }
        Some(Command::Info { name }) => {
            let _ = run(|| run_info(name));
        }
        Some(Command::Speedtest { name, software }) => {
            let _ = run(|| speedtest::speedtest(&name, software));
        }
        Some(Command::Source { action }) => {
            let _ = run(|| run_source(action));
        }
        Some(Command::Dirs { open }) => {
            let _ = run(|| run_dirs(open));
        }
        Some(Command::Urls { name }) => {
            let _ = run(|| run_urls(&name));
        }
        Some(Command::Cache { clear, open }) => {
            let _ = run(|| run_cache(clear, open));
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

fn run_list(filter: Option<String>, install_only: bool, missing: bool, search: Option<String>) -> anyhow::Result<()> {
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

    // Rows: (name, version, status, status_color, has_source)
    let mut rows: Vec<(String, String, &str, &str, &str, &str)> = Vec::new();
    let mut seen_registry: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Registry entries
    for reg in &reg_installed {
        let rn = reg.get("display_name").cloned().unwrap_or_default();
        if rn.is_empty() || !seen_registry.insert(rn.clone()) {
            continue;
        }
        let has_source = defs.iter().any(|sd| name_matches(&rn, sd));
        let src_label = if has_source { "有" } else { "无" };
        let src_color = if has_source { "\x1b[32m" } else { "\x1b[90m" };
        rows.push((rn, reg.get("version").cloned().unwrap_or_default(),
            "已安装", "\x1b[32m", src_label, src_color));
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
        if let Some(rec) = installed_db.get(name) {
            rows.push((display.to_string(), rec.version.clone(),
                "已安装", "\x1b[32m", "有", "\x1b[32m"));
            continue;
        }
        rows.push((display.to_string(), sd.default_version.clone(),
            "未安装", "\x1b[90m", "有", "\x1b[32m"));
    }

    // 3. Filter
    if install_only {
        rows.retain(|r| r.2 == "已安装");
    }
    if missing {
        rows.retain(|r| r.2 == "未安装");
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
    let header = format!("{}{}{}{}",
        pad("名称", max_name + 2),
        pad("版本", max_ver + 2),
        pad("状态", 10),
        pad("源", 4));
    println!("{}", header);
    println!("{}", "-".repeat(display_width(&header)));

    for (name, ver, status, status_color, src_label, src_color) in &rows {
        let name_d = truncate_display(name, max_name);
        let ver_d = truncate_display(ver, max_ver + 1);
        println!(
            "{}{}{}{}{}{}{}\x1b[0m",
            pad(&name_d, max_name + 2),
            pad(&ver_d, max_ver + 2),
            status_color,
            pad(status, 9),
            "\x1b[0m ",
            src_color,
            src_label,
        );
    }

    println!("\n\x1b[90m共 {} 项\x1b[0m", rows.len());
    Ok(())
}

// ── info (matches Python InfoCommand) ─────────────────────

fn run_info(name: String) -> anyhow::Result<()> {
    let name_lower = name.to_lowercase();
    let sd = software::read_software_def(&name_lower)?;
    let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
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
        SourceCmd::Path => {
            println!("{}", paths::source_dir().display());
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

// ── urls ──────────────────────────────────────────────────

fn run_urls(names: &[String]) -> anyhow::Result<()> {
    // Auto-init: if source dir is empty, suggest `as source update`
    let source = paths::source_dir();
    if !source.is_dir() || source.read_dir().map(|mut d| d.next().is_none()).unwrap_or(true) {
        println!("\x1b[33m  未找到源定义。首次使用请运行:\x1b[0m");
        println!("  as source update\n");
        return Ok(());
    }

    let defs: Vec<software::SoftwareDef> = if names.is_empty() {
        software::list_software_defs()?
    } else {
        let mut v = Vec::new();
        for n in names {
            match software::read_software_def(n) {
                Ok(sd) => v.push(sd),
                Err(_e) => eprintln!("\x1b[31m错误:\x1b[0m 未找到软件 '{}' 的定义", n),
            }
        }
        v
    };

    if defs.is_empty() {
        println!("没有匹配的软件。");
        return Ok(());
    }

    let mut total_urls = 0u32;

    for sd in &defs {
        let display = if sd.display_name.is_empty() { &sd.name } else { &sd.display_name };
        println!("\n\x1b[1;32m{}\x1b[0m", display);

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
            let expanded = downloader::expand_github_urls(&vi.urls);
            let count = expanded.len() as u32;
            total_urls += count;

            let inst_type = if vi.installer_type.is_empty() { "auto" } else { &vi.installer_type };
            if vi.arch.is_empty() {
                println!("  \x1b[36m{}\x1b[0m  \x1b[90m({}, {} 个源)\x1b[0m", vk, inst_type, count);
            } else {
                println!("  \x1b[36m{} {}\x1b[0m  \x1b[90m({}, {} 个源)\x1b[0m", vk, vi.arch, inst_type, count);
            }

            for url in &expanded {
                println!("    {}", url);
            }
        }
    }

    println!("\n\x1b[90m共 {} 个软件，{} 个下载链接\x1b[0m", defs.len(), total_urls);
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

    let mut entries: Vec<(String, u64)> = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(&downloads) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                entries.push((name, size));
            }
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let total_size: u64 = entries.iter().map(|(_, s)| s).sum();
    let max_name = entries.iter().map(|(n, _)| display_width(n)).max().unwrap_or(4).min(50);

    println!("\n\x1b[1;33m下载缓存\x1b[0m  \x1b[90m{}\x1b[0m\n", downloads.display());
    println!("  {}{}",
        pad("文件", max_name + 2),
        pad("大小", 12));

    for (name, size) in &entries {
        println!("  {}{}",
            pad(&truncate_display(name, max_name), max_name + 2),
            pad(&format_size(*size as f64), 12));
    }

    println!("\n\x1b[90m共 {} 个文件，{} 空间\x1b[0m", entries.len(), format_size(total_size as f64));
    println!("\x1b[90m  as cache --clear  清除缓存\x1b[0m");
    println!("\x1b[90m  as cache --open   在浏览器中打开\x1b[0m");
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
