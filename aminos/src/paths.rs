use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

/// 源定义根目录：`source/`（用于社区源缓存）。
pub fn source_dir() -> PathBuf {
    RESOLVER.source_dir()
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

/// 软件源定义（内置 + 本地扩展）：%LOCALAPPDATA%\aminos\software.json
pub fn software_json_path() -> PathBuf {
    RESOLVER.appdata_root().join("software.json")
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
