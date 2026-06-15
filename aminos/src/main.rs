mod cmd_cache;
mod cmd_config;
mod cmd_downloader;
mod cmd_info;
mod cmd_init;
mod cmd_install;
mod cmd_list;
mod cmd_self;
mod cmd_self_update;
mod cmd_source;
mod cmd_tool;
mod cmd_uninstall;
mod cmd_upgrade;
mod downloader;
mod helpers;
mod help;
mod installer;
mod opts;
mod paths;
mod pe_version;
mod registry;
mod software;
mod speedtest;

use clap::{Parser, Subcommand};

use help::{HELP_STYLES, HELP_TEMPLATE_OPTIONS, HELP_TEMPLATE_SUBCMDS, print_clap_error, print_root_help, run_example, run};
use opts::{InstallOpts, ListOpts, ToolInstallOpts, ToolUpgradeOpts, UninstallOpts, UpgradeOpts};

/// AminOS - lightweight software package manager
#[derive(Parser)]
#[command(
    name = "as",
    version,
    about,
    color = clap::ColorChoice::Always,
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
        #[arg(short = 'g', long)]
        gui: bool,
        /// 强制重新下载
        #[arg(short = 'r', long)]
        renew: bool,
        /// 仅下载，不安装
        #[arg(short = 'd', long = "download-only")]
        download_only: bool,
        /// 安装类型：portable（便携版）或 installer（安装版）
        #[arg(long = "type", value_parser = clap::builder::PossibleValuesParser::new(["portable", "installer"]))]
        inst_type: Option<String>,
    },
    /// 列出可用软件及安装状态
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    List {
        /// 按分类过滤（as list --categories 查看可用分类）
        #[arg(short, long)]
        filter: Option<String>,
        /// 仅显示已安装
        #[arg(long = "installed", short = 'i', conflicts_with = "missing")]
        install_only: bool,
        /// 仅显示未安装
        #[arg(long = "not-installed", short = 'm', conflicts_with = "install_only")]
        missing: bool,
        /// 搜索软件名、别名或描述
        #[arg(short = 's', long = "search")]
        search: Option<String>,
        /// 仅显示已下载
        #[arg(short = 'd', long, conflicts_with_all = &["downloading", "no_download"])]
        downloaded: bool,
        /// 仅显示下载中
        #[arg(long, conflicts_with_all = &["downloaded", "no_download"])]
        downloading: bool,
        /// 仅显示未下载
        #[arg(long = "no-download", conflicts_with_all = &["downloaded", "downloading"])]
        no_download: bool,
        /// 按分类分组显示
        #[arg(short = 'g', long)]
        group: bool,
        /// 显示所有分类概览
        #[arg(long)]
        categories: bool,
        /// 查看软件详细信息（as info 的替代）
        #[arg(long)]
        info: Option<String>,
    },
    /// 卸载指定软件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Uninstall {
        #[arg(required = true)]
        names: Vec<String>,
        /// 使用图形界面卸载
        #[arg(short = 'g', long)]
        gui: bool,
        /// 强制删除
        #[arg(short, long)]
        force: bool,
    },
    /// 升级所有已安装的软件
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Upgrade {
        /// 可选：仅升级指定软件（不指定则全部升级）
        names: Vec<String>,
        /// 仅检查更新，不下也不装
        #[arg(short, long, conflicts_with = "renew")]
        check: bool,
        /// 强制重新下载（即使版本相同）
        #[arg(long, conflicts_with = "check")]
        renew: bool,
    },
    /// 管理 as 环境（源、缓存、下载引擎）
    #[command(name = "config", help_template = HELP_TEMPLATE_SUBCMDS)]
    Config {
        #[command(subcommand)]
        action: Option<ConfigCmd>,
    },
    /// 管理 as 自身（初始化、更新）
    #[command(name = "self", help_template = HELP_TEMPLATE_SUBCMDS)]
    SelfMgmt {
        #[command(subcommand)]
        action: Option<SelfCmd>,
    },
    /// 管理自研工具（安装、升级、列出、移除）
#[command(help_template = HELP_TEMPLATE_SUBCMDS)]
Tool {
    #[command(subcommand)]
    action: Option<ToolCmd>,
},
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// 显示/打开配置目录（数据根目录一览）
    #[command(name = "path", help_template = HELP_TEMPLATE_OPTIONS)]
    Path {
        /// 在资源管理器中打开
        #[arg(short, long)]
        open: bool,
    },
    /// 管理下载缓存
    #[command(name = "cache", help_template = HELP_TEMPLATE_OPTIONS)]
    Cache {
        /// 清除所有缓存文件
        #[arg(short, long, conflicts_with = "open")]
        clear: bool,
        /// 在资源管理器中打开缓存目录
        #[arg(short, long, conflicts_with = "clear")]
        open: bool,
    },
    /// 管理软件源定义
    #[command(name = "source", help_template = HELP_TEMPLATE_SUBCMDS)]
    Source {
        #[command(subcommand)]
        action: Option<SourceCmd>,
    },
    /// 测速所有下载源
    #[command(name = "speedtest", help_template = HELP_TEMPLATE_OPTIONS)]
    Speedtest {
        /// 可选：仅测速指定软件
        name: Vec<String>,
        /// 以软件为单位统计（任一源可用即为通）
        #[arg(short = 'S', long = "software")]
        software: bool,
    },
    /// 管理下载引擎后端
    #[command(name = "downloader", help_template = HELP_TEMPLATE_SUBCMDS)]
    Downloader {
        #[command(subcommand)]
        action: Option<DownloaderCmd>,
    },
}

