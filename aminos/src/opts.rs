use clap::{Parser, Subcommand, Args, builder::styling};

fn styles() -> styling::Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default().bold())
        .usage(styling::AnsiColor::Green.on_default().bold())
        .literal(styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(styling::AnsiColor::Yellow.on_default().italic())
        .error(styling::AnsiColor::Red.on_default().bold())
        .valid(styling::AnsiColor::Cyan.on_default().bold())
        .invalid(styling::AnsiColor::Yellow.on_default())
}

#[derive(Parser)]
#[command(
    name = "as",
    version,
    about = "轻量级 Windows 软件包管理器",
    styles = styles(),
    color = clap::ColorChoice::Always,
    disable_help_subcommand = true,
    next_help_heading = "选项",
)]
pub struct Cli {
    /// 显示使用示例
    #[arg(short = 'e', long = "example")]
    pub example: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 安装指定软件
    #[command(arg_required_else_help = true)]
    Install(InstallOpts),
    /// 列出已安装的软件
    List(ListOpts),
    /// 查看软件详细信息
    #[command(arg_required_else_help = true)]
    Info(InfoOpts),
    /// 下载软件或文件
    Download(DownloadOpts),
    /// 卸载指定软件
    #[command(arg_required_else_help = true)]
    Uninstall(UninstallOpts),
    /// 管理下载缓存
    Cache(CacheOpts),
    /// 管理软件源
    #[command(subcommand)]
    Source(SourceCommand),
    /// 管理下载引擎后端
    #[command(subcommand)]
    Downloader(DownloaderCommand),
    /// 管理自研工具
    #[command(subcommand)]
    Tool(ToolCli),
}

#[derive(Args)]
pub struct InstallOpts {
    /// 软件名称（可同时指定多个）
    pub names: Vec<String>,

    /// 指定版本号
    #[arg(short = 'v', long = "ver")]
    pub version: Option<String>,

    /// 使用图形界面安装（不静默）
    #[arg(short = 'g', long = "gui")]
    pub gui: bool,

    /// 强制重新下载
    #[arg(short = 'r', long = "renew")]
    pub renew: bool,

    /// 仅下载，不安装
    #[arg(short = 'd', long = "download-only")]
    pub download_only: bool,

    /// 安装类型：portable 或 installer
    #[arg(long = "type", value_name = "TYPE")]
    pub inst_type: Option<String>,

    /// 检测更新，卸载旧版后安装新版
    #[arg(short = 'u', long = "upgrade")]
    pub upgrade: bool,
}

#[derive(Args)]
pub struct ListOpts {
    /// 显示全部（已安装 + 源中可用）
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// 按分类过滤
    #[arg(short = 'f', long = "filter", value_name = "CATEGORY")]
    pub filter: Option<String>,

    /// 搜索软件名、别名或描述
    #[arg(short = 's', long = "search", value_name = "KEYWORD")]
    pub search: Option<String>,

    /// 仅显示已下载
    #[arg(short = 'd', long = "downloaded")]
    pub downloaded: bool,

    /// 仅显示下载中
    #[arg(long = "downloading")]
    pub downloading: bool,

    /// 仅显示未下载
    #[arg(long = "no-download")]
    pub no_download: bool,

    /// 按分类分组显示
    #[arg(short = 'g', long = "group")]
    pub group: bool,

    /// 显示所有分类概览
    #[arg(long = "categories")]
    pub categories: bool,
}

impl Default for ListOpts {
    fn default() -> Self {
        Self {
            all: false,
            filter: None,
            search: None,
            downloaded: false,
            downloading: false,
            no_download: false,
            group: false,
            categories: false,
        }
    }
}

#[derive(Args)]
pub struct InfoOpts {
    /// 软件名称
    pub name: String,

    /// 显示所有下载地址
    #[arg(short = 'u', long = "urls")]
    pub urls: bool,
}

#[derive(Args)]
pub struct DownloadOpts {
    /// 软件名称或下载链接
    pub targets: Vec<String>,

    /// 打开下载目录
    #[arg(short = 'o', long = "open")]
    pub open: bool,

    /// 下载到指定目录
    #[arg(long = "target", value_name = "DIR")]
    pub target_dir: Option<String>,
}

#[derive(Args)]
pub struct UninstallOpts {
    /// 软件名称（可同时指定多个）
    pub names: Vec<String>,

    /// 强制删除（跳过卸载器）
    #[arg(short = 'f', long = "force")]
    pub force: bool,
}

#[derive(Args)]
pub struct CacheOpts {
    /// 列出缓存文件（默认行为）
    #[arg(long = "list")]
    pub list: bool,

    /// 清除所有缓存
    #[arg(short = 'c', long = "clear")]
    pub clear: bool,

    /// 在资源管理器中打开缓存目录
    #[arg(short = 'o', long = "open")]
    pub open: bool,
}

#[derive(Subcommand)]
pub enum SourceCommand {
    /// 更新所有源（内置 + 第三方社区源）
    Update,
    /// 清空所有源缓存
    Clear,
    /// 在资源管理器中打开源目录
    Open,
    /// 对源进行测速
    Speedtest {
        /// 测速时指定软件（可选）
        #[arg(long = "name", value_name = "SOFTWARE")]
        name: Option<String>,
        /// 测速时以软件为单位统计
        #[arg(short = 'S', long = "software")]
        software: bool,
    },
    /// 添加第三方社区源
    Add {
        name: String,
        url: String,
    },
    /// 移除第三方社区源
    Remove {
        name: String,
    },
    /// 列出所有已配置的源
    List,
    /// 启用一个第三方源
    Enable {
        name: String,
    },
    /// 禁用一个第三方源
    Disable {
        name: String,
    },
}

#[derive(Subcommand)]
pub enum DownloaderCommand {
    /// 列出所有下载后端
    List {
        /// 显示后端的详细说明
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,
    },
    /// 切换后端启用/禁用状态
    Set {
        /// 后端名称
        name: String,
        /// 启用或禁用（on/off）
        state: String,
    },
    /// 在资源管理器中打开配置目录
    Open,
}

#[derive(Subcommand)]
#[command(
    arg_required_else_help = true,
    styles = styles(),
    disable_help_subcommand = true,
)]
pub enum ToolCli {
    /// 初始化环境（默认打印 PATH 提示，-g 写入注册表）
    Init(ToolInitOpts),
    /// 安装/升级自研工具（--upgrade 升级模式）
    Add(ToolAddOpts),
    /// 列出已安装的自研工具
    List,
    /// 移除一个自研工具
    Remove(ToolRemoveOpts),
}

#[derive(Args)]
pub struct ToolInitOpts {
    /// 写入用户 PATH 注册表
    #[arg(short = 'g', long = "global")]
    pub global: bool,
}

#[derive(Args)]
pub struct ToolAddOpts {
    /// 工具名称（可同时指定多个）
    pub names: Vec<String>,

    /// 指定版本号
    #[arg(short = 'v', long = "ver")]
    pub version: Option<String>,

    /// 强制重新下载
    #[arg(short = 'r', long = "renew")]
    pub renew: bool,

    /// 仅下载，不安装
    #[arg(short = 'd', long = "download-only")]
    pub download_only: bool,

    /// 升级模式
    #[arg(short = 'u', long = "upgrade")]
    pub upgrade: bool,
}

#[derive(Args)]
pub struct ToolRemoveOpts {
    /// 工具名称
    pub name: String,
}
