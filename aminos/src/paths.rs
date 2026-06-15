use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

/// 源定义根目录：`source/`
pub fn source_dir() -> PathBuf {
    RESOLVER.source_dir()
}

/// 第三方软件源定义目录：`source/apps/`
pub fn apps_source_dir() -> PathBuf {
    source_dir().join("apps")
}

/// 自研工具源定义目录：`source/tools/`
pub fn tools_source_dir() -> PathBuf {
    source_dir().join("tools")
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
    RESOLVER.installed_json()
}

// ── 自研工具目录 ─────────────────────────────────

/// 自研工具安装目录：%LOCALAPPDATA%\aminos\tools\{name}\
pub fn tools_dir() -> PathBuf {
    RESOLVER.appdata_root().join("tools")
}

/// 自研工具 PATH 入口目录：%LOCALAPPDATA%\aminos\tools\bin\
/// 此目录应被注册到用户 PATH 中，所有工具在此创建硬链接。
pub fn tools_bin_dir() -> PathBuf {
    tools_dir().join("bin")
}

/// 下载引擎配置目录：%LOCALAPPDATA%\aminos\config\
pub fn config_dir() -> PathBuf {
    RESOLVER.appdata_root().join("config")
}