#[derive(Subcommand)]
pub enum SourceCmd {
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
}

#[derive(Subcommand)]
pub enum SelfCmd {
    /// 初始化 as 环境（创建 tools/bin 并注册到 PATH）
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Init,
    /// 更新 as 自身到最新版本
    #[command(name = "update", help_template = HELP_TEMPLATE_OPTIONS)]
    Update,
}

#[derive(Subcommand)]
pub enum ToolCmd {
    /// 安装自研工具
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Install {
        /// 工具名称（可同时指定多个）
        #[arg(required = true)]
        names: Vec<String>,
        /// 指定版本号
        #[arg(short, long)]
        version: Option<String>,
        /// 强制重新下载
        #[arg(short = 'r', long)]
        renew: bool,
        /// 仅下载，不安装
        #[arg(short = 'd', long = "download-only")]
        download_only: bool,
    },
    /// 升级自研工具
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Upgrade {
        /// 可选：仅升级指定工具（不指定则全部升级）
        names: Vec<String>,
        /// 仅检查更新，不下也不装
        #[arg(short, long, conflicts_with = "renew")]
        check: bool,
        /// 强制重新下载（即使版本相同）
        #[arg(long, conflicts_with = "check")]
        renew: bool,
    },
    /// 列出已安装的自研工具
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    List,
    /// 移除一个自研工具
    #[command(help_template = HELP_TEMPLATE_OPTIONS)]
    Remove {
        /// 工具名称
        #[arg(required = true)]
        name: String,
    },
}

#[derive(Subcommand)]
pub enum DownloaderCmd {
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

    // 拦截根级 --help，走自定义渲染（不经过 clap，避免干扰子命令的 -h）
    let args: Vec<String> = std::env::args().collect();
    let first_arg = args.get(1).map(|s| s.as_str());
    let is_root_help = first_arg.map_or(false, |a| a == "-h" || a == "--help");
    if is_root_help {
        print_root_help();
        return;
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            print_clap_error(e);
            return;
        }
    };
    // 设置工具二进制目录（供下载后端检测 aria2c 等工具）
    net::backend::set_tools_bin_dir(paths::tools_bin_dir());
    match cli.command {
        None => {
            if cli.example {
                run_example();
                return;
            }
            print_root_help();
        }
        Some(Command::Install { names, version, gui, renew, download_only, inst_type }) => {
            let opts = InstallOpts::new(names, version, gui, renew, download_only, inst_type);
            let _ = run(|| cmd_install::run_install(opts));
        }
        Some(Command::List { filter, install_only, missing, search, downloaded, downloading, no_download, group, categories, info }) => {
            // --info 优先级最高：查看软件详情
            if let Some(name) = info {
                let _ = run(|| cmd_info::run_info(&name, false));
                return;
            }
            let opts = ListOpts::new(filter, install_only, missing, search, downloaded, downloading, no_download, group, categories, info);
            let _ = run(|| cmd_list::run_list(opts));
        }
        Some(Command::Uninstall { names, gui, force }) => {
            let opts = UninstallOpts::new(names, gui, force);
            let _ = run(|| cmd_uninstall::run_uninstall(opts));
        }
        Some(Command::Upgrade { names, check, renew }) => {
            let opts = UpgradeOpts::new(names, check, renew);
            let _ = run(|| cmd_upgrade::run_upgrade(opts));
        }
        Some(Command::Config { action }) => {
            match action {
                Some(cmd) => { let _ = run(|| cmd_config::run_config(cmd)); }
                None => help::print_config_help(),
            }
        }
        Some(Command::SelfMgmt { action }) => {
            match action {
                Some(cmd) => { let _ = run(|| cmd_self::run_self(cmd)); }
                None => help::print_self_help(),
            }
        }
        Some(Command::Tool { action }) => {
            match action {
                Some(ToolCmd::Install { names, version, renew, download_only }) => {
                    let opts = ToolInstallOpts::new(names, version, renew, download_only);
                    let _ = run(|| cmd_tool::run_install(opts));
                }
                Some(ToolCmd::Upgrade { names, check, renew }) => {
                    let opts = ToolUpgradeOpts::new(names, check, renew);
                    let _ = run(|| cmd_tool::run_upgrade(opts));
                }
                Some(ToolCmd::List) => {
                    let _ = run(|| cmd_tool::run_list());
                }
                Some(ToolCmd::Remove { name }) => {
                    let _ = run(|| cmd_tool::run_remove(&name));
                }
                None => help::print_tool_help(),
            }
        }
    }
}
