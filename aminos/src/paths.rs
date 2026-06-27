use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

/// %LOCALAPPDATA%/aminos/apps/installed.json
pub fn installed_json() -> PathBuf {
    RESOLVER.apps_dir().join("installed.json")
}

/// %LOCALAPPDATA%/aminos/apps/（便携版安装目录）
pub fn apps_dir() -> PathBuf {
    RESOLVER.apps_dir()
}

/// %LOCALAPPDATA%/aminos/downloads/
pub fn downloads_dir() -> PathBuf {
    RESOLVER.downloads_dir()
}

/// %LOCALAPPDATA%/aminos/tools/bin/（自研工具目录）
pub fn tools_bin_dir() -> PathBuf {
    RESOLVER.appdata_root().join("tools").join("bin")
}
