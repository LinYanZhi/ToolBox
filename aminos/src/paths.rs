use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

pub fn source_dir() -> PathBuf {
    RESOLVER.source_dir()
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
