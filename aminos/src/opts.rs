/// 安装选项
#[derive(Debug, Clone)]
pub struct InstallOpts {
    /// 软件名称列表
    pub names: Vec<String>,
    /// 指定版本号（空字符串表示默认版本）
    pub version: String,
    /// 使用图形界面安装（不静默）
    pub gui: bool,
    /// 强制重新下载
    pub renew: bool,
    /// 仅下载，不安装
    pub download_only: bool,
    /// 安装类型：Some("portable") 或 Some("installer")，None 为交互选择
    pub inst_type: Option<String>,
    /// 升级模式：检测更新，卸载旧版后安装新版
    pub upgrade: bool,
}

impl InstallOpts {
    #[allow(clippy::too_many_arguments)]
    pub fn new(names: Vec<String>, version: Option<String>, gui: bool, renew: bool, download_only: bool, inst_type: Option<String>, upgrade: bool) -> Self {
        Self {
            names,
            version: version.unwrap_or_default(),
            gui,
            renew,
            download_only,
            inst_type,
            upgrade,
        }
    }
}

/// 列表显示选项
#[derive(Debug, Clone)]
pub struct ListOpts {
    /// 显示全部（已安装 + 源中可用）
    pub all: bool,
    /// 按分类过滤
    pub filter: Option<String>,
    /// 搜索关键字
    pub search: Option<String>,
    /// 仅显示已下载
    pub downloaded: bool,
    /// 仅显示下载中
    pub downloading: bool,
    /// 仅显示未下载
    pub no_download: bool,
    /// 按分类分组显示
    pub group: bool,
    /// 显示所有分类概览
    pub categories: bool,
}

impl ListOpts {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        all: bool,
        filter: Option<String>,
        search: Option<String>,
        downloaded: bool,
        downloading: bool,
        no_download: bool,
        group: bool,
        categories: bool,
    ) -> Self {
        Self {
            all,
            filter,
            search,
            downloaded,
            downloading,
            no_download,
            group,
            categories,
        }
    }
}

/// 卸载选项
#[derive(Debug, Clone)]
pub struct UninstallOpts {
    /// 软件名称列表
    pub names: Vec<String>,
    /// 使用图形界面卸载
    pub gui: bool,
    /// 强制删除
    pub force: bool,
}

impl UninstallOpts {
    pub fn new(names: Vec<String>, gui: bool, force: bool) -> Self {
        Self { names, gui, force }
    }
}

/// 下载选项
#[derive(Debug, Clone)]
pub struct DownloadOpts {
    /// 软件名称或 URL
    pub targets: Vec<String>,
    /// 打开下载目录
    pub open: bool,
    /// 下载到指定目录
    pub target_dir: Option<String>,
}

impl DownloadOpts {
    pub fn new(targets: Vec<String>, open: bool, target_dir: Option<String>) -> Self {
        Self { targets, open, target_dir }
    }
}

/// 自研工具添加选项
#[derive(Debug, Clone)]
pub struct ToolAddOpts {
    /// 工具名称列表
    pub names: Vec<String>,
    /// 指定版本号（空字符串表示默认）
    pub version: String,
    /// 强制重新下载
    pub renew: bool,
    /// 仅下载，不安装
    pub download_only: bool,
    /// 升级模式
    pub upgrade: bool,
}

impl ToolAddOpts {
    pub fn new(names: Vec<String>, version: Option<String>, renew: bool, download_only: bool, upgrade: bool) -> Self {
        Self {
            names,
            version: version.unwrap_or_default(),
            renew,
            download_only,
            upgrade,
        }
    }
}
