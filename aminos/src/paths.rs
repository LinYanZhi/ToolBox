use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

/// 源定义根目录：`source/`
pub fn source_dir() -> PathBuf {
    RESOLVER.source_dir()
}

/// 分类后的第三方软件源目录（所有分类子目录的父级）。
/// 子目录包括：editors, ide, lang, devtools, browsers, social,
/// media, office, system, enhance, remote, network, security, virt
pub fn apps_source_dir() -> PathBuf {
    source_dir().join("apps")
}

/// 返回所有软件分类目录的列表。
pub fn app_category_dirs() -> Vec<PathBuf> {
    CATEGORIES.iter().map(|name| source_dir().join(name)).collect()
}

/// 返回指定分类的目录：`source/{name}/`
pub fn category_dir(name: &str) -> PathBuf {
    source_dir().join(name)
}

/// 自研工具源定义目录：`source/tools/`
pub fn tools_source_dir() -> PathBuf {
    source_dir().join("tools")
}

/// 第三方社区源根目录：`source/community/`
pub fn community_source_dir() -> PathBuf {
    source_dir().join("community")
}

/// 获取某个第三方源的本地缓存目录：`source/community/{name}/`
pub fn community_source_named(name: &str) -> PathBuf {
    community_source_dir().join(name)
}

pub fn builds_dir() -> PathBuf {
    RESOLVER.appdata_root().join("builds")
}

pub fn downloads_dir() -> PathBuf {
    RESOLVER.downloads_dir()
}

pub fn apps_dir() -> PathBuf {
    RESOLVER.apps_dir()
}

pub fn installed_json() -> PathBuf {
    RESOLVER.apps_dir().join("installed.json")
}

// ── 自研工具目录 ─────────────────────────────────

/// 自研工具安装目录：%LOCALAPPDATA%\aminos\tools\{name}\
pub fn tools_dir() -> PathBuf {
    RESOLVER.appdata_root().join("tools")
}

/// 自研工具 PATH 入口目录：%LOCALAPPDATA%\aminos\tools\bin\
pub fn tools_bin_dir() -> PathBuf {
    tools_dir().join("bin")
}

/// 下载引擎配置目录：%LOCALAPPDATA%\aminos\config\
pub fn config_dir() -> PathBuf {
    RESOLVER.appdata_root().join("config")
}

// ── 源分类定义（与 GitHub 仓库目录结构对应） ────────

/// 所有软件分类目录名（用于本地缓存和远程同步）。
pub const CATEGORIES: &[&str] = &[
    "editors",
    "ide",
    "lang",
    "devtools",
    "browsers",
    "social",
    "media",
    "office",
    "system",
    "enhance",
    "remote",
    "network",
    "security",
    "virt",
];

/// 分类的显示名称和描述（与 cmd_source.rs 共享）。
pub const CATEGORY_META: &[(&str, &str, &str)] = &[
    ("editors",  "代码编辑器",   "VSCode/Notepad++/Trae/Cursor"),
    ("ide",      "集成开发环境", "PyCharm/DataGrip/Visual Studio"),
    ("lang",     "编程语言",     "Python/Node.js"),
    ("devtools", "开发工具",     "Git/影刀RPA"),
    ("browsers", "浏览器",       "Chrome/Edge/Firefox"),
    ("social",   "社交聊天",     "微信/企业微信/钉钉/飞书"),
    ("media",    "影音娱乐",     "PotPlayer/网易云/Steam/OBS"),
    ("office",   "办公软件",     "Office/WPS/PDFgear"),
    ("system",   "系统工具",     "Everything/SpaceSniffer/IObitUnlocker/WePE"),
    ("enhance",  "效率增强",     "uTools/Snipaste/PixPin/7-Zip/TTime"),
    ("remote",   "远程控制",     "向日葵/ToDesk/网易UU远程"),
    ("network",  "网络工具",     "Clash Verge/FlClash"),
    ("security", "安全防护",     "火绒安全/Watt Toolkit"),
    ("virt",     "虚拟化",       "VMware Workstation"),
];
