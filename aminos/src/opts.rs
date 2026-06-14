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
}

impl InstallOpts {
    pub fn new(names: Vec<String>, version: Option<String>, gui: bool, renew: bool, download_only: bool) -> Self {
        Self {
            names,
            version: version.unwrap_or_default(),
            gui,
            renew,
            download_only,
        }
    }
}

/// 列表显示选项
#[derive(Debug, Clone)]
pub struct ListOpts {
    /// 按分类过滤
    pub filter: Option<String>,
    /// 仅显示已安装
    pub install_only: bool,
    /// 仅显示未安装
    pub missing: bool,
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
        filter: Option<String>,
        install_only: bool,
        missing: bool,
        search: Option<String>,
        downloaded: bool,
        downloading: bool,
        no_download: bool,
        group: bool,
        categories: bool,
    ) -> Self {
        Self {
            filter,
            install_only,
            missing,
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

/// 升级选项
#[derive(Debug, Clone)]
pub struct UpgradeOpts {
    /// 可选：仅升级指定软件（空则全部升级）
    pub names: Vec<String>,
    /// 仅检查更新，不下也不装
    pub check: bool,
    /// 强制重新下载（即使版本相同）
    pub renew: bool,
}

impl UpgradeOpts {
    pub fn new(names: Vec<String>, check: bool, renew: bool) -> Self {
        Self { names, check, renew }
    }
}
